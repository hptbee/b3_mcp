use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use b3_core::{
    BranchId, ContractError, ContractResult, EmbeddingProvider, EmbeddingVector, FileId,
    FileRecord, IndexStore, IndexedFileRecord, ProjectId, QueryScope, SourceKind, VectorDocument,
    VectorStore,
};
use b3_embeddings::{
    LocalHashEmbeddingProvider, DEFAULT_LOCAL_HASH_DIMENSION, LOCAL_HASH_MODEL_ID,
    LOCAL_HASH_PROVIDER_ID,
};
use b3_indexer::embedding::{ChunkPlanner, ChunkPlannerConfig, ChunkSource};
use b3_mcp_runtime::{registered_tools_for_profile, ToolProfileConfig, ToolProfileName};
use b3_query::hybrid::{HybridSearchEngine, HybridSearchRequest};
use b3_storage::SqliteStorage;
use serde::{Deserialize, Serialize};

use crate::{to_contract_error, BenchmarkFixture, BenchmarkResult, BRANCH_ID, PROJECT_ID};

pub const TOKEN_REDUCTION_MULTIPLIER_TARGET: f64 = 10.0;
pub const TOOL_CALL_REDUCTION_MULTIPLIER_TARGET: f64 = 2.1;
pub const ANSWER_QUALITY_TARGET: f64 = 0.80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EfficiencyBenchmarkReport {
    pub fixture: String,
    pub task_count: usize,
    pub efficiency_targets: EfficiencyTargets,
    pub efficiency_metrics: EfficiencyMetrics,
    pub mode_comparison: Vec<ModeComparison>,
    pub task_results: Vec<TaskEfficiencyResult>,
    pub profile_results: Vec<ProfileEfficiencyResult>,
    pub control_mcp: EfficiencyControlMcpChecks,
    pub warnings: Vec<String>,
    pub limitations: Vec<String>,
    pub local_only: bool,
    pub external_api_required: bool,
    pub telemetry_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EfficiencyTargets {
    pub token_reduction_multiplier_target: f64,
    pub tool_call_reduction_multiplier_target: f64,
    pub answer_quality_target: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EfficiencyMetrics {
    pub token_reduction_multiplier: f64,
    pub tool_call_reduction_multiplier: f64,
    pub answer_quality_score: f64,
    pub token_target_comparison: TargetComparison,
    pub tool_call_target_comparison: TargetComparison,
    pub answer_quality_target_comparison: TargetComparison,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetComparison {
    pub current_value: f64,
    pub target_value: f64,
    pub target_met: bool,
    pub gap_to_target: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModeComparison {
    pub mode: String,
    pub selected_context_chars: usize,
    pub estimated_tokens: usize,
    pub files_touched: usize,
    pub tool_calls: usize,
    pub answer_quality_score: f64,
    pub token_reduction_percent: f64,
    pub token_reduction_multiplier: f64,
    pub tool_call_reduction_percent: f64,
    pub tool_call_reduction_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEfficiencyResult {
    pub task_id: String,
    pub question: String,
    pub expected_files: Vec<String>,
    pub expected_symbols: Vec<String>,
    pub expected_source_kinds: Vec<String>,
    pub expected_facts: Vec<String>,
    pub required_coverage_tags: Vec<String>,
    pub mode_results: Vec<WorkflowResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileEfficiencyResult {
    pub profile: String,
    pub mode: String,
    pub selected_context_chars: usize,
    pub estimated_tokens: usize,
    pub files_touched: usize,
    pub tool_calls: usize,
    pub answer_quality_score: f64,
    pub token_reduction_percent: f64,
    pub token_reduction_multiplier: f64,
    pub tool_call_reduction_percent: f64,
    pub tool_call_reduction_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub mode: String,
    pub profile: Option<String>,
    pub selected_context_chars: usize,
    pub estimated_tokens: usize,
    pub files_touched: usize,
    pub tool_calls: usize,
    pub answer_quality_score: f64,
    pub matched_expected_files: usize,
    pub matched_expected_symbols: usize,
    pub matched_source_kinds: usize,
    pub matched_expected_facts: usize,
    pub matched_coverage_tags: usize,
    pub top_k_relevance: f64,
    pub baseline_context_chars: usize,
    pub baseline_tool_calls: usize,
    pub token_reduction_percent: f64,
    pub token_reduction_multiplier: f64,
    pub tool_call_reduction_percent: f64,
    pub tool_call_reduction_multiplier: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EfficiencyControlMcpChecks {
    pub semantic_search_tool_present: bool,
    pub context_pack_tool_present: bool,
    pub tiny_count: usize,
    pub optimized_count: usize,
    pub full_count: usize,
    pub debug_count: usize,
    pub readonly_count: usize,
    pub editing_count: usize,
    pub web_app_count: usize,
    pub enterprise_count: usize,
}

#[derive(Debug, Clone)]
struct EfficiencyTask {
    id: &'static str,
    question: &'static str,
    expected_files: &'static [&'static str],
    expected_symbols: &'static [&'static str],
    expected_source_kinds: &'static [SourceKind],
    expected_facts: &'static [&'static str],
    required_coverage_tags: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextProfile {
    Minimal,
    Balanced,
    Deep,
}

impl ContextProfile {
    fn name(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Balanced => "balanced",
            Self::Deep => "deep",
        }
    }

    fn top_k(self) -> usize {
        match self {
            Self::Minimal => 3,
            Self::Balanced => 6,
            Self::Deep => 12,
        }
    }

    fn max_snippet_chars(self) -> usize {
        match self {
            Self::Minimal => 220,
            Self::Balanced => 420,
            Self::Deep => 800,
        }
    }
}

#[derive(Debug, Clone)]
struct SelectedContext {
    path: String,
    text: String,
    source_kind: SourceKind,
}

struct EfficiencyFixtureState {
    storage: SqliteStorage,
    files: BTreeMap<String, FixtureFile>,
    document_count: usize,
    vector_count: usize,
}

#[derive(Debug, Clone)]
struct FixtureFile {
    content: String,
    source_kind: SourceKind,
    language: Option<String>,
    framework: Option<String>,
}

pub fn run_efficiency_benchmark(
    fixture: &BenchmarkFixture,
) -> ContractResult<(EfficiencyBenchmarkReport, Vec<BenchmarkResult>)> {
    let tasks = efficiency_tasks();
    validate_tasks(&tasks, &fixture.path)?;
    let state = seed_efficiency_fixture(fixture)?;
    let mut task_results = Vec::new();

    for task in &tasks {
        let naive = naive_file_by_file(task, &state);
        let mut mode_results = vec![finalize_workflow(task, &naive, &naive)];
        let lexical = search_code_only(task, &state, ContextProfile::Balanced);
        mode_results.push(finalize_workflow(task, &lexical, &naive));
        let semantic = semantic_search_only(task, &state, ContextProfile::Balanced)?;
        mode_results.push(finalize_workflow(task, &semantic, &naive));
        for profile in [
            ContextProfile::Minimal,
            ContextProfile::Balanced,
            ContextProfile::Deep,
        ] {
            let context_pack = semantic_search_context_pack(task, &state, profile)?;
            mode_results.push(finalize_workflow(task, &context_pack, &naive));
        }
        let group = group_federated_summary(task, &state);
        mode_results.push(finalize_workflow(task, &group, &naive));
        task_results.push(TaskEfficiencyResult {
            task_id: task.id.to_string(),
            question: task.question.to_string(),
            expected_files: task
                .expected_files
                .iter()
                .map(|file| file.to_string())
                .collect(),
            expected_symbols: task
                .expected_symbols
                .iter()
                .map(|symbol| symbol.to_string())
                .collect(),
            expected_source_kinds: task
                .expected_source_kinds
                .iter()
                .map(|kind| kind.as_str().to_string())
                .collect(),
            expected_facts: task
                .expected_facts
                .iter()
                .map(|fact| fact.to_string())
                .collect(),
            required_coverage_tags: task
                .required_coverage_tags
                .iter()
                .map(|tag| tag.to_string())
                .collect(),
            mode_results,
        });
    }

    let mode_comparison = aggregate_modes(&task_results);
    let profile_results = aggregate_profiles(&task_results);
    let chosen = mode_comparison
        .iter()
        .find(|mode| mode.mode == "semantic_search_context_pack:balanced")
        .or_else(|| {
            mode_comparison
                .iter()
                .find(|mode| mode.mode == "semantic_search_only")
        })
        .ok_or_else(|| ContractError::new("missing efficiency comparison mode"))?;
    let efficiency_metrics = EfficiencyMetrics {
        token_reduction_multiplier: chosen.token_reduction_multiplier,
        tool_call_reduction_multiplier: chosen.tool_call_reduction_multiplier,
        answer_quality_score: chosen.answer_quality_score,
        token_target_comparison: compare_target(
            chosen.token_reduction_multiplier,
            TOKEN_REDUCTION_MULTIPLIER_TARGET,
        ),
        tool_call_target_comparison: compare_target(
            chosen.tool_call_reduction_multiplier,
            TOOL_CALL_REDUCTION_MULTIPLIER_TARGET,
        ),
        answer_quality_target_comparison: compare_target(
            chosen.answer_quality_score,
            ANSWER_QUALITY_TARGET,
        ),
    };
    let warnings = efficiency_warnings(&efficiency_metrics);
    let benchmark_results = vec![benchmark_result_from_mode(
        "efficiency_semantic_search_context_pack_balanced",
        chosen,
    )];
    let report = EfficiencyBenchmarkReport {
        fixture: fixture.name.clone(),
        task_count: tasks.len(),
        efficiency_targets: EfficiencyTargets {
            token_reduction_multiplier_target: TOKEN_REDUCTION_MULTIPLIER_TARGET,
            tool_call_reduction_multiplier_target: TOOL_CALL_REDUCTION_MULTIPLIER_TARGET,
            answer_quality_target: ANSWER_QUALITY_TARGET,
        },
        efficiency_metrics,
        mode_comparison,
        task_results,
        profile_results,
        control_mcp: verify_efficiency_control_mcp(),
        warnings,
        limitations: vec![
            "token estimates use chars/4 and are not tokenizer or billing accurate".to_string(),
            "tool-call counts are deterministic workflow-model estimates, not live agent traces"
                .to_string(),
            "answer quality is fixture coverage over expected files, symbols, source kinds, facts, and tags, not human grading".to_string(),
            "group_federated_summary models Phase 11.1 summary efficiency only; it does not perform cross-project matching".to_string(),
        ],
        local_only: true,
        external_api_required: false,
        telemetry_enabled: false,
    };
    debug_assert!(state.document_count > 0);
    debug_assert!(state.vector_count > 0);
    Ok((report, benchmark_results))
}

pub fn format_efficiency_summary(report: &EfficiencyBenchmarkReport) -> String {
    let mut lines = vec![
        String::new(),
        "efficiency mode | tokens | token x | tool calls | tool x | quality | target gaps"
            .to_string(),
        "--- | ---: | ---: | ---: | ---: | ---: | ---".to_string(),
    ];
    for mode in &report.mode_comparison {
        lines.push(format!(
            "{} | {} | {:.2} | {} | {:.2} | {:.3} | token {:.2}, tools {:.2}",
            mode.mode,
            mode.estimated_tokens,
            mode.token_reduction_multiplier,
            mode.tool_calls,
            mode.tool_call_reduction_multiplier,
            mode.answer_quality_score,
            (TOKEN_REDUCTION_MULTIPLIER_TARGET - mode.token_reduction_multiplier).max(0.0),
            (TOOL_CALL_REDUCTION_MULTIPLIER_TARGET - mode.tool_call_reduction_multiplier).max(0.0)
        ));
    }
    lines.push(format!(
        "selected baseline: semantic_search_context_pack:balanced token_x={:.2}/{} tool_x={:.2}/{} quality={:.3}/{}",
        report.efficiency_metrics.token_reduction_multiplier,
        TOKEN_REDUCTION_MULTIPLIER_TARGET,
        report.efficiency_metrics.tool_call_reduction_multiplier,
        TOOL_CALL_REDUCTION_MULTIPLIER_TARGET,
        report.efficiency_metrics.answer_quality_score,
        ANSWER_QUALITY_TARGET
    ));
    lines.push(
        "efficiency note: deterministic local simulation; no LLM, telemetry, network, hosted vector DB, or cloud API"
            .to_string(),
    );
    lines.join("\n")
}

fn efficiency_tasks() -> Vec<EfficiencyTask> {
    vec![
        EfficiencyTask {
            id: "order_creation_flow",
            question: "find order creation flow",
            expected_files: &["rust/orders.rs", "next/app/api/orders/route.ts"],
            expected_symbols: &["create_order", "POST"],
            expected_source_kinds: &[SourceKind::SymbolChunk, SourceKind::RouteChunk],
            expected_facts: &["order", "create"],
            required_coverage_tags: &["order", "route"],
        },
        EfficiencyTask {
            id: "user_create_route",
            question: "find API route that creates users",
            expected_files: &["web/users.controller.ts"],
            expected_symbols: &["createUser"],
            expected_source_kinds: &[SourceKind::RouteChunk],
            expected_facts: &["users", "create"],
            required_coverage_tags: &["api", "user"],
        },
        EfficiencyTask {
            id: "payment_created_producer",
            question: "find payment.created producer",
            expected_files: &["messaging/payments.ts"],
            expected_symbols: &["publishPaymentCreated"],
            expected_source_kinds: &[SourceKind::MessagingChunk],
            expected_facts: &["payment.created"],
            required_coverage_tags: &["messaging", "producer"],
        },
        EfficiencyTask {
            id: "order_created_consumer",
            question: "find order.created consumer",
            expected_files: &["messaging/payments.ts"],
            expected_symbols: &["consumeOrderCreated"],
            expected_source_kinds: &[SourceKind::MessagingChunk],
            expected_facts: &["order.created"],
            required_coverage_tags: &["messaging", "consumer"],
        },
        EfficiencyTask {
            id: "wpf_save_command",
            question: "find WPF SaveCommand binding",
            expected_files: &["wpf/MainWindow.xaml"],
            expected_symbols: &[],
            expected_source_kinds: &[SourceKind::WpfChunk],
            expected_facts: &["SaveCommand"],
            required_coverage_tags: &["wpf", "binding"],
        },
        EfficiencyTask {
            id: "kubernetes_api_deployment",
            question: "find Kubernetes deployment for api",
            expected_files: &["infra/deployment.yaml"],
            expected_symbols: &[],
            expected_source_kinds: &[SourceKind::InfrastructureChunk],
            expected_facts: &["Deployment", "api"],
            required_coverage_tags: &["kubernetes", "deployment"],
        },
        EfficiencyTask {
            id: "users_database_query",
            question: "find database query for users",
            expected_files: &["dotnet/UserRepository.cs"],
            expected_symbols: &["FindUserByEmail"],
            expected_source_kinds: &[SourceKind::DataAccessChunk],
            expected_facts: &["SELECT", "Users"],
            required_coverage_tags: &["database", "users"],
        },
        EfficiencyTask {
            id: "websocket_notification",
            question: "find websocket/signalr notification",
            expected_files: &["realtime/notifications.ts"],
            expected_symbols: &["sendWebSocketNotification"],
            expected_source_kinds: &[SourceKind::RealtimeChunk],
            expected_facts: &["send", "notification"],
            required_coverage_tags: &["realtime", "notification"],
        },
        EfficiencyTask {
            id: "go_health_endpoint",
            question: "find Go health endpoint",
            expected_files: &["go/main.go"],
            expected_symbols: &["healthHandler"],
            expected_source_kinds: &[SourceKind::GoChunk],
            expected_facts: &["health", "http"],
            required_coverage_tags: &["go", "endpoint"],
        },
        EfficiencyTask {
            id: "group_summary",
            question: "summarize local group metadata readiness",
            expected_files: &[
                "web/users.controller.ts",
                "messaging/payments.ts",
                "infra/deployment.yaml",
            ],
            expected_symbols: &["createUser", "publishPaymentCreated"],
            expected_source_kinds: &[
                SourceKind::RouteChunk,
                SourceKind::MessagingChunk,
                SourceKind::InfrastructureChunk,
            ],
            expected_facts: &["route", "messaging", "deployment"],
            required_coverage_tags: &["group", "summary"],
        },
    ]
}

fn validate_tasks(tasks: &[EfficiencyTask], root: &Path) -> ContractResult<()> {
    for task in tasks {
        if task.id.trim().is_empty() || task.question.trim().is_empty() {
            return Err(ContractError::new("efficiency tasks need id and question"));
        }
        if task.expected_files.is_empty() {
            return Err(ContractError::new(format!(
                "efficiency task has no expected files: {}",
                task.id
            )));
        }
        for file in task.expected_files {
            if !root.join(file).exists() {
                return Err(ContractError::new(format!(
                    "missing expected efficiency fixture target: {file}"
                )));
            }
        }
    }
    Ok(())
}

fn seed_efficiency_fixture(fixture: &BenchmarkFixture) -> ContractResult<EfficiencyFixtureState> {
    let db = fixture.path.join(".b3").join("efficiency.db");
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
    let mut files = BTreeMap::new();

    for file_path in fixture_files(&fixture.path)? {
        let relative = normalize_relative(&fixture.path, &file_path)?;
        let content = fs::read_to_string(&file_path).map_err(to_contract_error)?;
        let source_kind = source_kind_for_path(&relative);
        let language = language_for_path(&relative);
        let framework = framework_for_path(&relative, &content);
        let file_id = FileId::new(format!("efficiency:{relative}"));
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
            language: language.clone(),
            framework: framework.clone(),
            source_kind,
            path: relative.clone(),
            content_hash: file.content_hash,
            text: content.clone(),
            start_line: 1,
            metadata: BTreeMap::from([
                ("benchmark_fixture".to_string(), "efficiency".to_string()),
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
        files.insert(
            relative,
            FixtureFile {
                content,
                source_kind,
                language,
                framework,
            },
        );
    }
    storage.upsert_documents(&documents)?;
    storage.upsert_vectors(&vectors)?;
    Ok(EfficiencyFixtureState {
        storage,
        files,
        document_count: documents.len(),
        vector_count: vectors.len(),
    })
}

fn naive_file_by_file(task: &EfficiencyTask, state: &EfficiencyFixtureState) -> ModeSelection {
    let mut selected = ranked_files(task.question, state)
        .into_iter()
        .take(6)
        .collect::<Vec<_>>();
    for expected in task.expected_files {
        if !selected.iter().any(|path| path.ends_with(expected)) {
            selected.push((*expected).to_string());
        }
    }
    selected.sort();
    selected.dedup();
    let contexts = selected
        .into_iter()
        .filter_map(|path| {
            let file = state.files.get(&path)?;
            Some(SelectedContext {
                path,
                text: file.content.clone(),
                source_kind: file.source_kind,
            })
        })
        .collect::<Vec<_>>();
    ModeSelection {
        mode: "naive_file_by_file".to_string(),
        profile: None,
        tool_calls: 2 + contexts.len(),
        contexts,
    }
}

fn search_code_only(
    task: &EfficiencyTask,
    state: &EfficiencyFixtureState,
    profile: ContextProfile,
) -> ModeSelection {
    let contexts = ranked_files(task.question, state)
        .into_iter()
        .take(profile.top_k())
        .filter_map(|path| {
            let file = state.files.get(&path)?;
            Some(SelectedContext {
                path,
                text: best_snippet(&file.content, task.question, profile.max_snippet_chars()),
                source_kind: file.source_kind,
            })
        })
        .collect::<Vec<_>>();
    ModeSelection {
        mode: "search_code_only".to_string(),
        profile: None,
        tool_calls: 1,
        contexts,
    }
}

fn semantic_search_only(
    task: &EfficiencyTask,
    state: &EfficiencyFixtureState,
    profile: ContextProfile,
) -> ContractResult<ModeSelection> {
    let results = hybrid_results(task, state, profile)?;
    let contexts = results
        .into_iter()
        .map(|result| SelectedContext {
            path: result.path,
            text: clamp_chars(&result.text_preview, profile.max_snippet_chars()),
            source_kind: result.source_kind,
        })
        .collect::<Vec<_>>();
    Ok(ModeSelection {
        mode: "semantic_search_only".to_string(),
        profile: None,
        tool_calls: 1,
        contexts,
    })
}

fn semantic_search_context_pack(
    task: &EfficiencyTask,
    state: &EfficiencyFixtureState,
    profile: ContextProfile,
) -> ContractResult<ModeSelection> {
    let mut seen = BTreeSet::new();
    let mut contexts = Vec::new();
    for result in hybrid_results(task, state, profile)? {
        let dedupe_key = format!("{}:{}", result.path, result.text_preview);
        if seen.insert(dedupe_key) {
            contexts.push(SelectedContext {
                path: result.path,
                text: clamp_chars(&result.text_preview, profile.max_snippet_chars()),
                source_kind: result.source_kind,
            });
        }
    }
    contexts.sort_by(|left, right| {
        source_kind_priority(left.source_kind)
            .cmp(&source_kind_priority(right.source_kind))
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(ModeSelection {
        mode: format!("semantic_search_context_pack:{}", profile.name()),
        profile: Some(profile.name().to_string()),
        tool_calls: 2,
        contexts,
    })
}

fn group_federated_summary(task: &EfficiencyTask, state: &EfficiencyFixtureState) -> ModeSelection {
    let mut by_kind = BTreeMap::<String, Vec<String>>::new();
    for (path, file) in &state.files {
        if task
            .expected_source_kinds
            .iter()
            .any(|kind| *kind == file.source_kind)
        {
            by_kind
                .entry(file.source_kind.as_str().to_string())
                .or_default()
                .push(path.clone());
        }
    }
    let text = by_kind
        .iter()
        .map(|(kind, paths)| format!("{}: {}", kind.as_str(), paths.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    ModeSelection {
        mode: "group_federated_summary".to_string(),
        profile: None,
        tool_calls: 1,
        contexts: vec![SelectedContext {
            path: "registry://local-group-summary".to_string(),
            text,
            source_kind: SourceKind::SymbolChunk,
        }],
    }
}

fn hybrid_results(
    task: &EfficiencyTask,
    state: &EfficiencyFixtureState,
    profile: ContextProfile,
) -> ContractResult<Vec<b3_query::hybrid::HybridSearchResult>> {
    let engine = HybridSearchEngine::new(&state.storage, &state.storage);
    let mut request = HybridSearchRequest::new(bench_scope(), task.question);
    request.limit = profile.top_k();
    request.explain = true;
    Ok(engine.search(request)?.results)
}

#[derive(Debug, Clone)]
struct ModeSelection {
    mode: String,
    profile: Option<String>,
    tool_calls: usize,
    contexts: Vec<SelectedContext>,
}

fn finalize_workflow(
    task: &EfficiencyTask,
    selection: &ModeSelection,
    naive: &ModeSelection,
) -> WorkflowResult {
    let selected_context_chars = selection.contexts.iter().map(|ctx| ctx.text.len()).sum();
    let naive_context_chars = naive.contexts.iter().map(|ctx| ctx.text.len()).sum();
    let files_touched = selection
        .contexts
        .iter()
        .map(|ctx| ctx.path.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let matched_expected_files = task
        .expected_files
        .iter()
        .filter(|expected| {
            selection
                .contexts
                .iter()
                .any(|ctx| ctx.path.ends_with(**expected) || ctx.text.contains(**expected))
        })
        .count();
    let haystack = selection
        .contexts
        .iter()
        .map(|ctx| ctx.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    let matched_expected_symbols = task
        .expected_symbols
        .iter()
        .filter(|symbol| haystack.contains(&symbol.to_lowercase()))
        .count();
    let matched_source_kinds = task
        .expected_source_kinds
        .iter()
        .filter(|kind| {
            selection
                .contexts
                .iter()
                .any(|ctx| ctx.source_kind == **kind)
        })
        .count();
    let matched_expected_facts = task
        .expected_facts
        .iter()
        .filter(|fact| haystack.contains(&fact.to_lowercase()))
        .count();
    let matched_coverage_tags = task
        .required_coverage_tags
        .iter()
        .filter(|tag| haystack.contains(&tag.to_lowercase()) || selection.mode.contains(**tag))
        .count();
    let top_k_relevance = if selection.contexts.first().is_some_and(|ctx| {
        task.expected_files
            .iter()
            .any(|file| ctx.path.ends_with(file))
    }) {
        1.0
    } else if matched_expected_files > 0 {
        0.5
    } else {
        0.0
    };
    let answer_quality_score = quality_score(
        task,
        matched_expected_files,
        matched_expected_symbols,
        matched_source_kinds,
        matched_expected_facts,
        matched_coverage_tags,
        top_k_relevance,
    );
    WorkflowResult {
        mode: selection.mode.clone(),
        profile: selection.profile.clone(),
        selected_context_chars,
        estimated_tokens: estimate_tokens(selected_context_chars),
        files_touched,
        tool_calls: selection.tool_calls,
        answer_quality_score,
        matched_expected_files,
        matched_expected_symbols,
        matched_source_kinds,
        matched_expected_facts,
        matched_coverage_tags,
        top_k_relevance,
        baseline_context_chars: naive_context_chars,
        baseline_tool_calls: naive.tool_calls,
        token_reduction_percent: reduction_percent(naive_context_chars, selected_context_chars),
        token_reduction_multiplier: reduction_multiplier(
            naive_context_chars,
            selected_context_chars,
        ),
        tool_call_reduction_percent: reduction_percent(naive.tool_calls, selection.tool_calls),
        tool_call_reduction_multiplier: reduction_multiplier(
            naive.tool_calls,
            selection.tool_calls,
        ),
    }
}

fn quality_score(
    task: &EfficiencyTask,
    file_hits: usize,
    symbol_hits: usize,
    source_kind_hits: usize,
    fact_hits: usize,
    tag_hits: usize,
    top_k_relevance: f64,
) -> f64 {
    let file_score = ratio(file_hits, task.expected_files.len());
    let symbol_score = optional_ratio(symbol_hits, task.expected_symbols.len());
    let kind_score = optional_ratio(source_kind_hits, task.expected_source_kinds.len());
    let fact_score = optional_ratio(fact_hits, task.expected_facts.len());
    let tag_score = optional_ratio(tag_hits, task.required_coverage_tags.len());
    (file_score * 0.30
        + symbol_score * 0.20
        + kind_score * 0.20
        + fact_score * 0.15
        + tag_score * 0.10
        + top_k_relevance * 0.05)
        .clamp(0.0, 1.0)
}

fn aggregate_modes(task_results: &[TaskEfficiencyResult]) -> Vec<ModeComparison> {
    let mut by_mode = BTreeMap::<String, Vec<&WorkflowResult>>::new();
    for task in task_results {
        for result in &task.mode_results {
            by_mode.entry(result.mode.clone()).or_default().push(result);
        }
    }
    by_mode
        .into_iter()
        .map(|(mode, results)| ModeComparison {
            mode,
            selected_context_chars: sum_usize(&results, |result| result.selected_context_chars),
            estimated_tokens: sum_usize(&results, |result| result.estimated_tokens),
            files_touched: sum_usize(&results, |result| result.files_touched),
            tool_calls: sum_usize(&results, |result| result.tool_calls),
            answer_quality_score: avg_f64(&results, |result| result.answer_quality_score),
            token_reduction_percent: reduction_percent(
                sum_usize(&results, |result| result.baseline_context_chars),
                sum_usize(&results, |result| result.selected_context_chars),
            ),
            token_reduction_multiplier: reduction_multiplier(
                sum_usize(&results, |result| result.baseline_context_chars),
                sum_usize(&results, |result| result.selected_context_chars),
            ),
            tool_call_reduction_percent: reduction_percent(
                sum_usize(&results, |result| result.baseline_tool_calls),
                sum_usize(&results, |result| result.tool_calls),
            ),
            tool_call_reduction_multiplier: reduction_multiplier(
                sum_usize(&results, |result| result.baseline_tool_calls),
                sum_usize(&results, |result| result.tool_calls),
            ),
        })
        .collect()
}

fn aggregate_profiles(task_results: &[TaskEfficiencyResult]) -> Vec<ProfileEfficiencyResult> {
    aggregate_modes(task_results)
        .into_iter()
        .filter_map(|mode| {
            mode.mode
                .strip_prefix("semantic_search_context_pack:")
                .map(|profile| ProfileEfficiencyResult {
                    profile: profile.to_string(),
                    mode: mode.mode.clone(),
                    selected_context_chars: mode.selected_context_chars,
                    estimated_tokens: mode.estimated_tokens,
                    files_touched: mode.files_touched,
                    tool_calls: mode.tool_calls,
                    answer_quality_score: mode.answer_quality_score,
                    token_reduction_percent: mode.token_reduction_percent,
                    token_reduction_multiplier: mode.token_reduction_multiplier,
                    tool_call_reduction_percent: mode.tool_call_reduction_percent,
                    tool_call_reduction_multiplier: mode.tool_call_reduction_multiplier,
                })
        })
        .collect()
}

fn benchmark_result_from_mode(name: &str, mode: &ModeComparison) -> BenchmarkResult {
    let mut result = BenchmarkResult::new(name, std::time::Duration::ZERO);
    result.input_size = mode.selected_context_chars;
    result.query_result_count = mode.files_touched;
    result.metadata.insert(
        "estimated_tokens".to_string(),
        mode.estimated_tokens.to_string(),
    );
    result
        .metadata
        .insert("tool_calls".to_string(), mode.tool_calls.to_string());
    result.metadata.insert(
        "token_reduction_multiplier".to_string(),
        format!("{:.3}", mode.token_reduction_multiplier),
    );
    result.metadata.insert(
        "tool_call_reduction_multiplier".to_string(),
        format!("{:.3}", mode.tool_call_reduction_multiplier),
    );
    result.metadata.insert(
        "answer_quality_score".to_string(),
        format!("{:.3}", mode.answer_quality_score),
    );
    result
}

fn efficiency_warnings(metrics: &EfficiencyMetrics) -> Vec<String> {
    let mut warnings = Vec::new();
    if !metrics.token_target_comparison.target_met {
        warnings.push(format!(
            "token reduction target not met: current {:.2}x, target {:.2}x",
            metrics.token_reduction_multiplier, TOKEN_REDUCTION_MULTIPLIER_TARGET
        ));
    }
    if !metrics.tool_call_target_comparison.target_met {
        warnings.push(format!(
            "tool-call reduction target not met: current {:.2}x, target {:.2}x",
            metrics.tool_call_reduction_multiplier, TOOL_CALL_REDUCTION_MULTIPLIER_TARGET
        ));
    }
    if !metrics.answer_quality_target_comparison.target_met {
        warnings.push(format!(
            "answer quality target not met: current {:.3}, target {:.3}",
            metrics.answer_quality_score, ANSWER_QUALITY_TARGET
        ));
    }
    warnings
}

fn compare_target(current_value: f64, target_value: f64) -> TargetComparison {
    TargetComparison {
        current_value,
        target_value,
        target_met: current_value >= target_value,
        gap_to_target: (target_value - current_value).max(0.0),
    }
}

fn verify_efficiency_control_mcp() -> EfficiencyControlMcpChecks {
    let optimized =
        registered_tools_for_profile(&ToolProfileConfig::new(ToolProfileName::Optimized));
    EfficiencyControlMcpChecks {
        semantic_search_tool_present: optimized.iter().any(|tool| tool.name == "semantic_search"),
        context_pack_tool_present: optimized.iter().any(|tool| tool.name == "get_context_pack"),
        tiny_count: tool_count(ToolProfileName::Tiny),
        optimized_count: tool_count(ToolProfileName::Optimized),
        full_count: tool_count(ToolProfileName::Full),
        debug_count: tool_count(ToolProfileName::Debug),
        readonly_count: tool_count(ToolProfileName::Readonly),
        editing_count: tool_count(ToolProfileName::Editing),
        web_app_count: tool_count(ToolProfileName::WebApp),
        enterprise_count: tool_count(ToolProfileName::Enterprise),
    }
}

fn ranked_files(query: &str, state: &EfficiencyFixtureState) -> Vec<String> {
    let query_terms = terms(query);
    let mut scored = state
        .files
        .iter()
        .map(|(path, file)| {
            let path_lower = path.to_lowercase();
            let text = file.content.to_lowercase();
            let mut score = 0usize;
            for term in &query_terms {
                if path_lower.contains(term) {
                    score += 3;
                }
                score += text.matches(term).count();
                if file
                    .language
                    .as_deref()
                    .is_some_and(|language| language.contains(term))
                {
                    score += 1;
                }
                if file
                    .framework
                    .as_deref()
                    .is_some_and(|framework| framework.contains(term))
                {
                    score += 2;
                }
            }
            (path.clone(), score)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.into_iter().map(|(path, _)| path).collect()
}

fn best_snippet(content: &str, query: &str, max_chars: usize) -> String {
    let lowered = content.to_lowercase();
    let first_hit = terms(query)
        .into_iter()
        .filter_map(|term| lowered.find(&term))
        .min()
        .unwrap_or(0);
    let start = first_hit.saturating_sub(max_chars / 4);
    clamp_chars(&content[start..], max_chars)
}

fn terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.')
        .filter(|term| term.len() > 2)
        .map(|term| term.to_lowercase())
        .collect()
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

fn source_kind_priority(kind: SourceKind) -> usize {
    match kind {
        SourceKind::RouteChunk
        | SourceKind::MessagingChunk
        | SourceKind::DataAccessChunk
        | SourceKind::RealtimeChunk
        | SourceKind::InfrastructureChunk
        | SourceKind::WpfChunk
        | SourceKind::GoChunk
        | SourceKind::ComponentChunk => 0,
        _ => 1,
    }
}

fn estimate_tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

fn reduction_percent(baseline: usize, selected: usize) -> f64 {
    if baseline == 0 {
        0.0
    } else {
        1.0 - (selected as f64 / baseline as f64)
    }
}

fn reduction_multiplier(baseline: usize, selected: usize) -> f64 {
    if baseline == 0 || selected == 0 {
        0.0
    } else {
        baseline as f64 / selected as f64
    }
}

fn ratio(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64
    }
}

fn optional_ratio(part: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        ratio(part, total)
    }
}

fn sum_usize(results: &[&WorkflowResult], f: impl Fn(&WorkflowResult) -> usize) -> usize {
    results.iter().map(|result| f(result)).sum()
}

fn avg_f64(results: &[&WorkflowResult], f: impl Fn(&WorkflowResult) -> f64) -> f64 {
    if results.is_empty() {
        0.0
    } else {
        results.iter().map(|result| f(result)).sum::<f64>() / results.len() as f64
    }
}

fn clamp_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn tool_count(profile: ToolProfileName) -> usize {
    registered_tools_for_profile(&ToolProfileConfig::new(profile)).len()
}

fn bench_scope() -> QueryScope {
    QueryScope::new(ProjectId::new(PROJECT_ID), BranchId::new(BRANCH_ID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_fixture_root, load_fixtures};

    #[test]
    fn efficiency_tasks_have_expected_targets() {
        let root = default_fixture_root().join("semantic_search_repo");
        validate_tasks(&efficiency_tasks(), &root).expect("tasks");
        assert!(efficiency_tasks()
            .iter()
            .any(|task| task.id == "group_summary"));
    }

    #[test]
    fn token_estimate_uses_chars_divided_by_four() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 1);
        assert_eq!(estimate_tokens(8), 2);
    }

    #[test]
    fn reduction_math_handles_empty_selected_context() {
        assert_eq!(reduction_multiplier(100, 0), 0.0);
        assert_eq!(reduction_percent(100, 0), 1.0);
        assert_eq!(reduction_multiplier(100, 25), 4.0);
    }

    #[test]
    fn target_comparison_reports_gap() {
        let comparison = compare_target(1.5, 2.1);
        assert!(!comparison.target_met);
        assert!((comparison.gap_to_target - 0.6).abs() < 0.0001);
    }

    #[test]
    fn quality_scoring_is_bounded() {
        let task = &efficiency_tasks()[0];
        let score = quality_score(task, 99, 99, 99, 99, 99, 1.0);
        assert_eq!(score, 1.0);
        let empty = quality_score(task, 0, 0, 0, 0, 0, 0.0);
        assert_eq!(empty, 0.0);
    }

    #[test]
    fn efficiency_benchmark_runs_offline_and_serializes() {
        let fixtures = load_fixtures(&default_fixture_root()).expect("fixtures");
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == "semantic_search_repo")
            .expect("semantic fixture");
        let (report, results) = run_efficiency_benchmark(fixture).expect("efficiency");
        assert_eq!(report.local_only, true);
        assert_eq!(report.external_api_required, false);
        assert_eq!(report.telemetry_enabled, false);
        assert_eq!(report.task_count, efficiency_tasks().len());
        assert!(report
            .mode_comparison
            .iter()
            .any(|mode| mode.mode == "semantic_search_context_pack:balanced"));
        assert!(serde_json::to_string(&report)
            .expect("json")
            .contains("efficiency_metrics"));
        assert!(results
            .iter()
            .any(|result| result.name == "efficiency_semantic_search_context_pack_balanced"));
    }

    #[test]
    fn profile_comparison_includes_minimal_balanced_deep() {
        let task = EfficiencyTask {
            id: "unit",
            question: "find order",
            expected_files: &["a.rs"],
            expected_symbols: &[],
            expected_source_kinds: &[SourceKind::SymbolChunk],
            expected_facts: &["order"],
            required_coverage_tags: &[],
        };
        let naive = ModeSelection {
            mode: "naive_file_by_file".to_string(),
            profile: None,
            tool_calls: 3,
            contexts: vec![SelectedContext {
                path: "a.rs".to_string(),
                text: "order".repeat(100),
                source_kind: SourceKind::SymbolChunk,
            }],
        };
        let selected = ModeSelection {
            mode: "semantic_search_context_pack:minimal".to_string(),
            profile: Some("minimal".to_string()),
            tool_calls: 2,
            contexts: vec![SelectedContext {
                path: "a.rs".to_string(),
                text: "order".to_string(),
                source_kind: SourceKind::SymbolChunk,
            }],
        };
        let result = TaskEfficiencyResult {
            task_id: "unit".to_string(),
            question: "find order".to_string(),
            expected_files: vec!["a.rs".to_string()],
            expected_symbols: Vec::new(),
            expected_source_kinds: vec![SourceKind::SymbolChunk.as_str().to_string()],
            expected_facts: vec!["order".to_string()],
            required_coverage_tags: Vec::new(),
            mode_results: vec![
                finalize_workflow(&task, &naive, &naive),
                finalize_workflow(&task, &selected, &naive),
            ],
        };
        let profiles = aggregate_profiles(&[result]);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].profile, "minimal");
    }

    #[test]
    fn efficiency_control_mcp_guardrails_match_phase_10_4_counts() {
        let checks = verify_efficiency_control_mcp();
        assert!(checks.semantic_search_tool_present);
        assert!(checks.context_pack_tool_present);
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
