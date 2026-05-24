//! Control server and localhost API boundary.
//!
//! This crate is an adapter layer for local developer tooling. It exposes
//! health, status, query, graph, diagnostics, config, and event endpoints for
//! the future localhost UI while keeping indexing, storage internals,
//! embeddings, and MCP protocol handling behind their own crate boundaries.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use axum::{
    extract::{Json, Path, State},
    http::{HeaderValue, Method, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use b3_core::{
    AppConfig, BranchId, BranchMetadata, ContractError, ContractResult, EventBus, FileId,
    FileRecord, IndexStore, ProjectId, QueryRequest, QueryResult, SymbolRepository, PRODUCT_NAME,
};
use b3_indexer::{
    IndexerConfig, LocalIndexer, NoopTreeSitterParser, NotifyFileWatcher, WatchConfig,
    WatchEventKind,
};
use b3_mcp_runtime::{runtime_info, RuntimeResponsibility};
use b3_storage::{
    SavingsSummary, SqliteStorage, StorageStats, StoredCentralityRecord, StoredGraphEdge,
    StoredGraphNode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{
    net::TcpListener,
    sync::{broadcast, Mutex},
};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tower_http::cors::{AllowOrigin, CorsLayer};

pub use b3_core::{ConfigProvider, DomainEvent};

const DEFAULT_HOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_PORT: u16 = 7777;
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 200;
const DEFAULT_GRAPH_DEPTH: u8 = 1;
const MAX_GRAPH_DEPTH: u8 = 3;
const DEFAULT_TOKEN_BUDGET: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlaneInfo {
    pub product: &'static str,
    pub ui_path: &'static str,
    pub websocket_path: &'static str,
    pub enabled_by_default: bool,
}

pub fn control_plane_info() -> ControlPlaneInfo {
    ControlPlaneInfo {
        product: PRODUCT_NAME,
        ui_path: "/",
        websocket_path: "/api/events",
        enabled_by_default: false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeOptions {
    pub project_path: PathBuf,
    pub database_path: PathBuf,
    pub bind_addr: SocketAddr,
    pub allow_non_local_bind: bool,
    pub watch: bool,
    pub debounce_ms: u64,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            project_path: PathBuf::from("."),
            database_path: PathBuf::from(".b3").join("b3.db"),
            bind_addr: SocketAddr::new(DEFAULT_HOST, DEFAULT_PORT),
            allow_non_local_bind: false,
            watch: false,
            debounce_ms: 500,
        }
    }
}

impl ServeOptions {
    pub fn validate(&self) -> Result<(), ControlError> {
        if !self.allow_non_local_bind && !is_local_ip(self.bind_addr.ip()) {
            return Err(ControlError::bad_request(
                "non-local bind addresses require --allow-non-local-bind",
            ));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct ControlState {
    project_path: Arc<PathBuf>,
    database_path: Arc<PathBuf>,
    storage: Arc<Mutex<SqliteStorage>>,
    app_config: Arc<AppConfig>,
    events: EventHub,
}

impl ControlState {
    pub fn new(options: &ServeOptions) -> Result<Self, ControlError> {
        options.validate()?;
        let storage =
            SqliteStorage::open(&options.database_path).map_err(ControlError::internal)?;
        Ok(Self {
            project_path: Arc::new(options.project_path.clone()),
            database_path: Arc::new(options.database_path.clone()),
            storage: Arc::new(Mutex::new(storage)),
            app_config: Arc::new(AppConfig::default()),
            events: EventHub::new(256),
        })
    }

    pub fn from_storage(
        project_path: PathBuf,
        database_path: PathBuf,
        storage: SqliteStorage,
    ) -> Self {
        Self {
            project_path: Arc::new(project_path),
            database_path: Arc::new(database_path),
            storage: Arc::new(Mutex::new(storage)),
            app_config: Arc::new(AppConfig::default()),
            events: EventHub::new(256),
        }
    }
}

#[derive(Clone)]
struct EventHub {
    sender: broadcast::Sender<ServerEvent>,
}

impl EventHub {
    fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    fn emit(&self, event_type: impl Into<String>, payload: Value) {
        let _ = self.sender.send(ServerEvent {
            event_type: event_type.into(),
            payload,
        });
    }

    fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }
}

#[derive(Debug, Clone, Serialize)]
struct ServerEvent {
    event_type: String,
    payload: Value,
}

pub fn app(state: ControlState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/status", get(status))
        .route("/api/projects", get(projects))
        .route("/api/project", get(project))
        .route("/api/query/:operation", post(query_operation))
        .route("/api/graph/summary", get(graph_summary))
        .route("/api/graph/neighbors", post(graph_neighbors))
        .route("/api/graph/path", post(graph_path))
        .route("/api/graph/cycles", post(graph_cycles))
        .route("/api/graph/centrality", post(graph_centrality))
        .route("/api/savings/summary", get(savings_summary))
        .route("/api/diagnostics", get(diagnostics))
        .route("/api/capabilities", get(capabilities))
        .route("/api/config", get(config))
        .route("/api/config/validate", post(validate_config))
        .route("/api/events", get(events))
        .layer(localhost_cors())
        .with_state(state)
}

pub async fn serve(options: ServeOptions) -> Result<(), ControlError> {
    let state = ControlState::new(&options)?;
    state.events.emit(
        "server_started",
        json!({"status": "ok", "watch": options.watch}),
    );
    if options.watch {
        start_watch_daemon(&options, state.events.clone())?;
    }
    let listener = TcpListener::bind(options.bind_addr)
        .await
        .map_err(ControlError::internal)?;

    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(ControlError::internal)
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        offline_mode: true,
        telemetry_enabled: false,
    })
}

async fn status(State(state): State<ControlState>) -> Result<Json<StatusResponse>, ControlError> {
    let stats = storage_stats(&state).await?;
    let branch = current_branch(&state).await?;

    Ok(Json(StatusResponse {
        status: "ok",
        project_path: path_string(&state.project_path),
        database_path: path_string(&state.database_path),
        offline_mode: true,
        indexed_file_count: stats.files,
        symbol_count: stats.symbols,
        edge_count: stats.edges,
        current_branch: branch,
        mcp_runtime: RuntimeSummary::default(),
    }))
}

async fn projects(
    State(state): State<ControlState>,
) -> Result<Json<ProjectsResponse>, ControlError> {
    let roots = state
        .storage
        .lock()
        .await
        .project_roots()
        .map_err(ControlError::internal)?;
    let projects = if roots.is_empty() {
        vec![ProjectResponse {
            path: path_string(&state.project_path),
            active: true,
        }]
    } else {
        roots
            .into_iter()
            .map(|path| ProjectResponse {
                path,
                active: false,
            })
            .collect()
    };

    Ok(Json(ProjectsResponse { projects }))
}

async fn project(State(state): State<ControlState>) -> Result<Json<ProjectDetail>, ControlError> {
    let stats = storage_stats(&state).await?;
    Ok(Json(ProjectDetail {
        path: path_string(&state.project_path),
        database_path: path_string(&state.database_path),
        indexed_file_count: stats.files,
        symbol_count: stats.symbols,
        edge_count: stats.edges,
        offline_mode: true,
    }))
}

async fn query_operation(
    Path(operation): Path<String>,
    State(state): State<ControlState>,
    payload: Result<Json<QueryApiRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<QueryApiResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;

    match operation.as_str() {
        "find-symbol" => find_symbol(state, request).await,
        "search-code" => placeholder_query(operation, request),
        "find-callers" => placeholder_query(operation, request),
        "find-callees" => placeholder_query(operation, request),
        "related-symbols" => placeholder_query(operation, request),
        "impact-analysis" => placeholder_query(operation, request),
        "context-pack" => context_pack_placeholder(request),
        "trace-dependency" => placeholder_query(operation, request),
        "detect-cycles" => placeholder_query(operation, request),
        _ => Err(ControlError::not_found("unknown query operation")),
    }
}

async fn find_symbol(
    state: ControlState,
    request: QueryApiRequest,
) -> Result<Json<QueryApiResponse>, ControlError> {
    let symbol = request
        .symbol
        .or(request.query)
        .ok_or_else(|| ControlError::bad_request("find-symbol requires symbol or query"))?;
    let project_id = ProjectId::new(request.scope.project_id);
    let symbols = state
        .storage
        .lock()
        .await
        .find_symbol(&project_id, &symbol)
        .map_err(ControlError::internal)?;
    let limit = bounded_limit(request.limit);
    let matches = symbols
        .into_iter()
        .take(limit)
        .map(|symbol| QueryMatch {
            id: symbol.id.as_str().to_string(),
            name: symbol.name,
            file_id: Some(symbol.file_id.as_str().to_string()),
            path: None,
            score: None,
        })
        .collect();

    Ok(Json(QueryApiResponse {
        operation: "find-symbol".to_string(),
        status: "ok",
        partial: false,
        message: None,
        matches,
        include_trace: request.include_trace.unwrap_or(false),
        trace: Vec::new(),
        full_file_dump_included: false,
        query_result: Some(QueryResultDto::from(QueryResult {
            summary: format!("symbol lookup for {symbol}"),
            returned_tokens: 0,
            expansion_handles: Vec::new(),
        })),
    }))
}

fn placeholder_query(
    operation: String,
    request: QueryApiRequest,
) -> Result<Json<QueryApiResponse>, ControlError> {
    let token_budget = request.token_budget.unwrap_or(DEFAULT_TOKEN_BUDGET);
    let query_text = request.query.or(request.symbol).unwrap_or_default();
    let query = QueryRequest::new(query_text, token_budget);
    let result = QueryResult {
        summary: format!(
            "{operation} is exposed through the control API; ranking engine integration is deferred"
        ),
        returned_tokens: 0,
        expansion_handles: Vec::new(),
    };

    Ok(Json(QueryApiResponse {
        operation,
        status: "not_implemented",
        partial: true,
        message: Some("query engine integration is deferred; no fake results returned".to_string()),
        matches: Vec::new(),
        include_trace: request.include_trace.unwrap_or(false),
        trace: Vec::new(),
        full_file_dump_included: false,
        query_result: Some(QueryResultDto {
            summary: result.summary,
            returned_tokens: result.returned_tokens,
            expansion_handles: result.expansion_handles,
            request_token_budget: Some(query.token_budget),
        }),
    }))
}

fn context_pack_placeholder(
    request: QueryApiRequest,
) -> Result<Json<QueryApiResponse>, ControlError> {
    Ok(Json(QueryApiResponse {
        operation: "context-pack".to_string(),
        status: "not_implemented",
        partial: true,
        message: Some(
            "context pack generation is deferred; full-file dumps are disabled".to_string(),
        ),
        matches: Vec::new(),
        include_trace: request.include_trace.unwrap_or(false),
        trace: Vec::new(),
        full_file_dump_included: false,
        query_result: Some(QueryResultDto {
            summary: "context pack placeholder".to_string(),
            returned_tokens: 0,
            expansion_handles: Vec::new(),
            request_token_budget: request.token_budget,
        }),
    }))
}

async fn graph_summary(
    State(state): State<ControlState>,
) -> Result<Json<GraphSummaryResponse>, ControlError> {
    let summary = state
        .storage
        .lock()
        .await
        .graph_summary(None, None)
        .map_err(ControlError::internal)?;

    Ok(Json(GraphSummaryResponse {
        project_id: summary.project_id.unwrap_or_else(|| "default".to_string()),
        branch_id: summary.branch_id.unwrap_or_else(|| "main".to_string()),
        node_count: summary.node_count,
        edge_count: summary.edge_count,
        symbol_count: summary.symbol_count,
        file_count: summary.file_count,
        edge_type_counts: summary
            .edge_type_counts
            .into_iter()
            .map(CountDto::from)
            .collect(),
        node_kind_counts: summary
            .node_kind_counts
            .into_iter()
            .map(CountDto::from)
            .collect(),
        centrality_snapshot_status: if summary.centrality_snapshot_count > 0 {
            "available".to_string()
        } else {
            "empty".to_string()
        },
        max_depth: MAX_GRAPH_DEPTH,
        branch_aware: true,
        full_graph_dump_included: false,
        partial: false,
        message: "summary is read from local SQLite graph storage".to_string(),
    }))
}

async fn graph_neighbors(
    State(state): State<ControlState>,
    payload: Result<Json<GraphNeighborsRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphNeighborsResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let max_depth = checked_graph_depth(request.max_depth.or(request.depth))?;
    let limit = checked_graph_limit(request.limit)?;
    let min_confidence = checked_confidence(request.min_confidence)?;
    let edge_types = request.edge_types.clone().unwrap_or_default();
    let direction = request.direction.unwrap_or_default();

    let storage = state.storage.lock().await;
    let seed = resolve_seed_node(
        &storage,
        &project_id,
        &branch_id,
        request
            .seed_node_id
            .as_deref()
            .or(request.node_id.as_deref()),
        request.seed_symbol_id.as_deref(),
    )?;

    let Some(seed) = seed else {
        return Ok(Json(GraphNeighborsResponse {
            project_id,
            branch_id,
            seed_node_id: None,
            depth: max_depth,
            limit,
            nodes: Vec::new(),
            edges: Vec::new(),
            partial: false,
            full_graph_dump_included: false,
            message: "seed node or symbol was not found".to_string(),
        }));
    };

    let graph = bounded_neighbor_graph(
        &storage,
        &project_id,
        &branch_id,
        &seed,
        direction,
        &edge_types,
        max_depth,
        min_confidence,
        limit,
    )?;

    Ok(Json(GraphNeighborsResponse {
        project_id,
        branch_id,
        seed_node_id: Some(seed.id),
        depth: max_depth,
        limit,
        nodes: graph.nodes,
        edges: graph.edges,
        partial: graph.partial,
        full_graph_dump_included: false,
        message: "bounded neighbor graph read from local SQLite".to_string(),
    }))
}

async fn graph_path(
    State(state): State<ControlState>,
    payload: Result<Json<GraphPathRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphPathResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let max_depth = checked_graph_depth(request.max_depth.or(request.depth))?;
    let limit = checked_graph_limit(request.limit)?;
    let min_confidence = checked_confidence(request.min_confidence)?;
    let edge_types = request.edge_types.clone().unwrap_or_default();
    let source = request
        .source_node_id
        .as_deref()
        .or(request.from_node_id.as_deref())
        .ok_or_else(|| ControlError::bad_request("source_node_id is required"))?;
    let target = request
        .target_node_id
        .as_deref()
        .or(request.to_node_id.as_deref())
        .ok_or_else(|| ControlError::bad_request("target_node_id is required"))?;

    let storage = state.storage.lock().await;
    let response = shortest_path(
        &storage,
        &project_id,
        &branch_id,
        source,
        target,
        &edge_types,
        max_depth,
        min_confidence,
        limit,
    )?;
    Ok(Json(response))
}

async fn graph_cycles(
    State(state): State<ControlState>,
    payload: Result<Json<GraphCyclesRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphCyclesResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let limit = checked_graph_limit(request.limit)?;
    let min_confidence = checked_confidence(request.min_confidence)?;
    let edge_types = request.edge_types.clone().unwrap_or_default();
    let storage = state.storage.lock().await;
    let edges = filtered_edges(
        storage.graph_edges_scoped(&project_id, &branch_id, min_confidence, limit + 1)?,
        &edge_types,
    );
    let bounded_warning = (edges.len() > limit)
        .then(|| "edge scan reached the requested limit; cycle results may be partial".to_string());
    let cycles = cycle_groups(edges.into_iter().take(limit).collect());

    Ok(Json(GraphCyclesResponse {
        project_id,
        branch_id,
        cycle_count: cycles.len(),
        scc_groups: cycles,
        bounded_warning,
        partial: false,
    }))
}

async fn graph_centrality(
    State(state): State<ControlState>,
    payload: Result<Json<GraphCentralityRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphCentralityResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let limit = checked_graph_limit(request.limit)?;
    let rows = state
        .storage
        .lock()
        .await
        .centrality_snapshot(&project_id, &branch_id, limit)
        .map_err(ControlError::internal)?;

    let message = if rows.is_empty() {
        "no cached centrality snapshot is available; not computing automatically".to_string()
    } else {
        "cached centrality snapshot returned from local SQLite".to_string()
    };

    Ok(Json(GraphCentralityResponse {
        project_id,
        branch_id,
        nodes: rows.into_iter().map(GraphCentralityNodeDto::from).collect(),
        calculated: !message.starts_with("no cached"),
        message,
    }))
}

async fn savings_summary(
    State(state): State<ControlState>,
) -> Result<Json<SavingsSummaryResponse>, ControlError> {
    let summary = state
        .storage
        .lock()
        .await
        .savings_summary()
        .map_err(ControlError::internal)?;
    Ok(Json(SavingsSummaryResponse::from(summary)))
}

async fn diagnostics(State(state): State<ControlState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "project_path": path_string(&state.project_path),
        "database_path": path_string(&state.database_path),
        "offline_mode": true,
        "telemetry_enabled": false,
        "source_upload_enabled": false,
        "known_limitations": [
            "query ranking integration is deferred",
            "file watcher events are deferred",
            "frontend UI is deferred"
        ]
    }))
}

