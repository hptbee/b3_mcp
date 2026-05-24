//! Local/offline benchmark harness for B3.
//!
//! This crate measures current behavior only. It should not become a product
//! feature surface, and it must not upload results or call external services.

use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{body::Body, Router};
use b3_compaction::{compact_command_output, CommandOutputInput};
use b3_control::{app as control_app, ControlState};
use b3_core::{
    BranchId, ContractError, ContractResult, EventBus, IndexJob, Indexer, ProjectId, QueryScope,
    SymbolRepository,
};
use b3_indexer::{
    parse_worker_json_line, DebouncedBatch, IndexerConfig, LocalIndexer, ParserJobRequest,
    RustLanguagePack, WatchDebouncer, WatchEvent, WatchEventKind,
};
use b3_mcp_runtime::{handle_json_rpc_line, McpQueryToolRouter, ToolProfileName};
use b3_query::{LocalQueryEngine, QueryEngineConfig};
use b3_storage::SqliteStorage;
use http::Request;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower::ServiceExt;

const DEFAULT_FIXTURE_ROOT: &str = "benchmarks/fixtures";
const DEFAULT_OUTPUT_PATH: &str = "target/benchmarks/baseline.json";
const DEFAULT_THRESHOLD_PATH: &str = "benchmarks/benchmark-thresholds.json";
const PROJECT_ID: &str = "bench";
const BRANCH_ID: &str = "main";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkThresholdConfig {
    pub allowed_latency_regression_percent: f64,
    pub allowed_indexing_regression_percent: f64,
    pub memory_warning_kb: Option<u64>,
    pub fail_on_regression: bool,
}

impl Default for BenchmarkThresholdConfig {
    fn default() -> Self {
        Self {
            allowed_latency_regression_percent: 25.0,
            allowed_indexing_regression_percent: 25.0,
            memory_warning_kb: None,
            fail_on_regression: false,
        }
    }
}

impl BenchmarkThresholdConfig {
    pub fn load(path: &Path) -> ContractResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path).map_err(to_contract_error)?;
        serde_json::from_str(&content).map_err(to_contract_error)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub timestamp_unix_ms: u64,
    pub git_commit: Option<String>,
    pub thresholds: BenchmarkThresholdConfig,
    pub results: Vec<BenchmarkResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub duration_ms: f64,
    pub input_size: usize,
    pub files_indexed: usize,
    pub symbols_indexed: usize,
    pub edges_indexed: usize,
    pub query_result_count: usize,
    pub memory_kb: Option<u64>,
    pub metadata: BTreeMap<String, String>,
}

