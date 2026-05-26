use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use axum::{body::Body, Router};
use b3_control::{app as control_app, ControlState};
use b3_core::{
    BranchId, ContractError, ContractResult, EmbeddingVector, FileId, FileRecord, IndexStore,
    IndexedFileRecord, ProjectId, QueryScope, SourceKind, VectorDocument, VectorStore,
};
use b3_embeddings::{
    EmbeddingProvider, LocalHashEmbeddingProvider, DEFAULT_LOCAL_HASH_DIMENSION,
    LOCAL_HASH_MODEL_ID, LOCAL_HASH_PROVIDER_ID,
};
use b3_indexer::embedding::{ChunkPlanner, ChunkPlannerConfig, ChunkSource};
use b3_mcp_runtime::{
    handle_json_rpc_line, registered_tools_for_profile, McpQueryToolRouter, ToolProfileConfig,
    ToolProfileName,
};
use b3_query::{
    hybrid::{HybridSearchEngine, HybridSearchRequest},
    LocalQueryEngine, QueryEngineConfig,
};
use b3_storage::SqliteStorage;
use http::Request;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower::ServiceExt;

use crate::{to_contract_error, BenchmarkFixture, BenchmarkResult, BRANCH_ID, PROJECT_ID};

const MODE_LEXICAL: &str = "lexical_only";
const MODE_VECTOR: &str = "vector_only";
const MODE_HYBRID: &str = "hybrid";
const DEFAULT_TOP_K: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticQualityReport {
    pub fixture: String,
    pub query_count: usize,
    pub document_count: usize,
    pub vector_count: usize,
    pub search_modes: Vec<SearchModeReport>,
    pub control_mcp: ControlMcpVerification,
    pub token_savings_estimate: TokenSavingsSummary,
    pub warnings: Vec<String>,
    pub quality_limitations: Vec<String>,
    pub local_only: bool,
    pub external_api_required: bool,
    pub telemetry_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchModeReport {
    pub mode: String,
    pub hit_at_1: f64,
    pub hit_at_3: f64,
    pub hit_at_5: f64,
    pub hit_at_10: f64,
    pub mrr: f64,
    pub average_final_score: f64,
    pub average_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub result_count: usize,
    pub fallback_count: usize,
    pub no_vector_fallback_count: usize,
    pub source_kind_match_rate: f64,
    pub file_match_rate: f64,
    pub symbol_match_rate: f64,
    pub cases: Vec<QueryEvaluationResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryEvaluationResult {
    pub query: String,
    pub notes: String,
    pub expected_files: Vec<String>,
    pub expected_symbol: Option<String>,
    pub expected_source_kind: Option<String>,
    pub rank: Option<usize>,
    pub top_path: Option<String>,
    pub top_score: Option<f32>,
    pub result_count: usize,
    pub latency_ms: f64,
    pub warnings: Vec<String>,
    pub matched_expected_file: bool,
    pub matched_expected_symbol: bool,
    pub matched_source_kind: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlMcpVerification {
    pub control_hybrid_endpoint_status: u16,
    pub semantic_search_tool_present: bool,
    pub tiny_count: usize,
    pub optimized_count: usize,
    pub full_count: usize,
    pub debug_count: usize,
    pub readonly_count: usize,
    pub editing_count: usize,
    pub web_app_count: usize,
    pub enterprise_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSavingsSummary {
    pub naive_context_chars: usize,
    pub selected_context_chars: usize,
    pub estimated_naive_tokens: usize,
    pub estimated_selected_tokens: usize,
    pub estimated_token_reduction_percent: u8,
    pub estimate_method: String,
}

#[derive(Debug, Clone)]
struct QualityCase {
    query: &'static str,
    expected_files: &'static [&'static str],
    expected_symbol: Option<&'static str>,
    expected_source_kind: Option<SourceKind>,
    language: Option<&'static str>,
    framework: Option<&'static str>,
    notes: &'static str,
}

struct SemanticFixtureState {
    storage: SqliteStorage,
    document_count: usize,
    vector_count: usize,
    file_chars: BTreeMap<String, usize>,
}

pub fn run_semantic_quality_benchmark(
    fixture: &BenchmarkFixture,
) -> ContractResult<(SemanticQualityReport, Vec<BenchmarkResult>)> {
    let cases = quality_cases();
    validate_cases(&cases, &fixture.path)?;
    let setup_started = Instant::now();
    let state = seed_semantic_fixture(fixture)?;
    let mut benchmark_results = vec![benchmark_result_with_count(
        "semantic_fixture_setup_latency",
        setup_started.elapsed(),
        state.document_count,
    )];

    let mut reports = Vec::new();
    for mode in [MODE_LEXICAL, MODE_VECTOR, MODE_HYBRID] {
        let started = Instant::now();
        let report = evaluate_mode(mode, &state, &cases)?;
        let mut result = benchmark_result_with_count(
            format!("semantic_quality_{mode}_latency"),
            started.elapsed(),
            report.result_count,
        );
        result
            .metadata
            .insert("hit_at_1".to_string(), format!("{:.3}", report.hit_at_1));
        result
            .metadata
            .insert("hit_at_3".to_string(), format!("{:.3}", report.hit_at_3));
        result
            .metadata
            .insert("mrr".to_string(), format!("{:.3}", report.mrr));
        benchmark_results.push(result);
        reports.push(report);
    }

    benchmark_results.push(bench_mcp_semantic_search(&state.storage)?);
    benchmark_results.push(bench_control_hybrid_endpoint()?);

    let control_mcp = verify_control_mcp()?;
    let token_savings_estimate = token_savings_summary(&state, reports.as_slice());
    let warnings = quality_warnings(&reports);
    let report = SemanticQualityReport {
        fixture: fixture.name.clone(),
        query_count: cases.len(),
        document_count: state.document_count,
        vector_count: state.vector_count,
        search_modes: reports,
        control_mcp,
        token_savings_estimate,
        warnings,
        quality_limitations: vec![
            "local_hash is lexical/hash-based, not a neural embedding model".to_string(),
            "metrics are fixture-based baselines, not production quality guarantees".to_string(),
            "SQLite vector search is exact brute-force over filtered local candidates".to_string(),
        ],
        local_only: true,
        external_api_required: false,
        telemetry_enabled: false,
    };
    Ok((report, benchmark_results))
}

pub fn format_semantic_quality_summary(report: &SemanticQualityReport) -> String {
    let mut lines = vec![
        String::new(),
        "semantic quality | hit@1 | hit@3 | hit@5 | hit@10 | mrr | avg_ms | fallbacks".to_string(),
        "--- | ---: | ---: | ---: | ---: | ---: | ---: | ---:".to_string(),
    ];
    for mode in &report.search_modes {
        lines.push(format!(
            "{} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {}",
            mode.mode,
            mode.hit_at_1,
            mode.hit_at_3,
            mode.hit_at_5,
            mode.hit_at_10,
            mode.mrr,
            mode.average_latency_ms,
            mode.fallback_count
        ));
    }
    lines.push(format!(
        "token estimate | naive_chars={} | selected_chars={} | reduction={}%",
        report.token_savings_estimate.naive_context_chars,
        report.token_savings_estimate.selected_context_chars,
        report
            .token_savings_estimate
            .estimated_token_reduction_percent
    ));
    lines.push(
        "offline/free: no external APIs, telemetry, hosted vector DBs, or model downloads"
            .to_string(),
    );
    lines.push(
        "quality note: fixture baseline only; local_hash is not neural semantic quality"
            .to_string(),
    );
    lines.join("\n")
}

fn quality_cases() -> Vec<QualityCase> {
    vec![
        QualityCase {
            query: "where is order creation handled",
            expected_files: &["rust/orders.rs", "next/app/api/orders/route.ts"],
            expected_symbol: Some("create_order"),
            expected_source_kind: Some(SourceKind::SymbolChunk),
            language: None,
            framework: None,
            notes: "order creation flow",
        },
        QualityCase {
            query: "find the API route that creates users",
            expected_files: &["web/users.controller.ts"],
            expected_symbol: Some("createUser"),
            expected_source_kind: Some(SourceKind::RouteChunk),
            language: Some("typescript"),
            framework: Some("nestjs"),
            notes: "NestJS user route",
        },
        QualityCase {
            query: "where is payment.created published",
            expected_files: &["messaging/payments.ts"],
            expected_symbol: Some("publishPaymentCreated"),
            expected_source_kind: Some(SourceKind::MessagingChunk),
            language: Some("typescript"),
            framework: None,
            notes: "payment producer",
        },
        QualityCase {
            query: "who consumes order.created",
            expected_files: &["messaging/payments.ts"],
            expected_symbol: Some("consumeOrderCreated"),
            expected_source_kind: Some(SourceKind::MessagingChunk),
            language: Some("typescript"),
            framework: None,
            notes: "order consumer",
        },
        QualityCase {
            query: "find the WPF SaveCommand binding",
            expected_files: &["wpf/MainWindow.xaml"],
            expected_symbol: None,
            expected_source_kind: Some(SourceKind::WpfChunk),
            language: Some("xaml"),
            framework: Some("wpf"),
            notes: "WPF command binding",
        },
        QualityCase {
            query: "where is the Kubernetes deployment for api",
            expected_files: &["infra/deployment.yaml"],
            expected_symbol: None,
            expected_source_kind: Some(SourceKind::InfrastructureChunk),
            language: Some("yaml"),
            framework: Some("kubernetes"),
            notes: "deployment target",
        },
        QualityCase {
            query: "find database query for users",
            expected_files: &["dotnet/UserRepository.cs"],
            expected_symbol: Some("FindUserByEmail"),
            expected_source_kind: Some(SourceKind::DataAccessChunk),
            language: Some("csharp"),
            framework: Some("dapper"),
            notes: "Dapper-like SQL query",
        },
        QualityCase {
            query: "where is websocket message sent",
            expected_files: &["realtime/notifications.ts"],
            expected_symbol: Some("sendWebSocketNotification"),
            expected_source_kind: Some(SourceKind::RealtimeChunk),
            language: Some("typescript"),
            framework: Some("websocket"),
            notes: "WebSocket notification",
        },
        QualityCase {
            query: "find Go health endpoint",
            expected_files: &["go/main.go"],
            expected_symbol: Some("healthHandler"),
            expected_source_kind: Some(SourceKind::GoChunk),
            language: Some("go"),
            framework: Some("net/http"),
            notes: "Go HTTP handler",
        },
        QualityCase {
            query: "find React order component",
            expected_files: &["web/OrderPanel.tsx"],
            expected_symbol: Some("OrderPanel"),
            expected_source_kind: Some(SourceKind::ComponentChunk),
            language: Some("tsx"),
            framework: Some("react"),
            notes: "React component",
        },
    ]
}

fn validate_cases(cases: &[QualityCase], root: &Path) -> ContractResult<()> {
    for case in cases {
        if case.query.trim().is_empty() {
            return Err(ContractError::new("quality query must not be empty"));
        }
        if case.expected_files.is_empty() {
            return Err(ContractError::new(format!(
                "quality query has no expected files: {}",
                case.query
            )));
        }
        for file in case.expected_files {
            if !root.join(file).exists() {
                return Err(ContractError::new(format!(
                    "missing expected quality fixture target: {file}"
                )));
            }
        }
    }
    Ok(())
}

fn seed_semantic_fixture(fixture: &BenchmarkFixture) -> ContractResult<SemanticFixtureState> {
    let db = fixture.path.join(".b3").join("semantic-quality.db");
    let storage = SqliteStorage::open(db)?;
    let project_id = ProjectId::new(PROJECT_ID);
    let branch_id = BranchId::new(BRANCH_ID);
    storage.ensure_project_branch(&project_id, &branch_id, &fixture.path.to_string_lossy())?;
    let provider = LocalHashEmbeddingProvider::default_provider();
    let chunk_planner = ChunkPlanner::new(ChunkPlannerConfig {
        max_chunk_chars: 2_000,
    });
    let mut documents = Vec::<VectorDocument>::new();
    let mut vectors = Vec::<EmbeddingVector>::new();
    let mut file_chars = BTreeMap::new();

    for file_path in fixture_files(&fixture.path)? {
        let relative = normalize_relative(&fixture.path, &file_path)?;
        let content = fs::read_to_string(&file_path).map_err(to_contract_error)?;
        let language = language_for_path(&relative);
        let framework = framework_for_path(&relative, &content);
        let source_kind = source_kind_for_path(&relative);
        let file_id = FileId::new(format!("semantic:{relative}"));
        let file = FileRecord {
            id: file_id.clone(),
            project_id: project_id.clone(),
            path: relative.clone(),
            content_hash: b3_core::stable_hash(&[relative.as_str(), content.as_str()]),
        };
        storage.upsert_indexed_file(
            &project_id,
            &branch_id,
            IndexedFileRecord {
                file: file.clone(),
                language: language.clone(),
                size_bytes: content.len() as u64,
                content: content.clone(),
                symbols: Vec::new(),
                edges: Vec::new(),
            },
        )?;
        let chunks = chunk_planner.plan_source(ChunkSource {
            project_id: project_id.clone(),
            branch_id: branch_id.clone(),
            file_id,
            symbol_id: None,
            language,
            framework,
            source_kind,
            path: relative.clone(),
            content_hash: file.content_hash,
            text: content.clone(),
            start_line: 1,
            metadata: BTreeMap::from([
                (
                    "benchmark_fixture".to_string(),
                    "semantic_quality".to_string(),
                ),
                (
                    "benchmark_source_kind".to_string(),
                    source_kind.as_str().to_string(),
                ),
            ]),
        });
        for document in chunks {
            let vector = provider.embed_text(&document.text)?;
            vectors.push(
                EmbeddingVector::new(
                    document.id.clone(),
                    LOCAL_HASH_PROVIDER_ID,
                    DEFAULT_LOCAL_HASH_DIMENSION,
                    vector,
                    1,
                )
                .with_model_id(LOCAL_HASH_MODEL_ID),
            );
            documents.push(document);
        }
        file_chars.insert(relative, content.len());
    }
    storage.upsert_documents(&documents)?;
    storage.upsert_vectors(&vectors)?;
    Ok(SemanticFixtureState {
        storage,
        document_count: documents.len(),
        vector_count: vectors.len(),
        file_chars,
    })
}

fn evaluate_mode(
    mode: &str,
    state: &SemanticFixtureState,
    cases: &[QualityCase],
) -> ContractResult<SearchModeReport> {
    let engine = HybridSearchEngine::new(&state.storage, &state.storage);
    let mut case_results = Vec::new();
    let mut latencies = Vec::new();
    let mut result_count = 0usize;
    let mut fallback_count = 0usize;
    let mut no_vector_fallback_count = 0usize;
    let mut reciprocal_rank_sum = 0.0;
    let mut score_sum = 0.0;
    let mut score_count = 0usize;
    let mut source_kind_matches = 0usize;
    let mut file_matches = 0usize;
    let mut symbol_matches = 0usize;

    for case in cases {
        let mut request = HybridSearchRequest::new(bench_scope(), case.query);
        request.limit = DEFAULT_TOP_K;
        request.explain = true;
        request.language = case.language.map(str::to_string);
        request.framework = case.framework.map(str::to_string);
        match mode {
            MODE_LEXICAL => {
                request.lexical_weight = 1.0;
                request.vector_weight = 0.0;
                request.metadata_weight = 0.0;
            }
            MODE_VECTOR => {
                request.lexical_weight = 0.0;
                request.vector_weight = 1.0;
                request.metadata_weight = 0.0;
            }
            _ => {}
        }
        let started = Instant::now();
        let response = engine.search(request)?;
        let latency = started.elapsed().as_secs_f64() * 1000.0;
        latencies.push(latency);
        result_count += response.results.len();
        if !response.warnings.is_empty() {
            fallback_count += 1;
        }
        if response
            .warnings
            .iter()
            .any(|warning| warning.contains("No vector data available"))
        {
            no_vector_fallback_count += 1;
        }
        let rank = response
            .results
            .iter()
            .position(|result| expected_file_match(&result.path, case.expected_files))
            .map(|index| index + 1);
        if let Some(rank) = rank {
            reciprocal_rank_sum += 1.0 / rank as f64;
            file_matches += 1;
        }
        let top = response.results.first();
        if let Some(top) = top {
            score_sum += f64::from(top.final_score);
            score_count += 1;
        }
        let source_kind_match = top
            .zip(case.expected_source_kind)
            .is_some_and(|(result, expected)| result.source_kind == expected);
        let symbol_match = case.expected_symbol.is_some_and(|symbol| {
            response.results.iter().any(|result| {
                result.explanation.as_ref().is_some_and(|explanation| {
                    explanation
                        .matched_terms
                        .iter()
                        .any(|term| symbol.to_lowercase().contains(term))
                }) || result
                    .text_preview
                    .to_lowercase()
                    .contains(&symbol.to_lowercase())
            })
        });
        if source_kind_match {
            source_kind_matches += 1;
        }
        if symbol_match {
            symbol_matches += 1;
        }
        case_results.push(QueryEvaluationResult {
            query: case.query.to_string(),
            notes: case.notes.to_string(),
            expected_files: case
                .expected_files
                .iter()
                .map(|file| file.to_string())
                .collect(),
            expected_symbol: case.expected_symbol.map(str::to_string),
            expected_source_kind: case
                .expected_source_kind
                .map(|kind| kind.as_str().to_string()),
            rank,
            top_path: top.map(|result| result.path.clone()),
            top_score: top.map(|result| result.final_score),
            result_count: response.results.len(),
            latency_ms: latency,
            warnings: response.warnings,
            matched_expected_file: rank.is_some(),
            matched_expected_symbol: symbol_match,
            matched_source_kind: source_kind_match,
        });
    }

    let query_count = cases.len().max(1) as f64;
    let symbol_query_count = cases
        .iter()
        .filter(|case| case.expected_symbol.is_some())
        .count()
        .max(1) as f64;
    Ok(SearchModeReport {
        mode: mode.to_string(),
        hit_at_1: hit_at(&case_results, 1),
        hit_at_3: hit_at(&case_results, 3),
        hit_at_5: hit_at(&case_results, 5),
        hit_at_10: hit_at(&case_results, 10),
        mrr: reciprocal_rank_sum / query_count,
        average_final_score: if score_count == 0 {
            0.0
        } else {
            score_sum / score_count as f64
        },
        average_latency_ms: average(&latencies),
        p50_latency_ms: percentile(latencies.clone(), 50.0),
        p95_latency_ms: percentile(latencies, 95.0),
        result_count,
        fallback_count,
        no_vector_fallback_count,
        source_kind_match_rate: source_kind_matches as f64 / query_count,
        file_match_rate: file_matches as f64 / query_count,
        symbol_match_rate: symbol_matches as f64 / symbol_query_count,
        cases: case_results,
    })
}

fn token_savings_summary(
    state: &SemanticFixtureState,
    reports: &[SearchModeReport],
) -> TokenSavingsSummary {
    let naive_context_chars = state.file_chars.values().sum::<usize>();
    let selected_context_chars = reports
        .iter()
        .find(|report| report.mode == MODE_HYBRID)
        .map(|report| {
            report
                .cases
                .iter()
                .filter_map(|case| case.top_path.as_ref())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter_map(|path| state.file_chars.get(path))
                .sum::<usize>()
        })
        .unwrap_or_default();
    let estimated_naive_tokens = estimate_tokens(naive_context_chars);
    let estimated_selected_tokens = estimate_tokens(selected_context_chars);
    let estimated_token_reduction_percent = if naive_context_chars == 0 {
        0
    } else {
        (((naive_context_chars.saturating_sub(selected_context_chars)) as f64
            / naive_context_chars as f64)
            * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    };
    TokenSavingsSummary {
        naive_context_chars,
        selected_context_chars,
        estimated_naive_tokens,
        estimated_selected_tokens,
        estimated_token_reduction_percent,
        estimate_method: "chars divided by 4; fixture-local estimate only".to_string(),
    }
}

fn bench_mcp_semantic_search(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let router = McpQueryToolRouter::new(engine);
    let started = Instant::now();
    let outcome = handle_json_rpc_line(
        &router,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"semantic_search","arguments":{"scope":{"project_id":"bench","branch_id":"main"},"query":"order creation","limit":5,"explain":false}}}"#,
    )
    .map_err(ContractError::new)?;
    let mut result = BenchmarkResult::new("mcp_semantic_search_latency", started.elapsed());
    result.query_result_count = outcome
        .response
        .and_then(|value| {
            let text = value["result"]["content"][0]["text"].as_str()?;
            serde_json::from_str::<Value>(text).ok()
        })
        .and_then(|value| value["results"].as_array().map(Vec::len))
        .unwrap_or_default();
    Ok(result)
}

fn bench_control_hybrid_endpoint() -> ContractResult<BenchmarkResult> {
    let started = Instant::now();
    let status = control_hybrid_status()?;
    let mut result = BenchmarkResult::new("control_hybrid_search_latency", started.elapsed());
    result.query_result_count = usize::from(status == 200);
    result
        .metadata
        .insert("status".to_string(), status.to_string());
    Ok(result)
}

fn verify_control_mcp() -> ContractResult<ControlMcpVerification> {
    Ok(ControlMcpVerification {
        control_hybrid_endpoint_status: control_hybrid_status()?,
        semantic_search_tool_present: registered_tools_for_profile(&ToolProfileConfig::new(
            ToolProfileName::Optimized,
        ))
        .iter()
        .any(|tool| tool.name == "semantic_search"),
        tiny_count: tool_count(ToolProfileName::Tiny),
        optimized_count: tool_count(ToolProfileName::Optimized),
        full_count: tool_count(ToolProfileName::Full),
        debug_count: tool_count(ToolProfileName::Debug),
        readonly_count: tool_count(ToolProfileName::Readonly),
        editing_count: tool_count(ToolProfileName::Editing),
        web_app_count: tool_count(ToolProfileName::WebApp),
        enterprise_count: tool_count(ToolProfileName::Enterprise),
    })
}

fn control_hybrid_status() -> ContractResult<u16> {
    let runtime = tokio::runtime::Runtime::new().map_err(to_contract_error)?;
    let storage = SqliteStorage::open_in_memory()?;
    let app = control_app(ControlState::from_storage(
        PathBuf::from("."),
        PathBuf::from(":memory:"),
        storage,
    ));
    runtime.block_on(post_json_status(
        app,
        "/api/search/hybrid",
        r#"{"query":"order creation","project_id":"bench","branch_id":"main","limit":5}"#,
    ))
}

async fn post_json_status(app: Router, uri: &str, body: &str) -> ContractResult<u16> {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(uri)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .map_err(to_contract_error)?,
    )
    .await
    .map(|response| response.status().as_u16())
    .map_err(to_contract_error)
}

fn tool_count(profile: ToolProfileName) -> usize {
    registered_tools_for_profile(&ToolProfileConfig::new(profile)).len()
}

fn quality_warnings(reports: &[SearchModeReport]) -> Vec<String> {
    let mut warnings = Vec::new();
    for report in reports {
        if report.hit_at_1 < 1.0 {
            warnings.push(format!(
                "{} hit@1 is {:.3}; inspect fixture misses before overclaiming quality",
                report.mode, report.hit_at_1
            ));
        }
        if report.no_vector_fallback_count > 0 {
            warnings.push(format!(
                "{} had {} no-vector fallbacks",
                report.mode, report.no_vector_fallback_count
            ));
        }
    }
    warnings.sort();
    warnings.dedup();
    warnings
}

fn fixture_files(root: &Path) -> ContractResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> ContractResult<()> {
    for entry in fs::read_dir(path).map_err(to_contract_error)? {
        let entry = entry.map_err(to_contract_error)?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(to_contract_error)?;
        if metadata.is_dir() {
            if entry.file_name() == ".b3" {
                continue;
            }
            collect_files(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn normalize_relative(root: &Path, path: &Path) -> ContractResult<String> {
    path.strip_prefix(root)
        .map_err(to_contract_error)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

fn source_kind_for_path(path: &str) -> SourceKind {
    if path.contains("OrderPanel") {
        SourceKind::ComponentChunk
    } else if path.contains("users.controller") || path.contains("api/orders/route") {
        SourceKind::RouteChunk
    } else if path.contains("Repository") {
        SourceKind::DataAccessChunk
    } else if path.contains("notifications") {
        SourceKind::RealtimeChunk
    } else if path.contains("payments") {
        SourceKind::MessagingChunk
    } else if path.contains("deployment.yaml") {
        SourceKind::InfrastructureChunk
    } else if path.contains("MainWindow.xaml") {
        SourceKind::WpfChunk
    } else if path.ends_with(".go") {
        SourceKind::GoChunk
    } else {
        SourceKind::SymbolChunk
    }
}

fn language_for_path(path: &str) -> Option<String> {
    let language = if path.ends_with(".rs") {
        "rust"
    } else if path.ends_with(".tsx") {
        "tsx"
    } else if path.ends_with(".ts") {
        "typescript"
    } else if path.ends_with(".cs") {
        "csharp"
    } else if path.ends_with(".go") {
        "go"
    } else if path.ends_with(".xaml") {
        "xaml"
    } else if path.ends_with(".yaml") || path.ends_with(".yml") {
        "yaml"
    } else {
        return None;
    };
    Some(language.to_string())
}

fn framework_for_path(path: &str, content: &str) -> Option<String> {
    let framework = if path.contains("OrderPanel") {
        "react"
    } else if path.contains("next/") {
        "nextjs"
    } else if path.contains("angular/") {
        "angular"
    } else if path.contains("users.controller.ts") {
        "nestjs"
    } else if path.contains("UsersController.cs") {
        "aspnetcore"
    } else if path.contains("UserRepository.cs") {
        "dapper"
    } else if path.contains("notifications") {
        "websocket"
    } else if path.contains("deployment.yaml") {
        "kubernetes"
    } else if path.contains("MainWindow.xaml") {
        "wpf"
    } else if path.ends_with(".go") {
        "net/http"
    } else if content.contains("payment.created") || content.contains("order.created") {
        "messaging"
    } else {
        return None;
    };
    Some(framework.to_string())
}

fn expected_file_match(path: &str, expected_files: &[&str]) -> bool {
    expected_files
        .iter()
        .any(|expected| path.ends_with(expected))
}

fn hit_at(results: &[QueryEvaluationResult], k: usize) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    let hits = results
        .iter()
        .filter(|result| result.rank.is_some_and(|rank| rank <= k))
        .count();
    hits as f64 / results.len() as f64
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn percentile(mut values: Vec<f64>, percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((values.len() - 1) as f64 * percentile / 100.0).round() as usize;
    values[index.min(values.len() - 1)]
}

fn estimate_tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

fn benchmark_result_with_count(
    name: impl Into<String>,
    duration: Duration,
    count: usize,
) -> BenchmarkResult {
    let mut result = BenchmarkResult::new(name, duration);
    result.query_result_count = count;
    result
}

fn bench_scope() -> QueryScope {
    QueryScope::new(ProjectId::new(PROJECT_ID), BranchId::new(BRANCH_ID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_fixture_root, load_fixtures};

    #[test]
    fn quality_cases_have_local_expected_targets() {
        let root = default_fixture_root().join("semantic_search_repo");
        validate_cases(&quality_cases(), &root).expect("cases");
    }

    #[test]
    fn metrics_handle_empty_results_without_panic() {
        let report = SearchModeReport {
            mode: MODE_HYBRID.to_string(),
            hit_at_1: hit_at(&[], 1),
            hit_at_3: hit_at(&[], 3),
            hit_at_5: hit_at(&[], 5),
            hit_at_10: hit_at(&[], 10),
            mrr: 0.0,
            average_final_score: 0.0,
            average_latency_ms: average(&[]),
            p50_latency_ms: percentile(Vec::new(), 50.0),
            p95_latency_ms: percentile(Vec::new(), 95.0),
            result_count: 0,
            fallback_count: 0,
            no_vector_fallback_count: 0,
            source_kind_match_rate: 0.0,
            file_match_rate: 0.0,
            symbol_match_rate: 0.0,
            cases: Vec::new(),
        };
        assert_eq!(report.hit_at_1, 0.0);
        assert_eq!(report.p95_latency_ms, 0.0);
    }

    #[test]
    fn semantic_quality_benchmark_runs_offline() {
        let fixtures = load_fixtures(&default_fixture_root()).expect("fixtures");
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == "semantic_search_repo")
            .expect("semantic fixture");
        let (report, results) = run_semantic_quality_benchmark(fixture).expect("quality");

        assert_eq!(report.query_count, quality_cases().len());
        assert_eq!(report.local_only, true);
        assert_eq!(report.external_api_required, false);
        assert!(report.document_count > 0);
        assert!(report.vector_count > 0);
        assert_eq!(report.search_modes.len(), 3);
        assert!(results
            .iter()
            .any(|result| result.name == "mcp_semantic_search_latency"));
    }

    #[test]
    fn control_mcp_guardrails_match_phase_10_4_counts() {
        let checks = verify_control_mcp().expect("checks");
        assert_eq!(checks.control_hybrid_endpoint_status, 200);
        assert!(checks.semantic_search_tool_present);
        assert_eq!(checks.tiny_count, 5);
        assert_eq!(checks.optimized_count, 8);
        assert_eq!(checks.full_count, 12);
        assert_eq!(checks.debug_count, 12);
        assert_eq!(checks.readonly_count, 12);
        assert_eq!(checks.editing_count, 8);
        assert_eq!(checks.web_app_count, 8);
        assert_eq!(checks.enterprise_count, 10);
    }
}