async fn capabilities() -> Json<Value> {
    Json(json!({
        "product": PRODUCT_NAME,
        "offline_first": true,
        "free_by_default": true,
        "external_api_required": false,
        "telemetry_enabled": false,
        "mcp_runtime": RuntimeSummary::default(),
        "control_api": {
            "projects": true,
            "query": true,
            "graph": true,
            "config_read": true,
            "config_mutation": false,
            "events": "sse"
        }
    }))
}

async fn config(State(state): State<ControlState>) -> Json<Value> {
    let config = state.app_config.as_ref();
    Json(json!({
        "offline": {
            "local_storage_only": config.offline.local_storage_only,
            "local_embeddings_by_default": config.offline.local_embeddings_by_default,
            "external_providers_enabled_by_default": config.offline.external_providers.enabled_by_default
        },
        "project": {
            "root_path": config.project.root_path,
            "branch_aware": config.project.branch_aware
        },
        "indexing": {
            "enabled": config.indexing.enabled,
            "parser_subprocess_isolation": config.indexing.parser_subprocess_isolation,
            "watch_files": config.indexing.watch_files,
            "max_parallel_workers": config.indexing.max_parallel_workers,
            "debounce_ms": config.indexing.debounce_ms,
            "max_watch_batch_size": config.indexing.max_watch_batch_size,
            "ignore_patterns": config.indexing.ignore_patterns
        },
        "retrieval": {
            "max_graph_depth": config.retrieval.max_graph_depth,
            "max_tokens": config.retrieval.max_tokens,
            "bm25_enabled": config.retrieval.bm25_enabled,
            "semantic_enabled": config.retrieval.semantic_enabled,
            "local_qdrant_enabled": config.retrieval.local_qdrant_enabled
        },
        "ui": {
            "control_server_enabled": config.ui.control_server_enabled,
            "websocket_enabled": config.ui.websocket_enabled,
            "bind_address": config.ui.bind_address
        }
    }))
}