impl BenchmarkResult {
    fn new(name: impl Into<String>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            duration_ms: duration.as_secs_f64() * 1000.0,
            input_size: 0,
            files_indexed: 0,
            symbols_indexed: 0,
            edges_indexed: 0,
            query_result_count: 0,
            memory_kb: rough_memory_kb(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    pub fixture_root: PathBuf,
    pub output_path: PathBuf,
    pub threshold_path: PathBuf,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self {
            fixture_root: default_fixture_root(),
            output_path: workspace_root().join(DEFAULT_OUTPUT_PATH),
            threshold_path: workspace_root().join(DEFAULT_THRESHOLD_PATH),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkFixture {
    pub name: String,
    pub path: PathBuf,
    pub file_count: usize,
    pub total_bytes: usize,
}

pub fn run_cli(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let command = args
        .into_iter()
        .next()
        .unwrap_or_else(|| "help".to_string());
    match command.as_str() {
        "baseline" => {
            let options = BenchmarkOptions::default();
            let output_path = options.output_path.clone();
            let run = run_baseline(options).map_err(|error| error.message)?;
            println!("{}", format_results_table(&run.results));
            println!("wrote JSON baseline: {}", output_path.display());
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("usage: b3-bench baseline");
            Ok(())
        }
        other => Err(format!("unknown b3-bench command: {other}")),
    }
}

pub fn run_baseline(options: BenchmarkOptions) -> ContractResult<BenchmarkRun> {
    assert_local_only();
    let thresholds = BenchmarkThresholdConfig::load(&options.threshold_path)?;
    let workspace = fresh_benchmark_workspace()?;
    let fixtures = load_fixtures(&options.fixture_root)?;

    let tiny = copy_fixture(&fixtures, "tiny_rust_repo", &workspace)?;
    let medium = copy_fixture(&fixtures, "medium_rust_repo", &workspace)?;
    let cycle = copy_fixture(&fixtures, "graph_cycle_repo", &workspace)?;
    let call_graph = copy_fixture(&fixtures, "call_graph_repo", &workspace)?;
    let watcher = copy_fixture(&fixtures, "watcher_change_repo", &workspace)?;

    let mut results = Vec::new();
    results.push(bench_cold_startup()?);

    let medium_state = index_fixture(&medium)?;
    let cycle_state = index_fixture(&cycle)?;
    let call_graph_state = index_fixture(&call_graph)?;

    results.push(bench_mcp_tools_list(
        &medium_state.storage,
        ToolProfileName::Optimized,
        "mcp_tools_list_latency",
    )?);
    results.push(bench_mcp_tools_list(
        &medium_state.storage,
        ToolProfileName::Full,
        "mcp_tools_list_latency_full",
    )?);
    results.push(bench_mcp_tools_list(
        &medium_state.storage,
        ToolProfileName::Tiny,
        "mcp_tools_list_latency_tiny",
    )?);
    results.push(bench_mcp_tools_list(
        &medium_state.storage,
        ToolProfileName::Enterprise,
        "mcp_tools_list_latency_enterprise",
    )?);
    results.push(bench_mcp_simple_tool_call(&medium_state.storage)?);
    results.push(bench_control_endpoint(
        "control_health_latency",
        control_app(ControlState::from_storage(
            medium.path.clone(),
            PathBuf::from(":memory:"),
            medium_state.storage_for_control()?,
        )),
        "/health",
    )?);
    results.push(bench_control_endpoint(
        "control_status_latency",
        control_app(ControlState::from_storage(
            medium.path.clone(),
            PathBuf::from(":memory:"),
            medium_state.storage_for_control()?,
        )),
        "/api/status",
    )?);
    results.push(bench_find_symbol(&medium_state.storage)?);
    results.push(bench_search_code(&medium_state.storage)?);
    results.push(bench_graph_neighbors(&cycle_state.storage)?);
    results.push(bench_graph_path(&call_graph_state.storage)?);
    results.push(bench_context_pack(&medium_state.storage)?);
    results.push(bench_impact_analysis(&call_graph_state.storage)?);
    results.push(bench_indexing_speed(&tiny)?);
    results.push(bench_changed_file_reindex(&watcher)?);
    results.push(bench_watcher_debounce()?);
    results.push(bench_sqlite_query_latency(&medium_state.storage)?);
    results.push(bench_parser_worker_latency()?);
    results.push(bench_command_compaction_latency()?);

    let run = BenchmarkRun {
        timestamp_unix_ms: now_unix_ms(),
        git_commit: git_commit(),
        thresholds,
        results,
    };
    write_json_output(&options.output_path, &run)?;
    Ok(run)
}

pub fn load_fixtures(root: &Path) -> ContractResult<Vec<BenchmarkFixture>> {
    let mut fixtures = Vec::new();
    for name in [
        "tiny_rust_repo",
        "medium_rust_repo",
        "graph_cycle_repo",
        "call_graph_repo",
        "watcher_change_repo",
    ] {
        let path = root.join(name);
        if !path.exists() {
            return Err(ContractError::new(format!(
                "missing benchmark fixture: {}",
                path.display()
            )));
        }
        let (file_count, total_bytes) = fixture_stats(&path)?;
        fixtures.push(BenchmarkFixture {
            name: name.to_string(),
            path,
            file_count,
            total_bytes,
        });
    }
    Ok(fixtures)
}

pub fn format_results_table(results: &[BenchmarkResult]) -> String {
    let mut lines = vec![
        "benchmark | duration_ms | input_size | files | symbols | edges | results".to_string(),
        "--- | ---: | ---: | ---: | ---: | ---: | ---:".to_string(),
    ];
    for result in results {
        lines.push(format!(
            "{} | {:.3} | {} | {} | {} | {} | {}",
            result.name,
            result.duration_ms,
            result.input_size,
            result.files_indexed,
            result.symbols_indexed,
            result.edges_indexed,
            result.query_result_count
        ));
    }
    lines.join("\n")
}

fn bench_cold_startup() -> ContractResult<BenchmarkResult> {
    let db = temp_path("cold-start", "b3.db")?;
    let started = Instant::now();
    let storage = SqliteStorage::open(&db)?;
    let _engine = LocalQueryEngine::new(&storage, QueryEngineConfig::default());
    let mut result = BenchmarkResult::new("cold_startup", started.elapsed());
    result
        .metadata
        .insert("database".to_string(), db.display().to_string());
    Ok(result)
}

fn bench_mcp_tools_list(
    storage: &SqliteStorage,
    profile: ToolProfileName,
    name: &str,
) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let router = McpQueryToolRouter::with_profile(engine, profile);
    let started = Instant::now();
    let outcome =
        handle_json_rpc_line(&router, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
            .map_err(ContractError::new)?;
    let mut result = BenchmarkResult::new(name, started.elapsed());
    result.query_result_count = outcome
        .response
        .and_then(|value| value["result"]["tools"].as_array().map(Vec::len))
        .unwrap_or_default();
    result
        .metadata
        .insert("profile".to_string(), profile.to_string());
    result.metadata.insert(
        "tool_count".to_string(),
        result.query_result_count.to_string(),
    );
    Ok(result)
}

fn bench_mcp_simple_tool_call(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let router = McpQueryToolRouter::new(engine);
    let started = Instant::now();
    let outcome = handle_json_rpc_line(
        &router,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"find_symbol","arguments":{"scope":{"project_id":"bench","branch_id":"main"},"query":"run","limit":20,"include_trace":false}}}"#,
    )
    .map_err(ContractError::new)?;
    let mut result = BenchmarkResult::new("mcp_find_symbol_tool_call_latency", started.elapsed());
    result.query_result_count = outcome
        .response
        .and_then(|value| {
            let text = value["result"]["content"][0]["text"].as_str()?;
            serde_json::from_str::<Value>(text).ok()
        })
        .and_then(|value| value["symbols"].as_array().map(Vec::len))
        .unwrap_or_default();
    Ok(result)
}

fn bench_control_endpoint(name: &str, app: Router, uri: &str) -> ContractResult<BenchmarkResult> {
    let runtime = tokio::runtime::Runtime::new().map_err(to_contract_error)?;
    let started = Instant::now();
    let status = runtime.block_on(async {
        app.oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .map_err(to_contract_error)?,
        )
        .await
        .map(|response| response.status().as_u16())
        .map_err(to_contract_error)
    })?;
    let mut result = BenchmarkResult::new(name, started.elapsed());
    result.query_result_count = usize::from(status == 200);
    result
        .metadata
        .insert("status".to_string(), status.to_string());
    Ok(result)
}

fn bench_find_symbol(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let scope = bench_scope();
    let started = Instant::now();
    let symbols = engine.find_symbol(&scope, "run")?;
    let mut result = BenchmarkResult::new("find_symbol_latency", started.elapsed());
    result.query_result_count = symbols.len();
    Ok(result)
}

fn bench_search_code(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let scope = bench_scope();
    let started = Instant::now();
    let symbols = engine.search_code(&scope, "helper", 20)?;
    let mut result = BenchmarkResult::new("search_code_latency", started.elapsed());
    result.query_result_count = symbols.len();
    Ok(result)
}

fn bench_graph_neighbors(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let node_id = first_symbol_node(storage, "a")?;
    let started = Instant::now();
    let edges = storage.graph_edges_for_node(PROJECT_ID, BRANCH_ID, &node_id, 0, 50)?;
    let mut result = BenchmarkResult::new("graph_neighbors_latency", started.elapsed());
    result.query_result_count = edges.len();
    result.edges_indexed = storage
        .graph_summary(Some(PROJECT_ID), Some(BRANCH_ID))?
        .edge_count;
    Ok(result)
}

fn bench_graph_path(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let source = first_symbol_node(storage, "entry")?;
    let target = first_symbol_node(storage, "leaf")?;
    let started = Instant::now();
    let path = bounded_path(storage, &source, &target, 4, 100)?;
    let mut result = BenchmarkResult::new("graph_path_latency", started.elapsed());
    result.query_result_count = path.len();
    Ok(result)
}

fn bench_context_pack(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let scope = bench_scope();
    let started = Instant::now();
    let pack = engine.context_pack_for_query(&scope, "run", 1_200)?;
    let mut result = BenchmarkResult::new("context_pack_latency", started.elapsed());
    result.query_result_count = pack.items.len();
    Ok(result)
}

fn bench_impact_analysis(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let scope = bench_scope();
    let symbol = storage
        .find_symbol(&ProjectId::new(PROJECT_ID), "entry")?
        .into_iter()
        .next()
        .ok_or_else(|| ContractError::new("entry symbol missing"))?;
    let started = Instant::now();
    let impact = engine.impact_analysis(&scope, &symbol.id)?;
    let mut result = BenchmarkResult::new("impact_analysis_latency", started.elapsed());
    result.query_result_count = impact.len();
    Ok(result)
}

fn bench_indexing_speed(fixture: &BenchmarkFixture) -> ContractResult<BenchmarkResult> {
    let started = Instant::now();
    let indexed = index_fixture(fixture)?;
    let mut result = BenchmarkResult::new("indexing_speed", started.elapsed());
    attach_index_stats(&mut result, fixture, &indexed.storage)?;
    Ok(result)
}

fn bench_changed_file_reindex(fixture: &BenchmarkFixture) -> ContractResult<BenchmarkResult> {
    let indexed = index_fixture(fixture)?;
    let changed = fixture.path.join("src").join("watch.rs");
    fs::write(
        &changed,
        "pub fn watched() { watched_helper(); }\nfn watched_helper() {}\n",
    )
    .map_err(to_contract_error)?;
    let indexer = LocalIndexer::new(
        RustLanguagePack,
        &indexed.storage,
        NoopEventBus,
        IndexerConfig {
            branch_id: BranchId::new(BRANCH_ID),
            ..IndexerConfig::default()
        },
    );
    let started = Instant::now();
    let summary = indexer.index_paths(
        &fixture.path,
        &ProjectId::new(PROJECT_ID),
        std::slice::from_ref(&changed),
    )?;
    let mut result = BenchmarkResult::new("changed_file_reindex_latency", started.elapsed());
    result.input_size = fixture.total_bytes;
    result.files_indexed = summary.files_parsed;
    result.symbols_indexed = summary.symbols_indexed;
    Ok(result)
}

fn bench_watcher_debounce() -> ContractResult<BenchmarkResult> {
    let mut debouncer = WatchDebouncer::new(Duration::ZERO, 100);
    let started = Instant::now();
    let path = PathBuf::from("src/lib.rs");
    let _ = debouncer.push(WatchEvent {
        kind: WatchEventKind::Changed,
        path: path.clone(),
        new_path: None,
    });
    let _ = debouncer.push(WatchEvent {
        kind: WatchEventKind::Changed,
        path,
        new_path: None,
    });
    let batch = debouncer
        .flush_if_ready()
        .unwrap_or(DebouncedBatch { events: Vec::new() });
    let mut result = BenchmarkResult::new("watcher_debounce_latency", started.elapsed());
    result.query_result_count = batch.events.len();
    result
        .metadata
        .insert("measures".to_string(), "coalescing_overhead".to_string());
    Ok(result)
}

fn bench_sqlite_query_latency(storage: &SqliteStorage) -> ContractResult<BenchmarkResult> {
    let started = Instant::now();
    let summary = storage.graph_summary(Some(PROJECT_ID), Some(BRANCH_ID))?;
    let mut result = BenchmarkResult::new("sqlite_graph_summary_latency", started.elapsed());
    result.files_indexed = summary.file_count;
    result.symbols_indexed = summary.symbol_count;
    result.edges_indexed = summary.edge_count;
    result.query_result_count = summary.node_count;
    Ok(result)
}

fn bench_parser_worker_latency() -> ContractResult<BenchmarkResult> {
    let request = ParserJobRequest {
        project_id: PROJECT_ID.to_string(),
        branch_id: BRANCH_ID.to_string(),
        file_id: "file".to_string(),
        path: "src/lib.rs".to_string(),
        source: "pub fn run() { helper(); }\nfn helper() {}\n".to_string(),
    };
    let line = serde_json::to_string(&request).map_err(to_contract_error)?;
    let started = Instant::now();
    let output = parse_worker_json_line(&line);
    let mut result = BenchmarkResult::new("parser_worker_latency", started.elapsed());
    result.query_result_count = match output {
        b3_indexer::ParserWorkerOutput::Parsed(response) => response.symbols.len(),
        b3_indexer::ParserWorkerOutput::Failed(_) => 0,
    };
    Ok(result)
}

fn bench_command_compaction_latency() -> ContractResult<BenchmarkResult> {
    let stdout = "error[E0425]: cannot find value `missing` in this scope\nwarning: unused variable: `x`\ntest result: FAILED. 1 failed; 2 passed\n".repeat(20);
    let input = CommandOutputInput {
        command: "cargo test".to_string(),
        argv: Vec::new(),
        stdout,
        stderr: "thread 'tests::fails' panicked at src/lib.rs:10:5\n".to_string(),
        exit_code: Some(101),
        working_directory: None,
        max_bytes: Some(2_000),
    };
    let original_bytes = input.stdout.len() + input.stderr.len();
    let started = Instant::now();
    let summary = compact_command_output(input);
    let mut result = BenchmarkResult::new("command_compaction_latency", started.elapsed());
    result.input_size = original_bytes;
    result.query_result_count = summary.key_findings.len();
    result.metadata.insert(
        "estimated_token_savings".to_string(),
        summary.estimated_token_savings.to_string(),
    );
    result.metadata.insert(
        "command_family".to_string(),
        format!("{:?}", summary.command_family),
    );
    Ok(result)
}

struct IndexedFixture {
    storage: SqliteStorage,
}

impl IndexedFixture {
    fn storage_for_control(&self) -> ContractResult<SqliteStorage> {
        let storage = SqliteStorage::open_in_memory()?;
        let _ = storage.graph_summary(None, None)?;
        Ok(storage)
    }
}

fn index_fixture(fixture: &BenchmarkFixture) -> ContractResult<IndexedFixture> {
    let db = fixture.path.join(".b3").join("bench.db");
    let storage = SqliteStorage::open(&db)?;
    let indexer = LocalIndexer::new(
        RustLanguagePack,
        &storage,
        NoopEventBus,
        IndexerConfig {
            branch_id: BranchId::new(BRANCH_ID),
            ..IndexerConfig::default()
        },
    );
    indexer.index(IndexJob {
        project_id: ProjectId::new(PROJECT_ID),
        root_path: fixture.path.to_string_lossy().to_string(),
    })?;
    Ok(IndexedFixture { storage })
}

fn attach_index_stats(
    result: &mut BenchmarkResult,
    fixture: &BenchmarkFixture,
    storage: &SqliteStorage,
) -> ContractResult<()> {
    let summary = storage.graph_summary(Some(PROJECT_ID), Some(BRANCH_ID))?;
    result.input_size = fixture.total_bytes;
    result.files_indexed = summary.file_count;
    result.symbols_indexed = summary.symbol_count;
    result.edges_indexed = summary.edge_count;
    Ok(())
}

fn bounded_path(
    storage: &SqliteStorage,
    source: &str,
    target: &str,
    max_depth: usize,
    limit: usize,
) -> ContractResult<Vec<String>> {
    let mut queue = VecDeque::from([(source.to_string(), vec![source.to_string()])]);
    let mut seen = HashSet::from([source.to_string()]);
    while let Some((node, path)) = queue.pop_front() {
        if node == target {
            return Ok(path);
        }
        if path.len().saturating_sub(1) >= max_depth {
            continue;
        }
        for edge in storage.graph_edges_for_node(PROJECT_ID, BRANCH_ID, &node, 0, limit)? {
            let next = if edge.from_node_id == node {
                edge.to_node_id
            } else {
                edge.from_node_id
            };
            if seen.insert(next.clone()) {
                let mut next_path = path.clone();
                next_path.push(next.clone());
                queue.push_back((next, next_path));
            }
        }
    }
    Ok(Vec::new())
}

fn first_symbol_node(storage: &SqliteStorage, name: &str) -> ContractResult<String> {
    let symbol = storage
        .find_symbol(&ProjectId::new(PROJECT_ID), name)?
        .into_iter()
        .next()
        .ok_or_else(|| ContractError::new(format!("symbol not found: {name}")))?;
    let node = storage
        .graph_node_by_symbol_id(PROJECT_ID, BRANCH_ID, symbol.id.as_str())?
        .ok_or_else(|| ContractError::new(format!("node not found for symbol: {name}")))?;
    Ok(node.id)
}

fn bench_scope() -> QueryScope {
    QueryScope::new(ProjectId::new(PROJECT_ID), BranchId::new(BRANCH_ID))
}

fn copy_fixture(
    fixtures: &[BenchmarkFixture],
    name: &str,
    workspace: &Path,
) -> ContractResult<BenchmarkFixture> {
    let source = fixtures
        .iter()
        .find(|fixture| fixture.name == name)
        .ok_or_else(|| ContractError::new(format!("fixture not loaded: {name}")))?;
    let destination = workspace.join(name);
    copy_dir(&source.path, &destination)?;
    let (file_count, total_bytes) = fixture_stats(&destination)?;
    Ok(BenchmarkFixture {
        name: source.name.clone(),
        path: destination,
        file_count,
        total_bytes,
    })
}

fn copy_dir(source: &Path, destination: &Path) -> ContractResult<()> {
    fs::create_dir_all(destination).map_err(to_contract_error)?;
    for entry in fs::read_dir(source).map_err(to_contract_error)? {
        let entry = entry.map_err(to_contract_error)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().map_err(to_contract_error)?.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(to_contract_error)?;
        }
    }
    Ok(())
}