async fn validate_config(
    payload: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ControlError> {
    let _ = payload.map_err(|rejection| ControlError::bad_request(rejection.body_text()))?;
    Ok(Json(json!({
        "valid": true,
        "mutation_supported": false,
        "message": "config validation skeleton accepted JSON; mutation is deferred"
    })))
}

fn start_watch_daemon(options: &ServeOptions, events: EventHub) -> Result<(), ControlError> {
    let project_path = options.project_path.clone();
    let database_path = options.database_path.clone();
    let debounce_ms = options.debounce_ms;
    std::thread::Builder::new()
        .name("b3-watch-daemon".to_string())
        .spawn(move || {
            events.emit(
                "watcher_started",
                json!({"project_path": project_path.to_string_lossy(), "debounce_ms": debounce_ms}),
            );

            let project_id = ProjectId::new("default");
            let branch_id = BranchId::new("main");
            let storage = match SqliteStorage::open(&database_path) {
                Ok(storage) => storage,
                Err(error) => {
                    events.emit("indexing_failed", json!({"error": error.to_string()}));
                    return;
                }
            };
            let _ = storage.upsert_project(&project_id, "default", &project_path.to_string_lossy());
            let _ = storage.upsert_branch(&branch_id, &project_id, &BranchMetadata::new("main"));

            let indexer = LocalIndexer::new(
                NoopTreeSitterParser,
                SqliteIndexStore {
                    storage: StdMutex::new(storage),
                },
                EventForwarder {
                    events: events.clone(),
                },
                IndexerConfig {
                    branch_id,
                    ..IndexerConfig::default()
                },
            );
            let watcher = NotifyFileWatcher::new(WatchConfig {
                enabled: true,
                debounce_ms,
                max_batch_size: 100,
                ignore: b3_indexer::IgnoreRules::default(),
            });

            loop {
                match watcher.collect_batch(&project_path, Duration::from_secs(1)) {
                    Ok(Some(batch)) => {
                        let paths = batch
                            .events
                            .iter()
                            .map(|event| {
                                emit_watch_event(
                                    &events,
                                    event.kind,
                                    &event.path,
                                    event.new_path.as_ref(),
                                );
                                event.new_path.clone().unwrap_or_else(|| event.path.clone())
                            })
                            .collect::<Vec<_>>();
                        events.emit("debounce_flushed", json!({"path_count": paths.len()}));
                        events.emit("indexing_started", json!({"path_count": paths.len()}));
                        match indexer.index_paths(&project_path, &project_id, &paths) {
                            Ok(summary) => events.emit(
                                "indexing_completed",
                                json!({
                                    "files_seen": summary.files_seen,
                                    "files_parsed": summary.files_parsed,
                                    "symbols_indexed": summary.symbols_indexed
                                }),
                            ),
                            Err(error) => {
                                events.emit("indexing_failed", json!({"error": error.to_string()}));
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        events.emit("indexing_failed", json!({"error": error.to_string()}))
                    }
                }
            }
        })
        .map_err(ControlError::internal)?;
    Ok(())
}

fn emit_watch_event(
    events: &EventHub,
    kind: WatchEventKind,
    path: &std::path::Path,
    new_path: Option<&PathBuf>,
) {
    let event_type = match kind {
        WatchEventKind::Created => "file_created",
        WatchEventKind::Changed => "file_changed",
        WatchEventKind::Deleted => "file_deleted",
        WatchEventKind::Renamed => "file_renamed",
    };
    events.emit(
        event_type,
        json!({
            "path": path.to_string_lossy(),
            "new_path": new_path.map(|path| path.to_string_lossy().to_string())
        }),
    );
}

async fn events(
    State(state): State<ControlState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|event| match event {
        Ok(event) => Some(Ok(Event::default()
            .event(event.event_type)
            .data(event.payload.to_string()))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn storage_stats(state: &ControlState) -> Result<StorageStats, ControlError> {
    state
        .storage
        .lock()
        .await
        .storage_stats()
        .map_err(ControlError::internal)
}

async fn current_branch(state: &ControlState) -> Result<Option<String>, ControlError> {
    state
        .storage
        .lock()
        .await
        .current_branch_name()
        .map_err(ControlError::internal)
}

fn localhost_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([http::header::CONTENT_TYPE])
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            is_localhost_origin(origin)
        }))
}

fn is_localhost_origin(origin: &HeaderValue) -> bool {
    origin
        .to_str()
        .map(|value| {
            value.starts_with("http://127.0.0.1:")
                || value.starts_with("http://localhost:")
                || value.starts_with("http://[::1]:")
        })
        .unwrap_or(false)
}

fn is_local_ip(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn path_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}

fn bounded_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
}

#[derive(Debug)]
pub struct ControlError {
    status: StatusCode,
    message: String,
}

impl ControlError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl From<ContractError> for ControlError {
    fn from(error: ContractError) -> Self {
        Self::internal(error)
    }
}

impl IntoResponse for ControlError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: ErrorBody {
                code: self.status.as_u16(),
                message: self.message,
            },
        });
        (self.status, body).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: u16,
    message: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    offline_mode: bool,
    telemetry_enabled: bool,
}