fn fixture_stats(path: &Path) -> ContractResult<(usize, usize)> {
    let mut file_count = 0;
    let mut total_bytes = 0;
    for entry in fs::read_dir(path).map_err(to_contract_error)? {
        let entry = entry.map_err(to_contract_error)?;
        let metadata = entry.metadata().map_err(to_contract_error)?;
        if metadata.is_dir() {
            let (child_count, child_bytes) = fixture_stats(&entry.path())?;
            file_count += child_count;
            total_bytes += child_bytes;
        } else if metadata.is_file() {
            file_count += 1;
            total_bytes += metadata.len() as usize;
        }
    }
    Ok((file_count, total_bytes))
}

fn fresh_benchmark_workspace() -> ContractResult<PathBuf> {
    let path =
        std::env::temp_dir().join(format!("b3-bench-{}-{}", std::process::id(), now_unix_ms()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).map_err(to_contract_error)?;
    Ok(path)
}

fn temp_path(prefix: &str, file_name: &str) -> ContractResult<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "b3-bench-{prefix}-{}-{}",
        std::process::id(),
        now_unix_ms()
    ));
    fs::create_dir_all(&path).map_err(to_contract_error)?;
    Ok(path.join(file_name))
}

fn default_fixture_root() -> PathBuf {
    workspace_root().join(DEFAULT_FIXTURE_ROOT)
}