#[derive(Debug, Serialize)]
struct RuntimeSummary {
    name: &'static str,
    protocol: &'static str,
    boundary: &'static str,
    responsibilities: Vec<&'static str>,
}

impl Default for RuntimeSummary {
    fn default() -> Self {
        let info = runtime_info();
        Self {
            name: info.name,
            protocol: info.protocol,
            boundary: "protocol_only",
            responsibilities: vec![
                responsibility(RuntimeResponsibility::StdioTransport),
                responsibility(RuntimeResponsibility::JsonRpc),
                responsibility(RuntimeResponsibility::ToolRouting),
                responsibility(RuntimeResponsibility::Streaming),
                responsibility(RuntimeResponsibility::Cancellation),
                responsibility(RuntimeResponsibility::SessionLifecycle),
            ],
        }
    }
}

fn responsibility(value: RuntimeResponsibility) -> &'static str {
    match value {
        RuntimeResponsibility::StdioTransport => "stdio_transport",
        RuntimeResponsibility::JsonRpc => "json_rpc",
        RuntimeResponsibility::ToolRouting => "tool_routing",
        RuntimeResponsibility::Streaming => "streaming",
        RuntimeResponsibility::Cancellation => "cancellation",
        RuntimeResponsibility::SessionLifecycle => "session_lifecycle",
    }
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    status: &'static str,
    project_path: String,
    database_path: String,
    offline_mode: bool,
    indexed_file_count: usize,
    symbol_count: usize,
    edge_count: usize,
    current_branch: Option<String>,
    mcp_runtime: RuntimeSummary,
}

#[derive(Debug, Serialize)]
struct ProjectsResponse {
    projects: Vec<ProjectResponse>,
}

#[derive(Debug, Serialize)]
struct ProjectResponse {
    path: String,
    active: bool,
}

#[derive(Debug, Serialize)]
struct ProjectDetail {
    path: String,
    database_path: String,
    indexed_file_count: usize,
    symbol_count: usize,
    edge_count: usize,
    offline_mode: bool,
}

#[derive(Debug, Deserialize)]
struct QueryApiRequest {
    query: Option<String>,
    symbol: Option<String>,
    scope: QueryScope,
    include_trace: Option<bool>,
    limit: Option<usize>,
    token_budget: Option<usize>,
}