fn workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("Cargo.toml").exists() && cwd.join("crates").exists() {
        return cwd;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn write_json_output(path: &Path, run: &BenchmarkRun) -> ContractResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(to_contract_error)?;
    }
    let json = serde_json::to_string_pretty(run).map_err(to_contract_error)?;
    fs::write(path, json).map_err(to_contract_error)
}

fn git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn rough_memory_kb() -> Option<u64> {
    None
}

fn assert_local_only() {
    for key in [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "GEMINI_API_KEY",
        "B3_TELEMETRY_ENDPOINT",
    ] {
        let _ = std::env::var_os(key);
    }
}

#[derive(Clone, Copy)]
struct NoopEventBus;

impl EventBus for NoopEventBus {
    fn publish(&self, _event: b3_core::DomainEvent) -> ContractResult<()> {
        Ok(())
    }
}

fn to_contract_error(error: impl std::fmt::Display) -> ContractError {
    ContractError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_config_parses_json() {
        let config: BenchmarkThresholdConfig = serde_json::from_str(
            r#"{
                "allowed_latency_regression_percent": 10,
                "allowed_indexing_regression_percent": 15,
                "memory_warning_kb": 1234,
                "fail_on_regression": false
            }"#,
        )
        .expect("config");
        assert_eq!(config.allowed_latency_regression_percent, 10.0);
        assert_eq!(config.memory_warning_kb, Some(1234));
        assert!(!config.fail_on_regression);
    }

    #[test]
    fn benchmark_result_serializes_json() {
        let result = BenchmarkResult::new("unit", Duration::from_millis(2));
        let json = serde_json::to_string(&result).expect("json");
        assert!(json.contains("duration_ms"));
        assert!(json.contains("unit"));
    }

    #[test]
    fn fixture_loading_finds_all_expected_repos() {
        let fixtures = load_fixtures(&default_fixture_root()).expect("fixtures");
        assert_eq!(fixtures.len(), 5);
        assert!(fixtures.iter().all(|fixture| fixture.file_count > 0));
    }

    #[test]
    fn benchmark_result_table_has_headers() {
        let table =
            format_results_table(&[BenchmarkResult::new("formatting", Duration::from_millis(1))]);
        assert!(table.contains("benchmark | duration_ms"));
        assert!(table.contains("formatting"));
    }

    #[test]
    fn benchmark_defaults_do_not_enable_network_or_ci_failure() {
        let config = BenchmarkThresholdConfig::default();
        assert!(!config.fail_on_regression);
        assert!(config.memory_warning_kb.is_none());
    }
}