impl QueryApiRequest {
    fn validate(&self) -> Result<(), ControlError> {
        self.scope.validate()?;
        if self.limit.unwrap_or(DEFAULT_LIMIT) == 0 {
            return Err(ControlError::bad_request("limit must be greater than zero"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct QueryScope {
    project_id: String,
    branch_id: Option<String>,
    path_prefix: Option<String>,
}

impl QueryScope {
    fn validate(&self) -> Result<(), ControlError> {
        if self.project_id.trim().is_empty() {
            return Err(ControlError::bad_request("scope.project_id is required"));
        }
        if self.branch_id.as_deref().is_some_and(str::is_empty) {
            return Err(ControlError::bad_request(
                "scope.branch_id must not be empty when provided",
            ));
        }
        if self.path_prefix.as_deref().is_some_and(str::is_empty) {
            return Err(ControlError::bad_request(
                "scope.path_prefix must not be empty when provided",
            ));
        }
        Ok(())
    }

    fn branch_id_or_default(&self) -> String {
        self.branch_id.clone().unwrap_or_else(|| "main".to_string())
    }
}

#[derive(Debug, Serialize)]
struct QueryApiResponse {
    operation: String,
    status: &'static str,
    partial: bool,
    message: Option<String>,
    matches: Vec<QueryMatch>,
    include_trace: bool,
    trace: Vec<Value>,
    full_file_dump_included: bool,
    query_result: Option<QueryResultDto>,
}

#[derive(Debug, Serialize)]
struct QueryMatch {
    id: String,
    name: String,
    file_id: Option<String>,
    path: Option<String>,
    score: Option<f32>,
}

#[derive(Debug, Serialize)]
struct QueryResultDto {
    summary: String,
    returned_tokens: usize,
    expansion_handles: Vec<String>,
    request_token_budget: Option<usize>,
}

impl From<QueryResult> for QueryResultDto {
    fn from(value: QueryResult) -> Self {
        Self {
            summary: value.summary,
            returned_tokens: value.returned_tokens,
            expansion_handles: value.expansion_handles,
            request_token_budget: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GraphDirection {
    Inbound,
    Outbound,
    Both,
}

impl Default for GraphDirection {
    fn default() -> Self {
        Self::Both
    }
}

#[derive(Debug, Deserialize)]
struct GraphNeighborsRequest {
    scope: QueryScope,
    seed_node_id: Option<String>,
    seed_symbol_id: Option<String>,
    node_id: Option<String>,
    direction: Option<GraphDirection>,
    depth: Option<u8>,
    max_depth: Option<u8>,
    limit: Option<usize>,
    min_confidence: Option<u16>,
    edge_types: Option<Vec<String>>,
}

impl GraphNeighborsRequest {
    fn validate(&self) -> Result<(), ControlError> {
        self.scope.validate()?;
        validate_optional_id(self.seed_node_id.as_deref(), "seed_node_id")?;
        validate_optional_id(self.seed_symbol_id.as_deref(), "seed_symbol_id")?;
        validate_optional_id(self.node_id.as_deref(), "node_id")?;
        checked_graph_depth(self.max_depth.or(self.depth))?;
        checked_graph_limit(self.limit)?;
        checked_confidence(self.min_confidence)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GraphPathRequest {
    scope: QueryScope,
    source_node_id: Option<String>,
    target_node_id: Option<String>,
    from_node_id: Option<String>,
    to_node_id: Option<String>,
    depth: Option<u8>,
    max_depth: Option<u8>,
    limit: Option<usize>,
    min_confidence: Option<u16>,
    edge_types: Option<Vec<String>>,
}

impl GraphPathRequest {
    fn validate(&self) -> Result<(), ControlError> {
        self.scope.validate()?;
        validate_optional_id(self.source_node_id.as_deref(), "source_node_id")?;
        validate_optional_id(self.target_node_id.as_deref(), "target_node_id")?;
        validate_optional_id(self.from_node_id.as_deref(), "from_node_id")?;
        validate_optional_id(self.to_node_id.as_deref(), "to_node_id")?;
        checked_graph_depth(self.max_depth.or(self.depth))?;
        checked_graph_limit(self.limit)?;
        checked_confidence(self.min_confidence)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GraphCyclesRequest {
    scope: QueryScope,
    limit: Option<usize>,
    min_confidence: Option<u16>,
    edge_types: Option<Vec<String>>,
}

impl GraphCyclesRequest {
    fn validate(&self) -> Result<(), ControlError> {
        self.scope.validate()?;
        checked_graph_limit(self.limit)?;
        checked_confidence(self.min_confidence)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct GraphCentralityRequest {
    scope: QueryScope,
    limit: Option<usize>,
}

impl GraphCentralityRequest {
    fn validate(&self) -> Result<(), ControlError> {
        self.scope.validate()?;
        checked_graph_limit(self.limit)?;
        Ok(())
    }
}

fn validate_optional_id(value: Option<&str>, field: &str) -> Result<(), ControlError> {
    if value.is_some_and(str::is_empty) {
        return Err(ControlError::bad_request(format!(
            "{field} must not be empty when provided"
        )));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct GraphSummaryResponse {
    project_id: String,
    branch_id: String,
    node_count: usize,
    edge_count: usize,
    symbol_count: usize,
    file_count: usize,
    edge_type_counts: Vec<CountDto>,
    node_kind_counts: Vec<CountDto>,
    centrality_snapshot_status: String,
    max_depth: u8,
    branch_aware: bool,
    full_graph_dump_included: bool,
    partial: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct CountDto {
    name: String,
    count: usize,
}

impl From<b3_storage::GraphCount> for CountDto {
    fn from(value: b3_storage::GraphCount) -> Self {
        Self {
            name: value.name,
            count: value.count,
        }
    }
}

#[derive(Debug, Serialize)]
struct GraphNeighborsResponse {
    project_id: String,
    branch_id: String,
    seed_node_id: Option<String>,
    depth: u8,
    limit: usize,
    nodes: Vec<GraphNodeDto>,
    edges: Vec<GraphEdgeDto>,
    partial: bool,
    full_graph_dump_included: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct GraphNodeDto {
    id: String,
    name: String,
    kind: String,
    file_path: Option<String>,
    symbol_id: Option<String>,
    language: Option<String>,
    visibility: Option<String>,
    centrality: Option<f64>,
    branch_id: String,
    provenance: Option<String>,
}

impl From<StoredGraphNode> for GraphNodeDto {
    fn from(value: StoredGraphNode) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            language: value.language,
            visibility: value.visibility,
            centrality: None,
            branch_id: value.branch_id,
            provenance: value.provenance,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GraphEdgeDto {
    id: String,
    edge_type: String,
    from_node_id: String,
    to_node_id: String,
    confidence: u16,
    provenance: String,
    branch_id: String,
}

impl From<StoredGraphEdge> for GraphEdgeDto {
    fn from(value: StoredGraphEdge) -> Self {
        Self {
            id: value.id,
            edge_type: value.edge_type,
            from_node_id: value.from_node_id,
            to_node_id: value.to_node_id,
            confidence: value.confidence,
            provenance: value.provenance,
            branch_id: value.branch_id,
        }
    }
}

#[derive(Debug, Serialize)]
struct GraphPathResponse {
    project_id: String,
    branch_id: String,
    found: bool,
    nodes: Vec<GraphNodeDto>,
    edges: Vec<GraphEdgeDto>,
    path_length: usize,
    confidence_summary: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct GraphCyclesResponse {
    project_id: String,
    branch_id: String,
    cycle_count: usize,
    scc_groups: Vec<SccGroupDto>,
    bounded_warning: Option<String>,
    partial: bool,
}

#[derive(Debug, Serialize)]
struct SccGroupDto {
    node_ids: Vec<String>,
    edge_types: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GraphCentralityResponse {
    project_id: String,
    branch_id: String,
    nodes: Vec<GraphCentralityNodeDto>,
    calculated: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct GraphCentralityNodeDto {
    node_id: String,
    symbol_id: Option<String>,
    name: String,
    kind: String,
    pagerank_score: f64,
    in_degree: usize,
    out_degree: usize,
    fan_in: usize,
    fan_out: usize,
    degree_centrality: f64,
    component_size: usize,
    is_cycle_member: bool,
    calculated_at: u64,
    algorithm_version: String,
}

impl From<StoredCentralityRecord> for GraphCentralityNodeDto {
    fn from(value: StoredCentralityRecord) -> Self {
        Self {
            node_id: value.node_id,
            symbol_id: value.symbol_id,
            name: value.name,
            kind: value.kind,
            pagerank_score: value.pagerank_score,
            in_degree: value.in_degree,
            out_degree: value.out_degree,
            fan_in: value.fan_in,
            fan_out: value.fan_out,
            degree_centrality: value.degree_centrality,
            component_size: value.component_size,
            is_cycle_member: value.is_cycle_member,
            calculated_at: value.calculated_at_unix_ms,
            algorithm_version: value.algorithm_version,
        }
    }
}

struct NeighborGraph {
    nodes: Vec<GraphNodeDto>,
    edges: Vec<GraphEdgeDto>,
    partial: bool,
}

fn resolve_seed_node(
    storage: &SqliteStorage,
    project_id: &str,
    branch_id: &str,
    seed_node_id: Option<&str>,
    seed_symbol_id: Option<&str>,
) -> Result<Option<StoredGraphNode>, ControlError> {
    if let Some(node_id) = seed_node_id {
        return storage
            .graph_node_by_id(project_id, branch_id, node_id)
            .map_err(ControlError::internal);
    }

    if let Some(symbol_id) = seed_symbol_id {
        return storage
            .graph_node_by_symbol_id(project_id, branch_id, symbol_id)
            .map_err(ControlError::internal);
    }

    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn bounded_neighbor_graph(
    storage: &SqliteStorage,
    project_id: &str,
    branch_id: &str,
    seed: &StoredGraphNode,
    direction: GraphDirection,
    edge_types: &[String],
    max_depth: u8,
    min_confidence: u16,
    limit: usize,
) -> Result<NeighborGraph, ControlError> {
    let mut queue = VecDeque::from([(seed.id.clone(), 0_u8)]);
    let mut visited = HashSet::from([seed.id.clone()]);
    let mut node_ids = HashSet::from([seed.id.clone()]);
    let mut edge_map = HashMap::<String, StoredGraphEdge>::new();
    let mut partial = false;

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let edges = storage
            .graph_edges_for_node(project_id, branch_id, &node_id, min_confidence, limit + 1)
            .map_err(ControlError::internal)?;

        for edge in filtered_edges(edges, edge_types) {
            if !edge_matches_direction(&edge, &node_id, direction) {
                continue;
            }
            if edge_map.len() >= limit {
                partial = true;
                break;
            }

            let next = if edge.from_node_id == node_id {
                edge.to_node_id.clone()
            } else {
                edge.from_node_id.clone()
            };
            node_ids.insert(edge.from_node_id.clone());
            node_ids.insert(edge.to_node_id.clone());
            edge_map.entry(edge.id.clone()).or_insert(edge);

            if visited.insert(next.clone()) {
                queue.push_back((next, depth + 1));
            }
        }
    }

    let mut sorted_node_ids = node_ids.into_iter().collect::<Vec<_>>();
    sorted_node_ids.sort();
    let nodes = storage
        .graph_nodes_by_ids(project_id, branch_id, &sorted_node_ids)
        .map_err(ControlError::internal)?
        .into_iter()
        .map(GraphNodeDto::from)
        .collect();
    let edges = edge_map
        .into_values()
        .map(GraphEdgeDto::from)
        .collect::<Vec<_>>();

    Ok(NeighborGraph {
        nodes,
        edges,
        partial,
    })
}

#[allow(clippy::too_many_arguments)]
fn shortest_path(
    storage: &SqliteStorage,
    project_id: &str,
    branch_id: &str,
    source: &str,
    target: &str,
    edge_types: &[String],
    max_depth: u8,
    min_confidence: u16,
    limit: usize,
) -> Result<GraphPathResponse, ControlError> {
    let edges = filtered_edges(
        storage
            .graph_edges_scoped(project_id, branch_id, min_confidence, limit)
            .map_err(ControlError::internal)?,
        edge_types,
    );
    let mut adjacency = HashMap::<String, Vec<StoredGraphEdge>>::new();
    for edge in &edges {
        adjacency
            .entry(edge.from_node_id.clone())
            .or_default()
            .push(edge.clone());
    }

    let mut queue = VecDeque::from([(source.to_string(), Vec::<StoredGraphEdge>::new())]);
    let mut visited = HashSet::from([source.to_string()]);

    while let Some((node_id, path_edges)) = queue.pop_front() {
        if node_id == target {
            let mut node_ids = vec![source.to_string()];
            for edge in &path_edges {
                node_ids.push(edge.to_node_id.clone());
            }
            let nodes = storage
                .graph_nodes_by_ids(project_id, branch_id, &node_ids)
                .map_err(ControlError::internal)?
                .into_iter()
                .map(GraphNodeDto::from)
                .collect::<Vec<_>>();
            let confidence_summary = confidence_summary(&path_edges);
            return Ok(GraphPathResponse {
                project_id: project_id.to_string(),
                branch_id: branch_id.to_string(),
                found: true,
                path_length: path_edges.len(),
                nodes,
                edges: path_edges.into_iter().map(GraphEdgeDto::from).collect(),
                confidence_summary,
                reason: None,
            });
        }

        if path_edges.len() >= usize::from(max_depth) {
            continue;
        }

        for edge in adjacency.get(&node_id).cloned().unwrap_or_default() {
            if visited.insert(edge.to_node_id.clone()) {
                let mut next_path = path_edges.clone();
                next_path.push(edge.clone());
                queue.push_back((edge.to_node_id, next_path));
            }
        }
    }

    Ok(GraphPathResponse {
        project_id: project_id.to_string(),
        branch_id: branch_id.to_string(),
        found: false,
        nodes: Vec::new(),
        edges: Vec::new(),
        path_length: 0,
        confidence_summary: None,
        reason: Some("no bounded path found".to_string()),
    })
}

fn cycle_groups(edges: Vec<StoredGraphEdge>) -> Vec<SccGroupDto> {
    let mut adjacency = HashMap::<String, Vec<String>>::new();
    let mut edge_types_by_pair = HashMap::<(String, String), HashSet<String>>::new();
    for edge in edges {
        adjacency
            .entry(edge.from_node_id.clone())
            .or_default()
            .push(edge.to_node_id.clone());
        adjacency.entry(edge.to_node_id.clone()).or_default();
        edge_types_by_pair
            .entry((edge.from_node_id, edge.to_node_id))
            .or_default()
            .insert(edge.edge_type);
    }

    let mut state = TarjanState::default();
    let node_ids = adjacency.keys().cloned().collect::<Vec<_>>();
    for node_id in node_ids {
        if !state.indices.contains_key(&node_id) {
            strong_connect(&node_id, &adjacency, &mut state);
        }
    }

    state
        .components
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|mut node_ids| {
            node_ids.sort();
            let node_set = node_ids.iter().cloned().collect::<HashSet<_>>();
            let mut edge_types = HashSet::new();
            for ((from, to), types) in &edge_types_by_pair {
                if node_set.contains(from) && node_set.contains(to) {
                    edge_types.extend(types.iter().cloned());
                }
            }
            let mut edge_types = edge_types.into_iter().collect::<Vec<_>>();
            edge_types.sort();
            SccGroupDto {
                node_ids,
                edge_types,
            }
        })
        .collect()
}

#[derive(Default)]
struct TarjanState {
    index: usize,
    indices: HashMap<String, usize>,
    lowlinks: HashMap<String, usize>,
    stack: Vec<String>,
    on_stack: HashSet<String>,
    components: Vec<Vec<String>>,
}

fn strong_connect(
    node_id: &str,
    adjacency: &HashMap<String, Vec<String>>,
    state: &mut TarjanState,
) {
    state.indices.insert(node_id.to_string(), state.index);
    state.lowlinks.insert(node_id.to_string(), state.index);
    state.index += 1;
    state.stack.push(node_id.to_string());
    state.on_stack.insert(node_id.to_string());

    for next in adjacency.get(node_id).into_iter().flatten() {
        if !state.indices.contains_key(next) {
            strong_connect(next, adjacency, state);
            let lowlink = state.lowlinks[node_id].min(state.lowlinks[next]);
            state.lowlinks.insert(node_id.to_string(), lowlink);
        } else if state.on_stack.contains(next) {
            let lowlink = state.lowlinks[node_id].min(state.indices[next]);
            state.lowlinks.insert(node_id.to_string(), lowlink);
        }
    }

    if state.lowlinks[node_id] == state.indices[node_id] {
        let mut component = Vec::new();
        while let Some(value) = state.stack.pop() {
            state.on_stack.remove(&value);
            component.push(value.clone());
            if value == node_id {
                break;
            }
        }
        state.components.push(component);
    }
}

fn filtered_edges(edges: Vec<StoredGraphEdge>, edge_types: &[String]) -> Vec<StoredGraphEdge> {
    if edge_types.is_empty() {
        return edges;
    }
    let allowed = edge_types
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    edges
        .into_iter()
        .filter(|edge| allowed.contains(&edge.edge_type.to_ascii_lowercase()))
        .collect()
}

fn edge_matches_direction(
    edge: &StoredGraphEdge,
    node_id: &str,
    direction: GraphDirection,
) -> bool {
    match direction {
        GraphDirection::Inbound => edge.to_node_id == node_id,
        GraphDirection::Outbound => edge.from_node_id == node_id,
        GraphDirection::Both => edge.to_node_id == node_id || edge.from_node_id == node_id,
    }
}

fn confidence_summary(edges: &[StoredGraphEdge]) -> Option<String> {
    if edges.is_empty() {
        return None;
    }
    let min = edges.iter().map(|edge| edge.confidence).min().unwrap_or(0);
    let avg = edges
        .iter()
        .map(|edge| usize::from(edge.confidence))
        .sum::<usize>()
        / edges.len();
    Some(format!("min={min}; avg={avg}"))
}

fn checked_graph_limit(limit: Option<usize>) -> Result<usize, ControlError> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 {
        return Err(ControlError::bad_request("limit must be greater than zero"));
    }
    if limit > MAX_LIMIT {
        return Err(ControlError::bad_request(format!(
            "limit must be less than or equal to {MAX_LIMIT}"
        )));
    }
    Ok(limit)
}

fn checked_graph_depth(depth: Option<u8>) -> Result<u8, ControlError> {
    let depth = depth.unwrap_or(DEFAULT_GRAPH_DEPTH);
    if depth == 0 {
        return Err(ControlError::bad_request(
            "max_depth must be greater than zero",
        ));
    }
    if depth > MAX_GRAPH_DEPTH {
        return Err(ControlError::bad_request(format!(
            "max_depth must be less than or equal to {MAX_GRAPH_DEPTH}"
        )));
    }
    Ok(depth)
}

fn checked_confidence(confidence: Option<u16>) -> Result<u16, ControlError> {
    let confidence = confidence.unwrap_or(0);
    if confidence > 10_000 {
        return Err(ControlError::bad_request(
            "min_confidence must be less than or equal to 10000",
        ));
    }
    Ok(confidence)
}

#[derive(Debug, Serialize)]
struct SavingsSummaryResponse {
    records: usize,
    estimated_tokens_saved: usize,
    returned_tokens: usize,
    avoided_file_reads: usize,
    avoided_search_calls: usize,
    partial: bool,
}

impl From<SavingsSummary> for SavingsSummaryResponse {
    fn from(value: SavingsSummary) -> Self {
        Self {
            records: value.records,
            estimated_tokens_saved: value.estimated_tokens_saved,
            returned_tokens: value.returned_tokens,
            avoided_file_reads: value.avoided_file_reads,
            avoided_search_calls: value.avoided_search_calls,
            partial: false,
        }
    }
}

struct SqliteIndexStore {
    storage: StdMutex<SqliteStorage>,
}

impl IndexStore for SqliteIndexStore {
    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
        let storage = self
            .storage
            .lock()
            .map_err(|_| ContractError::new("sqlite index store lock poisoned"))?;
        b3_core::FileRepository::get_file(&*storage, file_id)
    }
    fn ensure_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        root_path: &str,
    ) -> ContractResult<()> {
        let storage = self
            .storage
            .lock()
            .map_err(|_| ContractError::new("sqlite index store lock poisoned"))?;
        storage.ensure_project_branch(project_id, branch_id, root_path)
    }

    fn cleanup_deleted_files(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        live_file_ids: &[FileId],
    ) -> ContractResult<()> {
        let storage = self
            .storage
            .lock()
            .map_err(|_| ContractError::new("sqlite index store lock poisoned"))?;
        storage.cleanup_deleted_files(project_id, branch_id, live_file_ids)
    }

    fn upsert_indexed_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file: b3_core::IndexedFileRecord,
    ) -> ContractResult<()> {
        let storage = self
            .storage
            .lock()
            .map_err(|_| ContractError::new("sqlite index store lock poisoned"))?;
        storage.upsert_indexed_file(project_id, branch_id, file)
    }

    fn remove_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        path: &str,
    ) -> ContractResult<()> {
        self.storage
            .lock()
            .map_err(|_| ContractError::new("sqlite index store lock poisoned"))?
            .remove_file_by_path(project_id, branch_id, path)
    }
}

#[derive(Clone)]
struct EventForwarder {
    events: EventHub,
}

impl EventBus for EventForwarder {
    fn publish(&self, event: DomainEvent) -> ContractResult<()> {
        match event {
            DomainEvent::IndexStarted(event) => self.events.emit(
                "indexing_started",
                json!({"project_id": event.project_id.as_str(), "root_path": event.root_path}),
            ),
            DomainEvent::FileParsed(event) => self.events.emit(
                "file_indexed",
                json!({"project_id": event.project_id.as_str(), "file_id": event.file_id.as_str(), "symbols_found": event.symbols_found}),
            ),
            DomainEvent::FileSkipped(event) => self.events.emit(
                "file_skipped",
                json!({"project_id": event.project_id.as_str(), "path": event.path, "reason": event.reason}),
            ),
            DomainEvent::IndexCompleted(event) => self.events.emit(
                "indexing_completed",
                json!({"project_id": event.project_id.as_str(), "files_seen": event.files_seen, "files_parsed": event.files_parsed}),
            ),
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use b3_core::{
        BranchId, BranchMetadata, EdgeConfidence, EdgeId, EdgeKind, EdgeProvenance, FileId,
        FileRecord, GraphEdge, GraphEdgeMetadata, GraphNode, NodeId, NodeKind, SymbolId,
        SymbolRecord,
    };
    use b3_storage::NewCentralityRecord;
    use http::{Request, StatusCode};
    use tempfile::tempdir;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .upsert_project(&project_id, "Project", ".")
            .expect("project");
        storage
            .upsert_branch(&branch_id, &project_id, &BranchMetadata::new("main"))
            .expect("branch");
        let file = FileRecord {
            id: FileId::new("file"),
            project_id: project_id.clone(),
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
        };
        storage.upsert_file(&file, &branch_id).expect("file");
        storage
            .upsert_symbol(
                &project_id,
                &branch_id,
                &SymbolRecord::new(SymbolId::new("symbol"), file.id, "run", NodeKind::Function),
            )
            .expect("symbol");

        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn empty_app() -> Router {
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("open storage"),
        ))
    }

    fn graph_app(with_centrality: bool, with_other_branch: bool) -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        seed_graph(&storage, "project", "main", with_centrality);
        if with_other_branch {
            seed_graph(&storage, "project", "dev", false);
        }

        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn seed_graph(
        storage: &SqliteStorage,
        project: &'static str,
        branch: &'static str,
        with_centrality: bool,
    ) {
        let project_id = ProjectId::new(project);
        let branch_id = BranchId::new(branch);
        storage
            .upsert_project(&project_id, "Project", ".")
            .expect("project");
        storage
            .upsert_branch(&branch_id, &project_id, &BranchMetadata::new(branch))
            .expect("branch");
        let file = FileRecord {
            id: FileId::new(format!("{branch}-file")),
            project_id: project_id.clone(),
            path: format!("{branch}/src/lib.rs"),
            content_hash: "hash".to_string(),
        };
        storage.upsert_file(&file, &branch_id).expect("file");
        let symbol = SymbolRecord::new(
            SymbolId::new(format!("{branch}-symbol-a")),
            file.id.clone(),
            "alpha",
            NodeKind::Function,
        );
        storage
            .upsert_symbol(&project_id, &branch_id, &symbol)
            .expect("symbol");

        for (id, label) in [
            (format!("{branch}-a"), "alpha"),
            (format!("{branch}-b"), "beta"),
            (format!("{branch}-c"), "gamma"),
        ] {
            storage
                .upsert_node(
                    &GraphNode {
                        id: NodeId::new(id),
                        project_id: project_id.clone(),
                        label: label.to_string(),
                    },
                    &branch_id,
                    NodeKind::Function,
                )
                .expect("node");
        }

        for (id, from, to) in [
            (
                format!("{branch}-edge-ab"),
                format!("{branch}-a"),
                format!("{branch}-b"),
            ),
            (
                format!("{branch}-edge-bc"),
                format!("{branch}-b"),
                format!("{branch}-c"),
            ),
            (
                format!("{branch}-edge-ca"),
                format!("{branch}-c"),
                format!("{branch}-a"),
            ),
        ] {
            storage
                .upsert_edge(
                    &project_id,
                    &branch_id,
                    &GraphEdge {
                        id: EdgeId::new(id),
                        from: NodeId::new(from),
                        to: NodeId::new(to),
                        metadata: GraphEdgeMetadata {
                            confidence: EdgeConfidence::from_basis_points(9_000),
                            provenance: EdgeProvenance::Ast,
                            created_at_unix_ms: 1,
                            updated_at_unix_ms: 1,
                        },
                    },
                    EdgeKind::Calls,
                )
                .expect("edge");
        }

        if with_centrality {
            storage
                .insert_centrality_snapshot(&NewCentralityRecord {
                    project_id: project.to_string(),
                    branch_id: branch.to_string(),
                    node_id: format!("{branch}-a"),
                    symbol_id: Some(format!("{branch}-symbol-a")),
                    name: "alpha".to_string(),
                    kind: "function".to_string(),
                    pagerank_score: 0.42,
                    in_degree: 1,
                    out_degree: 1,
                    fan_in: 1,
                    fan_out: 1,
                    degree_centrality: 0.5,
                    component_size: 3,
                    is_cycle_member: true,
                    calculated_at_unix_ms: 7,
                    algorithm_version: "test-v1".to_string(),
                })
                .expect("centrality");
        }
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        serde_json::from_slice(&bytes).expect("json body")
    }

    async fn post_json(app: Router, uri: &str, body: &str) -> Response {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response")
    }

    #[tokio::test]
    async fn health_endpoint_returns_json() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["offline_mode"], true);
    }

    #[tokio::test]
    async fn status_endpoint_includes_storage_counts() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["indexed_file_count"], 1);
        assert_eq!(body["symbol_count"], 1);
        assert_eq!(body["offline_mode"], true);
    }

    #[tokio::test]
    async fn find_symbol_validates_nested_scope() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/query/find-symbol")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"symbol":"run"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert!(body["error"]["message"]
            .as_str()
            .expect("message")
            .contains("scope"));
    }

    #[tokio::test]
    async fn find_symbol_returns_matches_without_full_file_dump() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/query/find-symbol")
                    .header(http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"symbol":"run","scope":{"project_id":"project"},"limit":10}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["matches"].as_array().expect("matches").len(), 1);
        assert_eq!(body["full_file_dump_included"], false);
    }

    #[tokio::test]
    async fn graph_summary_empty_db_returns_zero_counts() {
        let response = empty_app()
            .oneshot(
                Request::builder()
                    .uri("/api/graph/summary")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["full_graph_dump_included"], false);
        assert_eq!(body["max_depth"], u64::from(MAX_GRAPH_DEPTH));
        assert_eq!(body["node_count"], 0);
        assert_eq!(body["edge_count"], 0);
    }

    #[tokio::test]
    async fn graph_summary_with_indexed_data_returns_real_counts() {
        let response = graph_app(false, false)
            .oneshot(
                Request::builder()
                    .uri("/api/graph/summary")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["node_count"], 3);
        assert_eq!(body["edge_count"], 3);
        assert_eq!(body["symbol_count"], 1);
        assert_eq!(body["file_count"], 1);
        assert_eq!(body["edge_type_counts"][0]["name"], "calls");
    }

    #[tokio::test]
    async fn neighbors_returns_bounded_response() {
        let response = post_json(
            graph_app(false, false),
            "/api/graph/neighbors",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"seed_node_id":"main-a","direction":"outbound","max_depth":1,"limit":1}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(body["nodes"].as_array().expect("nodes").len() <= 2);
        assert_eq!(body["edges"].as_array().expect("edges").len(), 1);
        assert_eq!(body["full_graph_dump_included"], false);
    }

    #[tokio::test]
    async fn neighbors_respects_branch_isolation() {
        let response = post_json(
            graph_app(false, true),
            "/api/graph/neighbors",
            r#"{"scope":{"project_id":"project","branch_id":"dev"},"seed_node_id":"dev-a","direction":"outbound","max_depth":1,"limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let nodes = body["nodes"].as_array().expect("nodes");
        assert!(nodes
            .iter()
            .all(|node| node["branch_id"].as_str().expect("branch") == "dev"));
    }

    #[tokio::test]
    async fn path_found_returns_ordered_graph() {
        let response = post_json(
            graph_app(false, false),
            "/api/graph/path",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"source_node_id":"main-a","target_node_id":"main-c","max_depth":2,"limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["found"], true);
        assert_eq!(body["path_length"], 2);
        assert_eq!(body["edges"].as_array().expect("edges").len(), 2);
    }

    #[tokio::test]
    async fn path_not_found_returns_reason() {
        let response = post_json(
            graph_app(false, false),
            "/api/graph/path",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"source_node_id":"main-a","target_node_id":"missing","max_depth":2,"limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["found"], false);
        assert!(body["reason"]
            .as_str()
            .expect("reason")
            .contains("no bounded"));
    }

    #[tokio::test]
    async fn cycles_endpoint_returns_scc_groups() {
        let response = post_json(
            graph_app(false, false),
            "/api/graph/cycles",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["cycle_count"], 1);
        assert_eq!(
            body["scc_groups"][0]["node_ids"]
                .as_array()
                .expect("ids")
                .len(),
            3
        );
    }

    #[tokio::test]
    async fn centrality_empty_snapshot_returns_clear_message() {
        let response = post_json(
            graph_app(false, false),
            "/api/graph/centrality",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["nodes"].as_array().expect("nodes").len(), 0);
        assert_eq!(body["calculated"], false);
    }

    #[tokio::test]
    async fn centrality_snapshot_returned_when_available() {
        let response = post_json(
            graph_app(true, false),
            "/api/graph/centrality",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["nodes"][0]["node_id"], "main-a");
        assert_eq!(body["nodes"][0]["algorithm_version"], "test-v1");
    }

    #[tokio::test]
    async fn graph_endpoints_validate_invalid_scope() {
        let response = post_json(
            graph_app(false, false),
            "/api/graph/neighbors",
            r#"{"seed_node_id":"main-a","limit":50}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn graph_endpoints_validate_depth_and_limit() {
        let too_deep = post_json(
            graph_app(false, false),
            "/api/graph/neighbors",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"seed_node_id":"main-a","max_depth":4,"limit":50}"#,
        )
        .await;
        assert_eq!(too_deep.status(), StatusCode::BAD_REQUEST);

        let too_large = post_json(
            graph_app(false, false),
            "/api/graph/neighbors",
            r#"{"scope":{"project_id":"project","branch_id":"main"},"seed_node_id":"main-a","max_depth":1,"limit":201}"#,
        )
        .await;
        assert_eq!(too_large.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn local_bind_is_default_and_non_local_requires_opt_in() {
        let default_options = ServeOptions::default();
        assert_eq!(default_options.bind_addr.ip(), DEFAULT_HOST);
        assert!(default_options.validate().is_ok());

        let mut non_local = default_options;
        non_local.bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_PORT);
        assert!(non_local.validate().is_err());
        non_local.allow_non_local_bind = true;
        assert!(non_local.validate().is_ok());
    }

    #[test]
    fn state_opens_local_database_path() {
        let dir = tempdir().expect("temp dir");
        let options = ServeOptions {
            project_path: dir.path().to_path_buf(),
            database_path: dir.path().join("b3.db"),
            ..ServeOptions::default()
        };

        let state = ControlState::new(&options).expect("state");
        assert_eq!(
            path_string(&state.database_path),
            options.database_path.to_string_lossy()
        );
    }

    #[test]
    fn server_event_serializes_for_sse_payloads() {
        let event = ServerEvent {
            event_type: "file_changed".to_string(),
            payload: json!({"path": "src/lib.rs"}),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("file_changed"));
        assert!(json.contains("src/lib.rs"));
    }
}
