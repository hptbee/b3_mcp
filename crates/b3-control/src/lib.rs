//! Control server and localhost API boundary.
#![recursion_limit = "256"]
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
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Json, Path, Query, State},
    http::{HeaderValue, Method, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use b3_core::{
    default_language_backend_registry, AppConfig, ArchitectureCapabilityStatus, BranchId,
    BranchMetadata, ContractError, ContractResult, EditRequest, EventBus, IndexScope,
    IndexScopeKind, IndexSummary, Indexer, ParserIsolationMode, ProjectId, QueryRequest,
    QueryResult, QueryScope as CoreQueryScope, RenameRequest, ScopePreview, SourceKind,
    SymbolRepository, PRODUCT_NAME,
};
use b3_indexer::{
    lsp::LspBackend,
    scope::{parse_scope, plan_scope, ScopeTarget, ScopeTargetProvider},
    DefaultLanguagePack, IndexerConfig, LocalIndexer, NotifyFileWatcher, ParserIsolation,
    WatchConfig, WatchEventKind,
};
use b3_mcp_runtime::{runtime_info, RuntimeResponsibility};
use b3_query::architecture::{
    package_matching::DependencyMatchKindFilter, ArchitectureGraphRequest, DependencyMatchOptions,
    GraphConfidenceFilter, GroupFederation, GroupImpactRequest, MessageMatchOptions,
    RouteMatchOptions, ServiceMapRequest,
};
use b3_query::hybrid::{HybridRankingExplanation, HybridSearchEngine, HybridSearchRequest};
use b3_query::{editing::SymbolicEditEngine, refactor::RenameRefactorEngine};
use b3_storage::{
    SavingsSummary, SharedSqliteIndexStore, SqliteStorage, StorageStats, StoredCentralityRecord,
    StoredComponent, StoredDataAccess, StoredGraphEdge, StoredGraphNode, StoredInfrastructure,
    StoredMessaging, StoredParseFailure, StoredRealtime, StoredRoute, StoredWpf,
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

mod routes;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCommandOptions {
    pub project_path: PathBuf,
    pub database_path: PathBuf,
    pub scope: Option<String>,
    pub dry_run: bool,
    pub force: bool,
}

impl Default for ProjectCommandOptions {
    fn default() -> Self {
        Self {
            project_path: PathBuf::from("."),
            database_path: PathBuf::from(".b3").join("b3.db"),
            scope: None,
            dry_run: false,
            force: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ManualIndexSummary {
    pub project_path: String,
    pub database_path: String,
    pub files_discovered: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub symbols_indexed: usize,
    pub edges_indexed: usize,
    pub parse_failures: usize,
    pub duration_ms: u128,
    pub reindex: bool,
    pub behavior: String,
    pub scope: Option<String>,
    pub dry_run: bool,
    pub preview: Option<ScopePreview>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStatusResponse {
    pub status: String,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub duration_ms: Option<u128>,
    pub files_discovered: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub symbols_indexed: usize,
    pub edges_indexed: usize,
    pub parse_failures: usize,
    pub last_error: Option<String>,
}

impl Default for IndexStatusResponse {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            started_at: None,
            completed_at: None,
            duration_ms: None,
            files_discovered: 0,
            files_indexed: 0,
            files_skipped: 0,
            symbols_indexed: 0,
            edges_indexed: 0,
            parse_failures: 0,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IndexRunRequest {
    pub scope: Option<String>,
    pub dry_run: Option<bool>,
    pub force: Option<bool>,
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
    registry_path: Arc<PathBuf>,
    storage: Arc<Mutex<SqliteStorage>>,
    app_config: Arc<AppConfig>,
    events: EventHub,
    index_status: Arc<Mutex<IndexStatusResponse>>,
}

impl ControlState {
    pub fn new(options: &ServeOptions) -> Result<Self, ControlError> {
        options.validate()?;
        let storage =
            SqliteStorage::open(&options.database_path).map_err(ControlError::internal)?;
        Ok(Self {
            project_path: Arc::new(options.project_path.clone()),
            database_path: Arc::new(options.database_path.clone()),
            registry_path: Arc::new(b3_query::architecture::default_registry_path()),
            storage: Arc::new(Mutex::new(storage)),
            app_config: Arc::new(AppConfig::default()),
            events: EventHub::new(256),
            index_status: Arc::new(Mutex::new(IndexStatusResponse::default())),
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
            registry_path: Arc::new(b3_query::architecture::default_registry_path()),
            storage: Arc::new(Mutex::new(storage)),
            app_config: Arc::new(AppConfig::default()),
            events: EventHub::new(256),
            index_status: Arc::new(Mutex::new(IndexStatusResponse::default())),
        }
    }

    #[cfg(test)]
    fn from_storage_with_registry_path(
        project_path: PathBuf,
        database_path: PathBuf,
        storage: SqliteStorage,
        registry_path: PathBuf,
    ) -> Self {
        Self {
            project_path: Arc::new(project_path),
            database_path: Arc::new(database_path),
            registry_path: Arc::new(registry_path),
            storage: Arc::new(Mutex::new(storage)),
            app_config: Arc::new(AppConfig::default()),
            events: EventHub::new(256),
            index_status: Arc::new(Mutex::new(IndexStatusResponse::default())),
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
    routes::router().layer(localhost_cors()).with_state(state)
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

pub fn init_project(options: &ProjectCommandOptions) -> Result<(), ControlError> {
    if let Some(parent) = options.database_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(ControlError::internal)?;
        }
    }
    let storage = SqliteStorage::open(&options.database_path).map_err(ControlError::internal)?;
    let project_id = ProjectId::new("default");
    let branch_id = BranchId::new("main");
    storage
        .upsert_project(
            &project_id,
            "default",
            &options.project_path.to_string_lossy(),
        )
        .map_err(ControlError::internal)?;
    storage
        .upsert_branch(&branch_id, &project_id, &BranchMetadata::new("main"))
        .map_err(ControlError::internal)?;
    Ok(())
}

pub fn index_project(
    options: &ProjectCommandOptions,
    reindex: bool,
) -> Result<ManualIndexSummary, ControlError> {
    run_index_project(options, reindex, EventHub::new(1))
}

async fn run_index_for_state(
    state: ControlState,
    reindex: bool,
    request: Option<IndexRunRequest>,
) -> Result<ManualIndexSummary, ControlError> {
    let started_at = now_unix_ms();
    {
        let mut status = state.index_status.lock().await;
        *status = IndexStatusResponse {
            status: "running".to_string(),
            started_at: Some(started_at),
            completed_at: None,
            duration_ms: None,
            ..IndexStatusResponse::default()
        };
    }
    state.events.emit(
        "indexing_started",
        json!({"project_path": path_string(&state.project_path), "reindex": reindex}),
    );

    let options = ProjectCommandOptions {
        project_path: (*state.project_path).clone(),
        database_path: (*state.database_path).clone(),
        scope: request.as_ref().and_then(|request| request.scope.clone()),
        dry_run: request
            .as_ref()
            .and_then(|request| request.dry_run)
            .unwrap_or(false),
        force: request
            .as_ref()
            .and_then(|request| request.force)
            .unwrap_or(false),
    };
    let started = Instant::now();
    let result = run_index_with_events(&options, reindex, state.events.clone());
    match result {
        Ok(summary) => {
            let completed_at = now_unix_ms();
            let mut status = state.index_status.lock().await;
            *status = IndexStatusResponse {
                status: "completed".to_string(),
                started_at: Some(started_at),
                completed_at: Some(completed_at),
                duration_ms: Some(started.elapsed().as_millis()),
                files_discovered: summary.files_discovered,
                files_indexed: summary.files_indexed,
                files_skipped: summary.files_skipped,
                symbols_indexed: summary.symbols_indexed,
                edges_indexed: summary.edges_indexed,
                parse_failures: summary.parse_failures,
                last_error: None,
            };
            state.events.emit("indexing_completed", json!(summary));
            Ok(summary)
        }
        Err(error) => {
            let message = error.message.clone();
            let mut status = state.index_status.lock().await;
            *status = IndexStatusResponse {
                status: "failed".to_string(),
                started_at: Some(started_at),
                completed_at: Some(now_unix_ms()),
                duration_ms: Some(started.elapsed().as_millis()),
                last_error: Some(message.clone()),
                ..IndexStatusResponse::default()
            };
            state
                .events
                .emit("indexing_failed", json!({"error": message}));
            Err(error)
        }
    }
}

fn run_index_with_events(
    options: &ProjectCommandOptions,
    reindex: bool,
    events: EventHub,
) -> Result<ManualIndexSummary, ControlError> {
    run_index_project(options, reindex, events)
}

fn run_index_project(
    options: &ProjectCommandOptions,
    reindex: bool,
    events: EventHub,
) -> Result<ManualIndexSummary, ControlError> {
    let started = Instant::now();
    let project_id = ProjectId::new("default");
    let branch_id = BranchId::new("main");
    if options.dry_run {
        let scope_text = options.scope.as_deref().unwrap_or("project");
        let mut scope = parse_scope(scope_text).map_err(ControlError::from_scope)?;
        scope.dry_run = true;
        scope.force = options.force;
        scope.project_id = Some(project_id.as_str().to_string());
        scope.branch_id = Some(branch_id.as_str().to_string());
        let preview = if options.database_path.exists() {
            let storage =
                SqliteStorage::open(&options.database_path).map_err(ControlError::internal)?;
            let provider = StorageScopeTargetProvider { storage: &storage };
            plan_scope(
                &options.project_path,
                project_id.as_str(),
                branch_id.as_str(),
                scope,
                &IndexerConfig::default().ignore,
                &provider,
            )
            .map_err(ControlError::from_scope)?
            .preview
        } else {
            plan_scope(
                &options.project_path,
                project_id.as_str(),
                branch_id.as_str(),
                scope,
                &IndexerConfig::default().ignore,
                &b3_indexer::scope::EmptyScopeTargetProvider,
            )
            .map_err(ControlError::from_scope)?
            .preview
        };
        return Ok(index_summary_response(
            options,
            IndexSummary {
                files_seen: preview.matched_files,
                files_parsed: 0,
                symbols_indexed: 0,
            },
            0,
            0,
            started.elapsed(),
            reindex,
            Some(preview),
        ));
    }

    init_project(options)?;
    let before_failures = SqliteStorage::open(&options.database_path)
        .map_err(ControlError::internal)?
        .parse_failure_count(Some(project_id.as_str()), Some(branch_id.as_str()))
        .map_err(ControlError::internal)?;
    let storage = SqliteStorage::open(&options.database_path).map_err(ControlError::internal)?;
    let scoped_plan = if let Some(scope_text) = options.scope.as_deref() {
        let mut scope = parse_scope(scope_text).map_err(ControlError::from_scope)?;
        scope.dry_run = options.dry_run;
        scope.force = options.force;
        scope.project_id = Some(project_id.as_str().to_string());
        scope.branch_id = Some(branch_id.as_str().to_string());
        let provider = StorageScopeTargetProvider { storage: &storage };
        let plan = plan_scope(
            &options.project_path,
            project_id.as_str(),
            branch_id.as_str(),
            scope,
            &IndexerConfig::default().ignore,
            &provider,
        )
        .map_err(ControlError::from_scope)?;
        let preview = plan.preview.clone();
        Some((plan, preview))
    } else {
        None
    };
    let indexer = LocalIndexer::new(
        DefaultLanguagePack,
        SharedSqliteIndexStore::new(storage),
        EventForwarder {
            events: events.clone(),
        },
        IndexerConfig {
            branch_id: branch_id.clone(),
            parser_isolation: ParserIsolation::InProcess,
            ..IndexerConfig::default()
        },
    );
    let (summary, preview) = if let Some((plan, preview)) = scoped_plan {
        (
            indexer.index_scope(plan).map_err(ControlError::internal)?,
            Some(preview),
        )
    } else {
        (
            indexer
                .index(b3_core::IndexJob {
                    project_id: project_id.clone(),
                    root_path: options.project_path.to_string_lossy().to_string(),
                })
                .map_err(ControlError::internal)?,
            None,
        )
    };
    let storage = SqliteStorage::open(&options.database_path).map_err(ControlError::internal)?;
    let graph = storage
        .graph_summary(Some(project_id.as_str()), Some(branch_id.as_str()))
        .map_err(ControlError::internal)?;
    let parse_failures = storage
        .parse_failure_count(Some(project_id.as_str()), Some(branch_id.as_str()))
        .map_err(ControlError::internal)?
        .saturating_sub(before_failures);
    Ok(index_summary_response(
        options,
        summary,
        graph.edge_count,
        parse_failures,
        started.elapsed(),
        reindex,
        preview,
    ))
}

fn index_summary_response(
    options: &ProjectCommandOptions,
    summary: IndexSummary,
    edges_indexed: usize,
    parse_failures: usize,
    duration: Duration,
    reindex: bool,
    preview: Option<ScopePreview>,
) -> ManualIndexSummary {
    let scope = options.scope.clone();
    ManualIndexSummary {
        project_path: path_string(&options.project_path),
        database_path: path_string(&options.database_path),
        files_discovered: summary.files_seen,
        files_indexed: summary.files_parsed,
        files_skipped: summary.files_seen.saturating_sub(summary.files_parsed),
        symbols_indexed: summary.symbols_indexed,
        edges_indexed,
        parse_failures,
        duration_ms: duration.as_millis(),
        reindex,
        behavior: if reindex {
            if scope.is_some() {
                "scoped incremental reindex: only matched files are considered; unrelated indexed files are preserved".to_string()
            } else {
                "safe incremental reindex: unchanged files are skipped; deleted files are cleaned for the current branch".to_string()
            }
        } else {
            "incremental index: unchanged files are skipped by content hash".to_string()
        },
        scope,
        dry_run: options.dry_run,
        preview,
    }
}

struct StorageScopeTargetProvider<'a> {
    storage: &'a SqliteStorage,
}

impl ScopeTargetProvider for StorageScopeTargetProvider<'_> {
    fn targets(
        &self,
        scope: &IndexScope,
        project_id: &str,
        branch_id: &str,
        limit: usize,
    ) -> Result<Vec<ScopeTarget>, b3_core::ScopeError> {
        let value = scope.value.as_deref().unwrap_or_default();
        let targets = match scope.kind {
            IndexScopeKind::Route => self
                .storage
                .routes(project_id, branch_id, None, None, Some(value), limit)
                .map_err(scope_storage_error)?
                .into_iter()
                .map(|route| ScopeTarget {
                    label: format!("route:{} {}", route.method, route.path),
                    file_path: route.file_path,
                    language: None,
                    framework: Some(route.framework),
                    estimated_symbols: 1,
                })
                .collect(),
            IndexScopeKind::Component => self
                .storage
                .components(project_id, branch_id, None, Some(value), None, limit)
                .map_err(scope_storage_error)?
                .into_iter()
                .map(|component| ScopeTarget {
                    label: format!("component:{}", component.name),
                    file_path: component.file_path,
                    language: None,
                    framework: Some(component.framework),
                    estimated_symbols: 1,
                })
                .collect(),
            IndexScopeKind::Module => self
                .storage
                .components(
                    project_id,
                    branch_id,
                    Some("angular"),
                    Some(value),
                    None,
                    limit,
                )
                .map_err(scope_storage_error)?
                .into_iter()
                .map(|component| ScopeTarget {
                    label: format!("module:{}", component.name),
                    file_path: component.file_path,
                    language: None,
                    framework: Some(component.framework),
                    estimated_symbols: 1,
                })
                .collect(),
            IndexScopeKind::DataAccess => self
                .storage
                .data_access(project_id, branch_id, Some(value), None, None, None, limit)
                .map_err(scope_storage_error)?
                .into_iter()
                .map(|record| ScopeTarget {
                    label: format!("data_access:{}:{}", record.technology, record.kind),
                    file_path: record.file_path,
                    language: None,
                    framework: Some(record.technology),
                    estimated_symbols: 1,
                })
                .collect(),
            IndexScopeKind::Realtime => self
                .storage
                .realtime(project_id, branch_id, Some(value), None, None, None, limit)
                .map_err(scope_storage_error)?
                .into_iter()
                .map(|record| ScopeTarget {
                    label: format!(
                        "realtime:{}:{}",
                        record.technology,
                        record
                            .event_name
                            .or(record.hub_name)
                            .or(record.method_name)
                            .unwrap_or(record.kind)
                    ),
                    file_path: record.file_path,
                    language: None,
                    framework: Some(record.technology),
                    estimated_symbols: 1,
                })
                .collect(),
            IndexScopeKind::Messaging => {
                messaging_targets(self.storage, project_id, branch_id, value, limit)?
            }
            IndexScopeKind::Infrastructure => self
                .storage
                .infrastructure(project_id, branch_id, Some(value), None, None, limit)
                .map_err(scope_storage_error)?
                .into_iter()
                .map(|record| ScopeTarget {
                    label: format!(
                        "infrastructure:{}:{}",
                        record.technology,
                        record.name.unwrap_or(record.kind)
                    ),
                    file_path: record.file_path,
                    language: None,
                    framework: Some(record.technology),
                    estimated_symbols: 1,
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(targets)
    }
}

fn messaging_targets(
    storage: &SqliteStorage,
    project_id: &str,
    branch_id: &str,
    value: &str,
    limit: usize,
) -> Result<Vec<ScopeTarget>, b3_core::ScopeError> {
    let (topic, queue, routing_key) = match value.split_once('=') {
        Some(("topic", value)) => (Some(value), None, None),
        Some(("queue", value)) => (None, Some(value), None),
        Some(("routing_key", value)) => (None, None, Some(value)),
        Some((_field, _value)) => (None, None, None),
        None => (None, None, None),
    };
    Ok(storage
        .messaging(
            project_id,
            branch_id,
            None,
            None,
            topic,
            queue,
            routing_key,
            limit,
        )
        .map_err(scope_storage_error)?
        .into_iter()
        .filter(|record| match value.split_once('=') {
            Some(("exchange", value)) => record.exchange.as_deref() == Some(value),
            Some(("pattern", value)) => record.pattern.as_deref() == Some(value),
            Some(_) => true,
            None => record.technology == value,
        })
        .map(|record| ScopeTarget {
            label: format!(
                "messaging:{}:{}",
                record.technology,
                record
                    .topic
                    .or(record.queue)
                    .or(record.routing_key)
                    .or(record.exchange)
                    .or(record.pattern)
                    .unwrap_or(record.kind)
            ),
            file_path: record.file_path,
            language: None,
            framework: Some(record.technology),
            estimated_symbols: 1,
        })
        .collect())
}

fn scope_storage_error(error: impl std::fmt::Display) -> b3_core::ScopeError {
    b3_core::ScopeError::new("scope_metadata_error", error.to_string())
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
                DefaultLanguagePack,
                SharedSqliteIndexStore::new(storage),
                EventForwarder {
                    events: events.clone(),
                },
                IndexerConfig {
                    branch_id,
                    parser_isolation: ParserIsolation::InProcess,
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

    fn from_scope(error: b3_core::ScopeError) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
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

fn parser_isolation_mode(value: ParserIsolationMode) -> &'static str {
    match value {
        ParserIsolationMode::InProcess => "in_process",
        ParserIsolationMode::SubprocessWorker => "subprocess_worker",
    }
}

#[derive(Debug, Serialize)]
struct ParseFailureDto {
    failure_id: String,
    project_id: String,
    branch_id: String,
    file_id: String,
    file_path: String,
    file_hash: String,
    language: Option<String>,
    error_kind: String,
    error_message: String,
    stderr_excerpt: Option<String>,
    failed_at_unix_ms: u64,
    retry_count: usize,
}

impl From<StoredParseFailure> for ParseFailureDto {
    fn from(value: StoredParseFailure) -> Self {
        Self {
            failure_id: value.failure_id,
            project_id: value.project_id,
            branch_id: value.branch_id,
            file_id: value.file_id,
            file_path: value.file_path,
            file_hash: value.file_hash,
            language: value.language,
            error_kind: value.error_kind,
            error_message: value.error_message,
            stderr_excerpt: value.stderr_excerpt,
            failed_at_unix_ms: value.failed_at_unix_ms,
            retry_count: value.retry_count,
        }
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
struct HybridSearchApiRequest {
    query: String,
    project_id: Option<String>,
    branch_id: Option<String>,
    language: Option<String>,
    framework: Option<String>,
    source_kind: Option<String>,
    path_prefix: Option<String>,
    limit: Option<usize>,
    lexical_weight: Option<f32>,
    vector_weight: Option<f32>,
    metadata_weight: Option<f32>,
    min_score: Option<f32>,
    explain: Option<bool>,
    provider_id: Option<String>,
    dimension: Option<usize>,
}

impl HybridSearchApiRequest {
    fn validate(&self) -> Result<(), ControlError> {
        validate_non_empty("query", &self.query)?;
        if let Some(limit) = self.limit {
            if limit == 0 || limit > MAX_LIMIT {
                return Err(ControlError::bad_request(format!(
                    "limit must be between 1 and {MAX_LIMIT}"
                )));
            }
        }
        validate_optional_weight("lexical_weight", self.lexical_weight)?;
        validate_optional_weight("vector_weight", self.vector_weight)?;
        validate_optional_weight("metadata_weight", self.metadata_weight)?;
        validate_min_score(self.min_score)?;
        validate_path_prefix(self.path_prefix.as_deref())?;
        if let Some(source_kind) = &self.source_kind {
            parse_source_kind(source_kind)?;
        }
        Ok(())
    }
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

fn validate_non_empty(field: &str, value: &str) -> Result<(), ControlError> {
    if value.trim().is_empty() {
        Err(ControlError::bad_request(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

fn validate_optional_weight(field: &str, value: Option<f32>) -> Result<(), ControlError> {
    if let Some(value) = value {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ControlError::bad_request(format!(
                "{field} must be a finite value between 0.0 and 1.0"
            )));
        }
    }
    Ok(())
}

fn validate_min_score(value: Option<f32>) -> Result<(), ControlError> {
    if let Some(value) = value {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ControlError::bad_request(
                "min_score must be a finite value between 0.0 and 1.0",
            ));
        }
    }
    Ok(())
}

fn validate_path_prefix(value: Option<&str>) -> Result<(), ControlError> {
    if let Some(value) = value {
        if value.trim().is_empty()
            || value.contains("..")
            || value.starts_with('/')
            || value.starts_with('\\')
            || value.contains(':')
        {
            return Err(ControlError::bad_request(
                "path_prefix must be a relative path prefix without traversal",
            ));
        }
    }
    Ok(())
}

fn parse_source_kind(value: &str) -> Result<SourceKind, ControlError> {
    match value {
        "FileChunk" | "file" | "file_chunk" => Ok(SourceKind::FileChunk),
        "SymbolChunk" | "symbol" | "symbol_chunk" => Ok(SourceKind::SymbolChunk),
        "RouteChunk" | "route" | "route_chunk" => Ok(SourceKind::RouteChunk),
        "ComponentChunk" | "component" | "component_chunk" => Ok(SourceKind::ComponentChunk),
        "DataAccessChunk" | "data_access" | "data_access_chunk" => Ok(SourceKind::DataAccessChunk),
        "RealtimeChunk" | "realtime" | "realtime_chunk" => Ok(SourceKind::RealtimeChunk),
        "MessagingChunk" | "messaging" | "messaging_chunk" => Ok(SourceKind::MessagingChunk),
        "InfrastructureChunk" | "infrastructure" | "infrastructure_chunk" => {
            Ok(SourceKind::InfrastructureChunk)
        }
        "WpfChunk" | "wpf" | "wpf_chunk" => Ok(SourceKind::WpfChunk),
        "GoChunk" | "go" | "go_chunk" => Ok(SourceKind::GoChunk),
        _ => Err(ControlError::bad_request("unknown source_kind")),
    }
}

fn hybrid_result_json(result: b3_query::hybrid::HybridSearchResult) -> Value {
    json!({
        "document_id": result.document_id,
        "file_id": result.file_id.as_str(),
        "symbol_id": result.symbol_id.as_ref().map(|symbol| symbol.as_str().to_string()),
        "path": result.path,
        "text_preview": result.text_preview,
        "snippet": result.text_preview,
        "language": result.language,
        "framework": result.framework,
        "source_kind": result.source_kind.as_str(),
        "start_line": result.start_line,
        "end_line": result.end_line,
        "final_score": result.final_score,
        "lexical_score": result.lexical_score,
        "vector_score": result.vector_score,
        "metadata_score": result.metadata_score,
        "explanation": result.explanation.map(hybrid_explanation_json)
    })
}

fn hybrid_explanation_json(explanation: HybridRankingExplanation) -> Value {
    json!({
        "final_score": explanation.final_score,
        "lexical_score": explanation.lexical_score,
        "vector_score": explanation.vector_score,
        "metadata_score": explanation.metadata_score,
        "matched_terms": explanation.matched_terms,
        "boosts": explanation.boosts,
        "vector_provider": explanation.vector_provider,
        "vector_dimension": explanation.vector_dimension,
        "fallback_reason": explanation.fallback_reason,
        "filters": explanation.filters
    })
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

#[derive(Debug, Clone, Deserialize)]
struct RoutesQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    framework: Option<String>,
    method: Option<String>,
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct RouteMatchesQuery {
    method: Option<String>,
    path: Option<String>,
    source_project_id: Option<String>,
    target_project_id: Option<String>,
    min_confidence: Option<u16>,
    limit: Option<usize>,
    branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessageMatchesQuery {
    broker: Option<String>,
    channel_kind: Option<String>,
    name: Option<String>,
    source_project_id: Option<String>,
    target_project_id: Option<String>,
    min_confidence: Option<u16>,
    limit: Option<usize>,
    branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DependencyMatchesQuery {
    kind: Option<String>,
    ecosystem: Option<String>,
    contract_kind: Option<String>,
    infra_kind: Option<String>,
    name: Option<String>,
    source_project_id: Option<String>,
    target_project_id: Option<String>,
    min_confidence: Option<u16>,
    limit: Option<usize>,
    branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureGraphQuery {
    relationship_kind: Option<String>,
    relationship_kinds: Option<String>,
    project_id: Option<String>,
    project_ids: Option<String>,
    min_confidence: Option<String>,
    include_evidence: Option<bool>,
    include_warnings: Option<bool>,
    include_unresolved: Option<bool>,
    max_nodes: Option<usize>,
    max_edges: Option<usize>,
    depth: Option<usize>,
    seed_project_id: Option<String>,
    seed_node_id: Option<String>,
    layout_hint: Option<String>,
    branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServiceMapQuery {
    project_id: Option<String>,
    project_ids: Option<String>,
    min_confidence: Option<String>,
    include_routes: Option<bool>,
    include_messaging: Option<bool>,
    include_dependencies: Option<bool>,
    include_infrastructure: Option<bool>,
    include_evidence: Option<bool>,
    include_unresolved: Option<bool>,
    limit: Option<usize>,
    branch: Option<String>,
}

impl ArchitectureGraphQuery {
    fn into_request(self) -> ArchitectureGraphRequest {
        let mut relationship_kinds = split_query_list(self.relationship_kinds);
        relationship_kinds.extend(split_query_list(self.relationship_kind));
        ArchitectureGraphRequest {
            branch: self.branch,
            relationship_kinds,
            project_ids: merged_query_list(self.project_ids, self.project_id),
            min_confidence: self.min_confidence.map(confidence_filter),
            include_evidence: self.include_evidence.unwrap_or(true),
            include_warnings: self.include_warnings.unwrap_or(true),
            include_unresolved: self.include_unresolved.unwrap_or(true),
            max_nodes: self.max_nodes,
            max_edges: self.max_edges,
            depth: self.depth,
            seed_project_id: self.seed_project_id,
            seed_node_id: self.seed_node_id,
            layout_hint: self.layout_hint,
        }
    }
}

impl ServiceMapQuery {
    fn into_request(self) -> ServiceMapRequest {
        ServiceMapRequest {
            branch: self.branch,
            project_ids: merged_query_list(self.project_ids, self.project_id),
            include_routes: self.include_routes.unwrap_or(true),
            include_messaging: self.include_messaging.unwrap_or(true),
            include_dependencies: self.include_dependencies.unwrap_or(true),
            include_infrastructure: self.include_infrastructure.unwrap_or(true),
            min_confidence: self.min_confidence.map(confidence_filter),
            include_evidence: self.include_evidence.unwrap_or(true),
            include_unresolved: self.include_unresolved.unwrap_or(true),
            limit: self.limit,
        }
    }
}

fn confidence_filter(value: String) -> GraphConfidenceFilter {
    value
        .parse::<u16>()
        .map(GraphConfidenceFilter::Score)
        .unwrap_or(GraphConfidenceFilter::Level(value))
}

fn merged_query_list(primary: Option<String>, secondary: Option<String>) -> Vec<String> {
    let mut values = split_query_list(primary);
    values.extend(split_query_list(secondary));
    values.sort();
    values.dedup();
    values
}

fn split_query_list(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

impl DependencyMatchesQuery {
    fn into_options(self) -> DependencyMatchOptions {
        DependencyMatchOptions {
            kind: DependencyMatchKindFilter::from_query(self.kind.as_deref()),
            ecosystem: self.ecosystem,
            contract_kind: self.contract_kind,
            infra_kind: self.infra_kind,
            name: self.name,
            source_project_id: self.source_project_id,
            target_project_id: self.target_project_id,
            min_confidence: self.min_confidence,
            limit: self.limit.unwrap_or(DEFAULT_LIMIT),
            branch: self.branch,
        }
    }
}

impl MessageMatchesQuery {
    fn into_options(self) -> MessageMatchOptions {
        MessageMatchOptions {
            broker: self.broker,
            channel_kind: self.channel_kind,
            name: self.name,
            source_project_id: self.source_project_id,
            target_project_id: self.target_project_id,
            min_confidence: self.min_confidence,
            limit: self.limit.unwrap_or(DEFAULT_LIMIT),
            branch: self.branch,
        }
    }
}

impl RouteMatchesQuery {
    fn into_options(self) -> RouteMatchOptions {
        RouteMatchOptions {
            method: self.method,
            path: self.path,
            source_project_id: self.source_project_id,
            target_project_id: self.target_project_id,
            min_confidence: self.min_confidence,
            limit: self.limit.unwrap_or(DEFAULT_LIMIT),
            branch: self.branch,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RoutesResponse {
    status: String,
    project_id: String,
    branch_id: String,
    routes: Vec<RouteDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComponentsQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    framework: Option<String>,
    name: Option<String>,
    file: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct ComponentsResponse {
    status: String,
    project_id: String,
    branch_id: String,
    components: Vec<ComponentDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct DataAccessQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    technology: Option<String>,
    kind: Option<String>,
    operation: Option<String>,
    file: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct DataAccessResponse {
    status: String,
    project_id: String,
    branch_id: String,
    data_access: Vec<DataAccessDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct RealtimeQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    technology: Option<String>,
    kind: Option<String>,
    event: Option<String>,
    file: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct RealtimeResponse {
    status: String,
    project_id: String,
    branch_id: String,
    realtime: Vec<RealtimeDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct MessagingQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    technology: Option<String>,
    kind: Option<String>,
    topic: Option<String>,
    queue: Option<String>,
    routing_key: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct MessagingResponse {
    status: String,
    project_id: String,
    branch_id: String,
    messaging: Vec<MessagingDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct InfrastructureQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    technology: Option<String>,
    kind: Option<String>,
    name: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct InfrastructureResponse {
    status: String,
    project_id: String,
    branch_id: String,
    infrastructure: Vec<InfrastructureDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct WpfQuery {
    project_id: Option<String>,
    branch_id: Option<String>,
    kind: Option<String>,
    binding: Option<String>,
    command: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
struct WpfResponse {
    status: String,
    project_id: String,
    branch_id: String,
    wpf: Vec<WpfDto>,
}

#[derive(Debug, Clone, Serialize)]
struct DataAccessDto {
    id: String,
    technology: String,
    kind: String,
    operation: Option<String>,
    file_path: String,
    symbol_id: String,
    class_name: Option<String>,
    method_name: Option<String>,
    entity_name: Option<String>,
    context_name: Option<String>,
    repository_name: Option<String>,
    query_text: Option<String>,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct RealtimeDto {
    id: String,
    technology: String,
    kind: String,
    direction: String,
    event_name: Option<String>,
    channel_name: Option<String>,
    hub_name: Option<String>,
    method_name: Option<String>,
    endpoint: Option<String>,
    file_path: String,
    symbol_id: String,
    class_name: Option<String>,
    function_name: Option<String>,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct MessagingDto {
    id: String,
    technology: String,
    kind: String,
    direction: String,
    topic: Option<String>,
    queue: Option<String>,
    exchange: Option<String>,
    routing_key: Option<String>,
    pattern: Option<String>,
    consumer_group: Option<String>,
    file_path: String,
    symbol_id: String,
    class_name: Option<String>,
    function_name: Option<String>,
    method_name: Option<String>,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct InfrastructureDto {
    id: String,
    technology: String,
    kind: String,
    name: Option<String>,
    resource_type: Option<String>,
    provider: Option<String>,
    image: Option<String>,
    service_name: Option<String>,
    container_name: Option<String>,
    namespace: Option<String>,
    ports: Vec<String>,
    env_keys: Vec<String>,
    labels: Vec<String>,
    selectors: Vec<String>,
    file_path: String,
    symbol_id: String,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct WpfDto {
    id: String,
    technology: String,
    kind: String,
    name: Option<String>,
    x_class: Option<String>,
    code_behind: Option<String>,
    view_model: Option<String>,
    binding_paths: Vec<String>,
    command_bindings: Vec<String>,
    resource_keys: Vec<String>,
    resource_sources: Vec<String>,
    data_context: Option<String>,
    file_path: String,
    symbol_id: String,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct ComponentDto {
    id: String,
    name: String,
    framework: String,
    file_path: String,
    symbol_id: String,
    export_kind: Option<String>,
    component_kind: String,
    props_type_name: Option<String>,
    hooks: Vec<String>,
    usages: Vec<String>,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct RouteDto {
    id: String,
    framework: String,
    route_kind: String,
    method: String,
    path: String,
    file_path: String,
    symbol_id: String,
    handler_name: Option<String>,
    class_name: Option<String>,
    function_name: Option<String>,
    line_start: usize,
    line_end: usize,
    confidence: u16,
    source_kind: String,
}

impl From<StoredComponent> for ComponentDto {
    fn from(value: StoredComponent) -> Self {
        Self {
            id: value.id,
            name: value.name,
            framework: value.framework,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            export_kind: value.export_kind,
            component_kind: value.component_kind,
            props_type_name: value.props_type_name,
            hooks: value.hooks,
            usages: value.usages,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
}

impl From<StoredRoute> for RouteDto {
    fn from(value: StoredRoute) -> Self {
        Self {
            id: value.id,
            framework: value.framework,
            route_kind: value.route_kind,
            method: value.method,
            path: value.path,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            handler_name: value.handler_name,
            class_name: value.class_name,
            function_name: value.function_name,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
}

impl From<StoredDataAccess> for DataAccessDto {
    fn from(value: StoredDataAccess) -> Self {
        Self {
            id: value.id,
            technology: value.technology,
            kind: value.kind,
            operation: value.operation,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            class_name: value.class_name,
            method_name: value.method_name,
            entity_name: value.entity_name,
            context_name: value.context_name,
            repository_name: value.repository_name,
            query_text: value.query_text,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
}

impl From<StoredRealtime> for RealtimeDto {
    fn from(value: StoredRealtime) -> Self {
        Self {
            id: value.id,
            technology: value.technology,
            kind: value.kind,
            direction: value.direction,
            event_name: value.event_name,
            channel_name: value.channel_name,
            hub_name: value.hub_name,
            method_name: value.method_name,
            endpoint: value.endpoint,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            class_name: value.class_name,
            function_name: value.function_name,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
}

impl From<StoredMessaging> for MessagingDto {
    fn from(value: StoredMessaging) -> Self {
        Self {
            id: value.id,
            technology: value.technology,
            kind: value.kind,
            direction: value.direction,
            topic: value.topic,
            queue: value.queue,
            exchange: value.exchange,
            routing_key: value.routing_key,
            pattern: value.pattern,
            consumer_group: value.consumer_group,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            class_name: value.class_name,
            function_name: value.function_name,
            method_name: value.method_name,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
}

impl From<StoredInfrastructure> for InfrastructureDto {
    fn from(value: StoredInfrastructure) -> Self {
        Self {
            id: value.id,
            technology: value.technology,
            kind: value.kind,
            name: value.name,
            resource_type: value.resource_type,
            provider: value.provider,
            image: value.image,
            service_name: value.service_name,
            container_name: value.container_name,
            namespace: value.namespace,
            ports: value.ports,
            env_keys: value.env_keys,
            labels: value.labels,
            selectors: value.selectors,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
}

impl From<StoredWpf> for WpfDto {
    fn from(value: StoredWpf) -> Self {
        Self {
            id: value.id,
            technology: value.technology,
            kind: value.kind,
            name: value.name,
            x_class: value.x_class,
            code_behind: value.code_behind,
            view_model: value.view_model,
            binding_paths: value.binding_paths,
            command_bindings: value.command_bindings,
            resource_keys: value.resource_keys,
            resource_sources: value.resource_sources,
            data_context: value.data_context,
            file_path: value.file_path,
            symbol_id: value.symbol_id,
            line_start: value.line_start,
            line_end: value.line_end,
            confidence: value.confidence,
            source_kind: value.source_kind,
        }
    }
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

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
            DomainEvent::ParserWorkerStarted(event) => self.events.emit(
                "parser_worker_started",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "attempt": event.attempt}),
            ),
            DomainEvent::ParserWorkerCompleted(event) => self.events.emit(
                "parser_worker_completed",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "elapsed_ms": event.elapsed_ms}),
            ),
            DomainEvent::ParserWorkerTimeout(event) => self.events.emit(
                "parser_worker_timeout",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "timeout_ms": event.timeout_ms, "attempt": event.attempt}),
            ),
            DomainEvent::ParserWorkerCrashed(event) => self.events.emit(
                "parser_worker_crashed",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "exit_code": event.exit_code, "stderr_excerpt": event.stderr_excerpt, "attempt": event.attempt}),
            ),
            DomainEvent::ParseFailed(event) => self.events.emit(
                "parse_failed",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "error_kind": event.error_kind, "error_message": event.error_message, "retry_count": event.retry_count}),
            ),
            DomainEvent::ParseRetried(event) => self.events.emit(
                "parse_retried",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "attempt": event.attempt, "reason": event.reason}),
            ),
            DomainEvent::ParseFailureRecorded(event) => self.events.emit(
                "parse_failure_recorded",
                json!({"project_id": event.project_id.as_str(), "branch_id": event.branch_id.as_str(), "file_id": event.file_id.as_str(), "path": event.path, "error_kind": event.error_kind}),
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
        FileRecord, GraphEdge, GraphEdgeMetadata, GraphNode, IndexStore, IndexedFileRecord, NodeId,
        NodeKind, ParseFailureRecord, SymbolId, SymbolRecord,
    };
    use b3_storage::NewCentralityRecord;
    use http::{Request, StatusCode};
    use std::fs;
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

    fn edit_app(root: PathBuf, database_path: PathBuf) -> Router {
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .upsert_project(&project_id, "Default", &root.display().to_string())
            .expect("project");
        storage
            .upsert_branch(&branch_id, &project_id, &BranchMetadata::new("main"))
            .expect("branch");
        let file = FileRecord {
            id: FileId::new("src-lib"),
            project_id: project_id.clone(),
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
        };
        storage.upsert_file(&file, &branch_id).expect("file");
        let mut symbol = SymbolRecord::new(
            SymbolId::new("run-symbol"),
            file.id.clone(),
            "run",
            NodeKind::Function,
        );
        symbol.start_line = 1;
        symbol.end_line = 1;
        storage
            .upsert_symbol(&project_id, &branch_id, &symbol)
            .expect("symbol");

        app(ControlState::from_storage(root, database_path, storage))
    }

    fn rename_app(root: PathBuf, database_path: PathBuf) -> Router {
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .upsert_project(&project_id, "Default", &root.display().to_string())
            .expect("project");
        storage
            .upsert_branch(&branch_id, &project_id, &BranchMetadata::new("main"))
            .expect("branch");
        let file = FileRecord {
            id: FileId::new("src-lib"),
            project_id: project_id.clone(),
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
        };
        storage.upsert_file(&file, &branch_id).expect("file");
        let mut symbol = SymbolRecord::new(
            SymbolId::new("old-name-symbol"),
            file.id.clone(),
            "old_name",
            NodeKind::Function,
        );
        symbol.start_line = 1;
        symbol.end_line = 1;
        storage
            .upsert_symbol(&project_id, &branch_id, &symbol)
            .expect("symbol");

        app(ControlState::from_storage(root, database_path, storage))
    }

    fn write_group_registry(
        path: &std::path::Path,
        projects: &[(&str, &str, &std::path::Path)],
        group_projects: &[&str],
    ) {
        let projects_json = projects
            .iter()
            .map(|(id, name, db)| {
                format!(
                    r#"{{"id":"{id}","name":"{name}","path":"{}","database":"{}","tags":[]}}"#,
                    db.parent()
                        .expect("db parent")
                        .display()
                        .to_string()
                        .replace('\\', "\\\\"),
                    db.display().to_string().replace('\\', "\\\\")
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let project_ids = group_projects
            .iter()
            .map(|id| format!(r#""{id}""#))
            .collect::<Vec<_>>()
            .join(",");
        let registry = format!(
            r#"{{"version":1,"projects":[{projects_json}],"groups":[{{"id":"suite","name":"Suite","description":"","project_ids":[{project_ids}],"tags":[]}}]}}"#
        );
        fs::write(path, registry).expect("registry");
    }

    fn seed_route_database(database_path: &std::path::Path, project: &str, route_path: &str) {
        let storage = SqliteStorage::open(database_path).expect("open project database");
        let project_id = ProjectId::new(project);
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project}-route-file")),
                        project_id: project_id.clone(),
                        path: format!("src/{project}.ts"),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: 32,
                    content: format!("app.get('{route_path}', handler);"),
                    symbols: vec![SymbolRecord {
                        id: SymbolId::new(format!("{project}-route")),
                        file_id: FileId::new(format!("{project}-route-file")),
                        name: format!("GET {route_path}"),
                        kind: NodeKind::Route,
                        start_byte: 0,
                        end_byte: 32,
                        start_line: 1,
                        start_column: 0,
                        end_line: 1,
                        end_column: 32,
                        visibility: Some(format!(
                            "route.framework=express;route.method=GET;route.path={route_path};route.file=src/{project}.ts;route.handler=handler;route.source=ExpressCall;route.line_start=1;route.line_end=1;route.confidence=9500"
                        )),
                    }],
                    edges: Vec::new(),
                },
            )
            .expect("route file");
    }

    fn seed_route_match_database(
        database_path: &std::path::Path,
        project: &str,
        content: &str,
        routes: &[(&str, &str)],
    ) {
        let storage = SqliteStorage::open(database_path).expect("open project database");
        let project_id = ProjectId::new(project);
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let symbols = routes
            .iter()
            .enumerate()
            .map(|(index, (method, route_path))| {
                let mut symbol = SymbolRecord::new(
                    SymbolId::new(format!("{project}-route-{index}")),
                    FileId::new(format!("{project}-file")),
                    format!("{method} {route_path}"),
                    NodeKind::Route,
                );
                symbol.start_line = index + 1;
                symbol.end_line = index + 1;
                symbol.visibility = Some(format!(
                    "route.framework=express;route.kind=api;route.method={method};route.path={route_path};route.file=src/{project}.ts;route.handler=handler;route.source=ExpressCall;route.line_start={};route.line_end={};route.confidence=9500",
                    index + 1,
                    index + 1
                ));
                symbol
            })
            .collect::<Vec<_>>();
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project}-file")),
                        project_id: project_id.clone(),
                        path: format!("src/{project}.ts"),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: content.len() as u64,
                    content: content.to_string(),
                    symbols,
                    edges: Vec::new(),
                },
            )
            .expect("route match file");
    }

    fn seed_message_match_database(
        database_path: &std::path::Path,
        project: &str,
        records: &[(&str, &str, &str, Option<&str>, Option<&str>, Option<&str>)],
    ) {
        let storage = SqliteStorage::open(database_path).expect("open project database");
        let project_id = ProjectId::new(project);
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let symbols = records
            .iter()
            .enumerate()
            .map(|(index, (technology, direction, kind, topic, queue, pattern))| {
                let mut metadata = format!(
                    "messaging.technology={technology};messaging.kind={kind};messaging.direction={direction};messaging.file=src/{project}.ts;messaging.source=TestMessaging;messaging.line_start={};messaging.line_end={};messaging.confidence=9000",
                    index + 1,
                    index + 1
                );
                if let Some(topic) = topic {
                    metadata.push_str(&format!(";messaging.topic={topic}"));
                }
                if let Some(queue) = queue {
                    metadata.push_str(&format!(";messaging.queue={queue}"));
                }
                if let Some(pattern) = pattern {
                    metadata.push_str(&format!(";messaging.pattern={pattern}"));
                }
                let mut symbol = SymbolRecord::new(
                    SymbolId::new(format!("{project}-message-{index}")),
                    FileId::new(format!("{project}-message-file")),
                    format!("{technology} {kind} {index}"),
                    NodeKind::Endpoint,
                );
                symbol.start_line = index + 1;
                symbol.end_line = index + 1;
                symbol.visibility = Some(metadata);
                symbol
            })
            .collect::<Vec<_>>();
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project}-message-file")),
                        project_id: project_id.clone(),
                        path: format!("src/{project}.ts"),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: 1,
                    content: "messaging metadata".to_string(),
                    symbols,
                    edges: Vec::new(),
                },
            )
            .expect("message match file");
    }

    fn seed_manifest_database(
        database_path: &std::path::Path,
        project: &str,
        file_path: &str,
        content: &str,
    ) {
        let storage = SqliteStorage::open(database_path).expect("open project database");
        let project_id = ProjectId::new(project);
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new(format!("{project}-manifest-file")),
                        project_id: project_id.clone(),
                        path: file_path.to_string(),
                        content_hash: format!("hash-{project}"),
                    },
                    language: Some("json".to_string()),
                    size_bytes: content.len() as u64,
                    content: content.to_string(),
                    symbols: Vec::new(),
                    edges: Vec::new(),
                },
            )
            .expect("manifest file");
    }

    fn route_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new("route-file"),
                        project_id: project_id.clone(),
                        path: "src/server.ts".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: 32,
                    content: "app.get('/users', listUsers);".to_string(),
                    symbols: vec![
                        SymbolRecord {
                            id: SymbolId::new("route-symbol"),
                            file_id: FileId::new("route-file"),
                            name: "GET /users".to_string(),
                            kind: NodeKind::Route,
                            start_byte: 0,
                            end_byte: 28,
                            start_line: 1,
                            start_column: 0,
                            end_line: 1,
                            end_column: 28,
                            visibility: Some(
                                "route.framework=express;route.kind=api;route.method=GET;route.path=/users;route.file=src/server.ts;route.handler=listUsers;route.function=listUsers;route.source=ExpressCall;route.line_start=1;route.line_end=1;route.confidence=9500".to_string(),
                            ),
                        },
                        SymbolRecord {
                            id: SymbolId::new("next-route-symbol"),
                            file_id: FileId::new("route-file"),
                            name: "GET /dashboard".to_string(),
                            kind: NodeKind::Route,
                            start_byte: 29,
                            end_byte: 58,
                            start_line: 2,
                            start_column: 0,
                            end_line: 2,
                            end_column: 29,
                            visibility: Some(
                                "route.framework=nextjs;route.kind=page;route.method=GET;route.path=/dashboard;route.file=app/dashboard/page.tsx;route.source=NextAppPage;route.line_start=1;route.line_end=1;route.confidence=9000".to_string(),
                            ),
                        },
                        SymbolRecord {
                            id: SymbolId::new("angular-route-symbol"),
                            file_id: FileId::new("route-file"),
                            name: "GET /users/:id".to_string(),
                            kind: NodeKind::Route,
                            start_byte: 59,
                            end_byte: 90,
                            start_line: 3,
                            start_column: 0,
                            end_line: 3,
                            end_column: 31,
                            visibility: Some(
                                "route.framework=angular;route.kind=route;route.method=GET;route.path=/users/:id;route.file=src/app/app-routing.module.ts;route.handler=UserDetailComponent;route.class=UserDetailComponent;route.source=AngularRoute;route.line_start=1;route.line_end=1;route.confidence=8000".to_string(),
                            ),
                        },
                        SymbolRecord {
                            id: SymbolId::new("aspnetcore-route-symbol"),
                            file_id: FileId::new("route-file"),
                            name: "GET /api/users/{id}".to_string(),
                            kind: NodeKind::Route,
                            start_byte: 91,
                            end_byte: 130,
                            start_line: 4,
                            start_column: 0,
                            end_line: 4,
                            end_column: 39,
                            visibility: Some(
                                "route.framework=aspnetcore;route.kind=api;route.method=GET;route.path=/api/users/{id};route.file=Controllers/UsersController.cs;route.handler=Get;route.class=UsersController;route.function=Get;route.source=AspNetCoreHttpGetAttribute;route.line_start=1;route.line_end=1;route.confidence=9500".to_string(),
                            ),
                        },
                    ],
                    edges: Vec::new(),
                },
            )
            .expect("route file");
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn component_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new("component-file"),
                        project_id: project_id.clone(),
                        path: "src/ProductCard.tsx".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("tsx".to_string()),
                    size_bytes: 32,
                    content: "export function ProductCard() { return <div />; }".to_string(),
                    symbols: vec![
                        SymbolRecord {
                            id: SymbolId::new("component-symbol"),
                            file_id: FileId::new("component-file"),
                            name: "ProductCard".to_string(),
                            kind: NodeKind::Function,
                            start_byte: 0,
                            end_byte: 48,
                            start_line: 1,
                            start_column: 0,
                            end_line: 1,
                            end_column: 48,
                            visibility: Some("export;component.framework=react;component.export=named;component.kind=function;component.props=ProductCardProps;component.source=FunctionDeclaration;component.hooks=useState;component.usages=Badge;component.line_start=1;component.line_end=1;component.confidence=9500".to_string()),
                        },
                        SymbolRecord {
                            id: SymbolId::new("angular-component-symbol"),
                            file_id: FileId::new("component-file"),
                            name: "UserCardComponent".to_string(),
                            kind: NodeKind::Class,
                            start_byte: 49,
                            end_byte: 90,
                            start_line: 2,
                            start_column: 0,
                            end_line: 2,
                            end_column: 41,
                            visibility: Some("export;component.framework=angular;component.export=named;component.kind=component;component.source=AngularComponent;component.hooks=;component.usages=;component.line_start=2;component.line_end=2;component.confidence=9000;angular.framework=angular;angular.kind=component;angular.source=AngularComponent;angular.class=UserCardComponent;angular.selector=app-user-card;angular.template_url=./user-card.component.html;angular.style_urls=./user-card.component.scss;angular.inline_template_present=false;angular.standalone=true;angular.imports=CommonModule;angular.providers=UserService;angular.dependencies=;angular.declarations=;angular.exports=;angular.bootstrap=;angular.line_start=2;angular.line_end=2;angular.confidence=9000".to_string()),
                        },
                    ],
                    edges: Vec::new(),
                },
            )
            .expect("component file");
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn realtime_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new("realtime-file"),
                        project_id: project_id.clone(),
                        path: "src/socket.ts".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: 32,
                    content: "socket.on('message', handler);".to_string(),
                    symbols: vec![SymbolRecord {
                        id: SymbolId::new("realtime-symbol"),
                        file_id: FileId::new("realtime-file"),
                        name: "Socket.IO on message".to_string(),
                        kind: NodeKind::Endpoint,
                        start_byte: 0,
                        end_byte: 29,
                        start_line: 1,
                        start_column: 0,
                        end_line: 1,
                        end_column: 29,
                        visibility: Some("realtime.technology=socketio;realtime.kind=Listener;realtime.direction=inbound;realtime.event=message;realtime.file=src/socket.ts;realtime.function=connect;realtime.source=SocketIoOn;realtime.line_start=1;realtime.line_end=1;realtime.confidence=9000".to_string()),
                    }],
                    edges: Vec::new(),
                },
            )
            .expect("realtime file");
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn messaging_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new("messaging-file"),
                        project_id: project_id.clone(),
                        path: "src/messaging.ts".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("typescript".to_string()),
                    size_bytes: 32,
                    content: "producer.send({ topic: 'orders' });".to_string(),
                    symbols: vec![SymbolRecord {
                        id: SymbolId::new("messaging-symbol"),
                        file_id: FileId::new("messaging-file"),
                        name: "Kafka send orders".to_string(),
                        kind: NodeKind::Endpoint,
                        start_byte: 0,
                        end_byte: 35,
                        start_line: 1,
                        start_column: 0,
                        end_line: 1,
                        end_column: 35,
                        visibility: Some("messaging.technology=kafka;messaging.kind=Producer;messaging.direction=outbound;messaging.topic=orders;messaging.queue=orders.queue;messaging.exchange=orders.exchange;messaging.routing_key=order.created;messaging.pattern=order.created;messaging.consumer_group=orders-workers;messaging.file=src/messaging.ts;messaging.function=publish;messaging.method=publish;messaging.source=KafkaProducerSend;messaging.line_start=1;messaging.line_end=1;messaging.confidence=9000".to_string()),
                    }],
                    edges: Vec::new(),
                },
            )
            .expect("messaging file");
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn infrastructure_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new("infra-file"),
                        project_id: project_id.clone(),
                        path: "deploy/k8s.yaml".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("kubernetes".to_string()),
                    size_bytes: 32,
                    content: "kind: Deployment".to_string(),
                    symbols: vec![SymbolRecord {
                        id: SymbolId::new("infra-symbol"),
                        file_id: FileId::new("infra-file"),
                        name: "Kubernetes Deployment".to_string(),
                        kind: NodeKind::ConfigKey,
                        start_byte: 0,
                        end_byte: 16,
                        start_line: 1,
                        start_column: 0,
                        end_line: 1,
                        end_column: 16,
                        visibility: Some("infrastructure.technology=kubernetes;infrastructure.kind=Deployment;infrastructure.name=api;infrastructure.resource_type=Deployment;infrastructure.image=my-api:latest;infrastructure.service_name=api;infrastructure.container_name=api;infrastructure.namespace=default;infrastructure.ports=8080;infrastructure.env_keys=NODE_ENV;infrastructure.labels=app=api;infrastructure.selectors=app=api;infrastructure.file=deploy/k8s.yaml;infrastructure.source=KubernetesDeployment;infrastructure.line_start=1;infrastructure.line_end=12;infrastructure.confidence=9000".to_string()),
                    }],
                    edges: Vec::new(),
                },
            )
            .expect("infrastructure file");
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
    }

    fn wpf_app() -> Router {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("default");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: FileId::new("wpf-file"),
                        project_id: project_id.clone(),
                        path: "Views/MainWindow.xaml".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("xaml".to_string()),
                    size_bytes: 32,
                    content: "<Window />".to_string(),
                    symbols: vec![SymbolRecord {
                        id: SymbolId::new("wpf-symbol"),
                        file_id: FileId::new("wpf-file"),
                        name: "MainWindow".to_string(),
                        kind: NodeKind::Endpoint,
                        start_byte: 0,
                        end_byte: 10,
                        start_line: 1,
                        start_column: 0,
                        end_line: 12,
                        end_column: 0,
                        visibility: Some("wpf.technology=wpf;wpf.kind=Window;wpf.name=MainWindow;wpf.x_class=App.Views.MainWindow;wpf.code_behind=Views/MainWindow.xaml.cs;wpf.view_model=MainWindowViewModel;wpf.binding_paths=UserName,SelectedUser;wpf.command_bindings=SaveCommand;wpf.resource_keys=PrimaryBrush;wpf.resource_sources=Themes/Colors.xaml;wpf.data_context=MainViewModel;wpf.file=Views/MainWindow.xaml;wpf.source=XamlWindow;wpf.line_start=1;wpf.line_end=12;wpf.confidence=9000".to_string()),
                    }],
                    edges: Vec::new(),
                },
            )
            .expect("wpf file");
        app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
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
    async fn capabilities_include_language_backend_truth() {
        let response = empty_app()
            .oneshot(
                Request::builder()
                    .uri("/api/capabilities")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let backends = body["language_backends"]["backends"]
            .as_array()
            .expect("backends");
        let rust = backends
            .iter()
            .find(|backend| backend["backend_id"] == "tree-sitter-rust")
            .expect("rust backend");
        let csharp = backends
            .iter()
            .find(|backend| backend["language_id"] == "csharp")
            .expect("csharp backend");
        let go = backends
            .iter()
            .find(|backend| backend["language_id"] == "go")
            .expect("go backend");

        assert_eq!(rust["support_level"], "Good");
        assert_eq!(rust["available"], true);
        assert_eq!(csharp["available"], true);
        assert_eq!(go["backend_id"], "static-go");
        assert_eq!(go["support_level"], "Basic");
        assert_eq!(go["available"], true);
        assert_eq!(body["language_backend"]["go"]["support"], "basic_static");
        assert_eq!(
            body["language_backend"]["go"]["go_toolchain_required"],
            false
        );
        assert_eq!(
            body["language_backend"]["additional_backend_languages"]["available"],
            true
        );
        assert_eq!(
            body["language_backend"]["additional_backend_languages"]["languages"]["python"],
            "basic"
        );
        let python = backends
            .iter()
            .find(|backend| backend["backend_id"] == "static-python")
            .expect("python backend");
        assert_eq!(python["support_level"], "Basic");
        assert_eq!(python["available"], true);
        assert_eq!(body["vector_search"]["architecture_available"], true);
        assert_eq!(body["vector_search"]["semantic_search_available"], true);
        assert_eq!(body["vector_search"]["semantic_search_ready"], true);
        assert_eq!(body["vector_search"]["hybrid_ranking_available"], true);
        assert_eq!(body["control_api"]["hybrid_search"], true);
        assert_eq!(
            body["architecture"]["architecture_contracts_available"],
            true
        );
        assert_eq!(body["architecture"]["group_federation_ready"], true);
        assert_eq!(body["architecture"]["route_matching_ready"], true);
        assert_eq!(body["architecture"]["messaging_matching_ready"], true);
        assert_eq!(
            body["architecture"]["package_contract_infra_matching_ready"],
            true
        );
        assert_eq!(body["architecture"]["group_impact_ready"], true);
        assert_eq!(body["architecture"]["group_context_pack_ready"], true);
        assert_eq!(body["architecture"]["service_map_ready"], true);
        assert_eq!(body["architecture"]["architecture_graph_api_ready"], true);
        assert_eq!(body["architecture"]["architecture_graph_ui_ready"], false);
        assert_eq!(body["architecture"]["local_only"], true);
        assert_eq!(body["architecture"]["global_db_merge_required"], false);
        assert_eq!(body["architecture"]["cloud_graph_database_required"], false);
        assert_eq!(
            body["architecture"]["hosted_vector_database_required"],
            false
        );
        assert_eq!(body["architecture"]["telemetry_enabled"], false);
        assert_eq!(body["control_api"]["architecture_status"], true);
        assert_eq!(body["control_api"]["architecture_groups"], true);
        assert_eq!(body["control_api"]["architecture_group_summary"], true);
        assert_eq!(
            body["control_api"]["architecture_group_route_matches"],
            true
        );
        assert_eq!(
            body["control_api"]["architecture_group_message_matches"],
            true
        );
        assert_eq!(
            body["control_api"]["architecture_group_dependency_matches"],
            true
        );
        assert_eq!(body["control_api"]["architecture_group_impact"], true);
        assert_eq!(body["control_api"]["architecture_group_graph"], true);
        assert_eq!(body["control_api"]["architecture_group_service_map"], true);
        assert_eq!(body["vector_search"]["local_only"], true);
        assert_eq!(body["vector_search"]["external_plugins_enabled"], false);
        assert_eq!(body["language_backends"]["lsp_enabled"], false);
        assert_eq!(body["quality_audit"]["phase"], "17");
        assert_eq!(body["quality_audit"]["completed"], true);
        assert_eq!(body["quality_audit"]["support_matrix_audited"], true);
        assert_eq!(body["quality_audit"]["runtime_validation_claimed"], false);
        assert_eq!(body["quality_audit"]["compiler_grade_claimed"], false);
        assert_eq!(body["quality_audit"]["ide_grade_refactor_claimed"], false);
        assert_eq!(body["quality_audit"]["architecture_graph_ui_ready"], false);
        assert_eq!(body["quality_audit"]["full_git_intelligence_ready"], false);
        assert_eq!(body["quality_audit"]["local_only"], true);
        assert_eq!(body["editing"]["ide_grade_semantic_rename"], false);
    }

    #[tokio::test]
    async fn architecture_status_reports_route_matching_ready() {
        let response = empty_app()
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["architecture_contracts_available"], true);
        assert_eq!(body["group_federation_ready"], true);
        assert_eq!(body["route_matching_ready"], true);
        assert_eq!(body["messaging_matching_ready"], true);
        assert_eq!(body["package_contract_infra_matching_ready"], true);
        assert_eq!(body["group_impact_ready"], true);
        assert_eq!(body["group_context_pack_ready"], true);
        assert_eq!(body["service_map_ready"], true);
        assert_eq!(body["architecture_graph_api_ready"], true);
        assert_eq!(body["architecture_graph_ui_ready"], false);
        assert_eq!(body["local_only"], true);
        assert_eq!(body["global_db_merge_required"], false);
        assert_eq!(body["cloud_graph_database_required"], false);
        assert_eq!(body["hosted_vector_database_required"], false);
        assert_eq!(body["telemetry_enabled"], false);
    }

    #[tokio::test]
    async fn architecture_group_endpoints_resolve_registry_and_warn_on_missing_db() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        let missing_db = dir.path().join("missing").join(".b3").join("b3.db");
        seed_route_database(&api_db, "api", "/orders");
        write_group_registry(
            &registry_path,
            &[("api", "API", &api_db), ("missing", "Missing", &missing_db)],
            &["api", "missing"],
        );

        let storage = SqliteStorage::open_in_memory().expect("control storage");
        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
            registry_path,
        ));

        let groups_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("groups");
        assert_eq!(groups_response.status(), StatusCode::OK);
        let groups = response_json(groups_response).await;
        assert_eq!(groups["groups"][0]["id"], "suite");
        assert_eq!(groups["federation_ready"], true);
        assert_eq!(groups["matching_ready"], true);
        assert_eq!(groups["route_matching_ready"], true);
        assert_eq!(groups["messaging_matching_ready"], true);

        let status_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("status");
        assert_eq!(status_response.status(), StatusCode::OK);
        let status = response_json(status_response).await;
        assert_eq!(status["group"]["opened_project_count"], 1);
        assert_eq!(status["group"]["skipped_project_count"], 1);
        assert_eq!(status["group"]["projects"][0]["project_id"], "api");
        assert_eq!(status["group"]["projects"][0]["status"], "Ready");
        assert_eq!(status["group"]["projects"][1]["status"], "MissingDb");
        assert_eq!(status["matching_ready"], true);
        assert_eq!(status["route_matching_ready"], true);
        assert_eq!(status["messaging_matching_ready"], true);
        assert_eq!(status["group_impact_ready"], true);
        assert_eq!(status["group_context_pack_ready"], true);
        assert_eq!(status["service_map_ready"], true);
        assert_eq!(status["architecture_graph_api_ready"], true);
        assert_eq!(status["architecture_graph_ui_ready"], false);

        let summary_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/summary")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("summary");
        assert_eq!(summary_response.status(), StatusCode::OK);
        let summary = response_json(summary_response).await;
        assert_eq!(summary["summary"]["ready_project_count"], 1);
        assert_eq!(summary["summary"]["skipped_project_count"], 1);
        assert_eq!(summary["summary"]["counts"]["routes"], 1);
        assert_eq!(summary["summary"]["federation_ready"], true);
        assert_eq!(summary["summary"]["matching_ready"], true);

        let missing_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/nope/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("missing");
        assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn architecture_group_route_matches_endpoint_filters_and_stays_local() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_route_match_database(
            &web_db,
            "web",
            r#"fetch("/api/orders"); axios.post("/api/users");"#,
            &[("GET", "/dashboard")],
        );
        seed_route_match_database(
            &api_db,
            "api",
            "app routes",
            &[("GET", "/api/orders"), ("POST", "/api/users")],
        );
        write_group_registry(
            &registry_path,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );

        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("control storage"),
            registry_path,
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/route-matches?method=POST&limit=5")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["matching_kind"], "route_api");
        assert_eq!(body["local_only"], true);
        assert_eq!(body["federation_ready"], true);
        assert_eq!(body["route_matching_ready"], true);
        assert_eq!(body["match_count"], 1);
        assert_eq!(body["matches"][0]["method"], "POST");
        assert_eq!(body["matches"][0]["path"], "/api/users");
        assert_eq!(
            body["matches"][0]["candidate"]["relationship_kind"],
            "CallsHttpRoute"
        );
    }

    #[tokio::test]
    async fn architecture_group_impact_endpoint_returns_context_pack() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_route_match_database(
            &web_db,
            "web",
            r#"fetch("/api/orders");"#,
            &[("GET", "/dashboard")],
        );
        seed_route_match_database(
            &api_db,
            "api",
            "app.get('/api/orders', handler);",
            &[("GET", "/api/orders")],
        );
        write_group_registry(
            &registry_path,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );

        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("control storage"),
            registry_path,
        ));
        let response = post_json(
            app,
            "/api/architecture/groups/suite/impact",
            r#"{"seed_type":"route","method":"GET","route_path":"/api/orders","direction":"downstream","max_depth":2,"limit":20,"context_profile":"minimal","include_context_pack":true}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["local_only"], true);
        assert_eq!(body["seed"]["seed_type"], "route");
        assert_eq!(body["direction"], "downstream");
        assert!(body["impacted_project_count"].as_u64().unwrap_or_default() >= 2);
        assert!(
            body["context_pack"]["returned_chars"]
                .as_u64()
                .unwrap_or_default()
                > 0
        );
        assert_eq!(body["context_pack"]["profile"], "minimal");
        assert!(body["edges"].as_array().expect("edges").iter().any(|edge| {
            edge["relationship_kind"] == "CallsHttpRoute"
                && edge["source_phase"] == "route_matching"
        }));
    }

    #[tokio::test]
    async fn architecture_group_graph_endpoint_returns_static_service_relationships() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_route_match_database(
            &web_db,
            "web",
            r#"fetch("/api/orders");"#,
            &[("GET", "/dashboard")],
        );
        seed_route_match_database(
            &api_db,
            "api",
            "app.get('/api/orders', handler);",
            &[("GET", "/api/orders")],
        );
        write_group_registry(
            &registry_path,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );

        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("control storage"),
            registry_path,
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/graph?relationship_kind=CallsHttpRoute&max_nodes=20&max_edges=20")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["local_only"], true);
        assert_eq!(body["graph_api_ready"], true);
        assert_eq!(body["service_map_ready"], true);
        assert!(body["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .any(|node| { node["kind"] == "Project" && node["project_id"] == "api" }));
        assert!(body["edges"]
            .as_array()
            .expect("edges")
            .iter()
            .all(|edge| { edge["relationship_kind"] == "CallsHttpRoute" }));
        assert_eq!(
            body["summary"]["relationship_kind_counts"]["CallsHttpRoute"],
            1
        );
    }

    #[tokio::test]
    async fn architecture_group_service_map_endpoint_summarizes_projects() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let web_db = dir.path().join("web").join(".b3").join("b3.db");
        let api_db = dir.path().join("api").join(".b3").join("b3.db");
        seed_route_match_database(
            &web_db,
            "web",
            r#"fetch("/api/orders");"#,
            &[("GET", "/dashboard")],
        );
        seed_route_match_database(
            &api_db,
            "api",
            "app.get('/api/orders', handler);",
            &[("GET", "/api/orders")],
        );
        write_group_registry(
            &registry_path,
            &[("web", "Web", &web_db), ("api", "API", &api_db)],
            &["web", "api"],
        );

        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("control storage"),
            registry_path,
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/service-map?include_messaging=false&include_dependencies=false&include_infrastructure=false&limit=20")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["service_map_ready"], true);
        assert_eq!(body["architecture_graph_api_ready"], true);
        assert_eq!(body["services"].as_array().expect("services").len(), 2);
        assert!(body["service_edges"]
            .as_array()
            .expect("edges")
            .iter()
            .any(|edge| {
                edge["from_project_id"] == "web"
                    && edge["to_project_id"] == "api"
                    && edge["relationship_kind"] == "CallsHttpRoute"
            }));
    }

    #[tokio::test]
    async fn architecture_group_message_matches_endpoint_filters_and_stays_local() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let producer_db = dir.path().join("producer").join(".b3").join("b3.db");
        let consumer_db = dir.path().join("consumer").join(".b3").join("b3.db");
        seed_message_match_database(
            &producer_db,
            "producer",
            &[
                (
                    "kafka",
                    "outbound",
                    "Producer",
                    Some("orders.created"),
                    None,
                    None,
                ),
                (
                    "rabbitmq",
                    "outbound",
                    "Producer",
                    None,
                    Some("payments.created"),
                    None,
                ),
            ],
        );
        seed_message_match_database(
            &consumer_db,
            "consumer",
            &[
                (
                    "kafka",
                    "inbound",
                    "Consumer",
                    Some("orders.created"),
                    None,
                    None,
                ),
                (
                    "rabbitmq",
                    "inbound",
                    "Consumer",
                    None,
                    Some("payments.created"),
                    None,
                ),
            ],
        );
        write_group_registry(
            &registry_path,
            &[
                ("producer", "Producer", &producer_db),
                ("consumer", "Consumer", &consumer_db),
            ],
            &["producer", "consumer"],
        );

        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("control storage"),
            registry_path,
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/message-matches?broker=kafka&name=orders.created&limit=5")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["matching_kind"], "messaging");
        assert_eq!(body["local_only"], true);
        assert_eq!(body["federation_ready"], true);
        assert_eq!(body["messaging_matching_ready"], true);
        assert_eq!(body["match_count"], 1);
        assert_eq!(body["matches"][0]["broker"], "kafka");
        assert_eq!(body["matches"][0]["channel_name"], "orders.created");
        assert_eq!(
            body["matches"][0]["candidate"]["relationship_kind"],
            "PublishesMessage"
        );
    }

    #[tokio::test]
    async fn architecture_group_dependency_matches_endpoint_filters_and_stays_local() {
        let dir = tempdir().expect("tempdir");
        let registry_path = dir.path().join("registry.json");
        let shared_db = dir.path().join("shared").join(".b3").join("b3.db");
        let app_db = dir.path().join("app").join(".b3").join("b3.db");
        seed_manifest_database(
            &shared_db,
            "shared",
            "package.json",
            r#"{"name":"shared-contracts","version":"1.0.0"}"#,
        );
        seed_manifest_database(
            &app_db,
            "app",
            "package.json",
            r#"{"name":"app","dependencies":{"shared-contracts":"file:../shared"}}"#,
        );
        write_group_registry(
            &registry_path,
            &[("shared", "Shared", &shared_db), ("app", "App", &app_db)],
            &["shared", "app"],
        );

        let app = app(ControlState::from_storage_with_registry_path(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            SqliteStorage::open_in_memory().expect("control storage"),
            registry_path,
        ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/architecture/groups/suite/dependency-matches?kind=package&ecosystem=npm&name=shared-contracts&limit=5")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["matching_kind"], "dependency");
        assert_eq!(body["local_only"], true);
        assert_eq!(body["federation_ready"], true);
        assert_eq!(body["dependency_matching_ready"], true);
        assert_eq!(body["package_contract_infra_matching_ready"], true);
        assert_eq!(body["match_count"], 1);
        assert_eq!(body["matches"][0]["kind"], "Package");
        assert_eq!(body["matches"][0]["ecosystem"], "npm");
        assert_eq!(body["matches"][0]["name"], "shared-contracts");
        assert_eq!(
            body["matches"][0]["candidate"]["relationship_kind"],
            "DependsOnPackage"
        );
    }

    #[tokio::test]
    async fn vector_status_and_stats_are_read_only_and_truthful() {
        let app = empty_app();
        let status_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/vector/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("status response");
        assert_eq!(status_response.status(), StatusCode::OK);
        let status = response_json(status_response).await;
        assert_eq!(status["enabled"], false);
        assert_eq!(status["provider"], "local_hash");
        assert_eq!(status["local_hash_provider_available"], true);
        assert_eq!(status["local_only"], true);
        assert_eq!(status["semantic_search_available"], true);
        assert_eq!(status["semantic_search_ready"], true);
        assert_eq!(status["vector_search_ready"], true);
        assert_eq!(status["hybrid_ranking_available"], true);
        assert_eq!(status["quality_benchmark_ready"], false);

        let providers_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/vector/providers")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("providers response");
        assert_eq!(providers_response.status(), StatusCode::OK);
        let providers = response_json(providers_response).await;
        assert_eq!(providers["providers"][0]["id"], "local_hash");
        assert_eq!(providers["providers"][0]["local_only"], true);

        let stats_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/vector/stats")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("stats response");
        assert_eq!(stats_response.status(), StatusCode::OK);
        let stats = response_json(stats_response).await;
        assert_eq!(stats["documents"], 0);
        assert_eq!(stats["vectors"], 0);
        assert_eq!(stats["providers"].as_array().expect("providers").len(), 0);
        assert_eq!(stats["hosted_vector_database_required"], false);
    }

    #[tokio::test]
    async fn hybrid_search_endpoint_returns_local_fallback_and_explanation_shape() {
        let response = post_json(
            empty_app(),
            "/api/search/hybrid",
            r#"{"query":"order flow","project_id":"default","branch_id":"main","limit":5,"explain":true}"#,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["query"], "order flow");
        assert_eq!(body["local_only"], true);
        assert_eq!(body["semantic_search_ready"], true);
        assert_eq!(body["hybrid_ranking"], true);
        assert_eq!(body["provider_id"], "local_hash");
        assert!(body["results"].as_array().expect("results").is_empty());
        assert!(body["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning
                .as_str()
                .is_some_and(|text| text.contains("No vector data available"))));
    }

    #[tokio::test]
    async fn hybrid_search_endpoint_validates_invalid_requests() {
        let empty_query = post_json(
            empty_app(),
            "/api/search/hybrid",
            r#"{"query":"","limit":5}"#,
        )
        .await;
        let bad_path = post_json(
            empty_app(),
            "/api/search/hybrid",
            r#"{"query":"order","path_prefix":"../outside"}"#,
        )
        .await;

        assert_eq!(empty_query.status(), StatusCode::BAD_REQUEST);
        assert_eq!(bad_path.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn languages_endpoint_reports_known_languages() {
        let response = empty_app()
            .oneshot(
                Request::builder()
                    .uri("/api/languages")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["lsp_enabled"], false);
        assert!(body["known_languages"]
            .to_string()
            .contains("docker-compose"));
        assert!(body["known_languages"].to_string().contains("go.mod"));
        assert!(body["languages"]
            .as_array()
            .expect("languages")
            .iter()
            .any(|language| language["language_id"] == "go"
                && language["support"] == "basic"
                && language["backend"] == "static-go"));
        assert!(body["languages"]
            .as_array()
            .expect("languages")
            .iter()
            .any(|language| language["language_id"] == "python"
                && language["support"] == "basic"
                && language["backend"] == "static-python"));
        for language_id in ["sql", "env", "ksql", "xaml", "threejs_webgl"] {
            let note = body["languages"]
                .as_array()
                .expect("languages")
                .iter()
                .any(|language| {
                    language["language_id"] == language_id
                        && language["notes"]
                            .as_str()
                            .unwrap_or_default()
                            .contains("without")
                });
            let env_note = language_id == "env"
                && body["languages"]
                    .as_array()
                    .expect("languages")
                    .iter()
                    .any(|language| {
                        language["language_id"] == language_id
                            && language["notes"]
                                .as_str()
                                .unwrap_or_default()
                                .contains("never reads")
                    });
            assert!(note || env_note);
        }
    }

    #[tokio::test]
    async fn diagnostics_include_parser_failure_state() {
        let storage = SqliteStorage::open_in_memory().expect("open storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .upsert_project(&project_id, "Project", ".")
            .expect("project");
        storage
            .upsert_branch(&branch_id, &project_id, &BranchMetadata::new("main"))
            .expect("branch");
        storage
            .record_parse_failure(&ParseFailureRecord {
                failure_id: "failure".to_string(),
                project_id,
                branch_id,
                file_id: FileId::new("file"),
                file_path: "src/lib.rs".to_string(),
                file_hash: "hash".to_string(),
                language: Some("rs".to_string()),
                error_kind: "timeout".to_string(),
                error_message: "parser timed out".to_string(),
                stderr_excerpt: None,
                failed_at_unix_ms: 7,
                retry_count: 1,
            })
            .expect("failure");

        let response = app(ControlState::from_storage(
            PathBuf::from("."),
            PathBuf::from(":memory:"),
            storage,
        ))
        .oneshot(
            Request::builder()
                .uri("/api/diagnostics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["parser"]["parse_failure_count"], 1);
        assert_eq!(body["parser"]["isolation_mode"], "in_process");
        assert_eq!(body["parser"]["timeout_ms"], 10_000);
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

    #[test]
    fn init_project_creates_database_and_default_scope() {
        let dir = tempdir().expect("temp dir");
        let options = ProjectCommandOptions {
            project_path: dir.path().join("repo"),
            database_path: dir.path().join("repo").join(".b3").join("b3.db"),
            ..ProjectCommandOptions::default()
        };

        init_project(&options).expect("init");
        let storage = SqliteStorage::open(&options.database_path).expect("open db");
        let summary = storage.graph_summary(None, None).expect("summary");

        assert!(storage.table_exists("projects").expect("projects table"));
        assert_eq!(summary.project_id, Some("default".to_string()));
        assert_eq!(summary.branch_id, Some("main".to_string()));
    }

    #[test]
    fn index_project_indexes_small_rust_fixture_and_reindex_is_safe() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(
            root.join("src").join("lib.rs"),
            "pub fn entry() { helper(); }\nfn helper() {}\n",
        )
        .expect("fixture");
        let options = ProjectCommandOptions {
            project_path: root,
            database_path: dir.path().join("b3.db"),
            ..ProjectCommandOptions::default()
        };

        let first = index_project(&options, false).expect("index");
        let second = index_project(&options, true).expect("reindex");

        assert!(first.files_discovered > 0);
        assert!(first.files_indexed > 0);
        assert!(first.symbols_indexed > 0);
        assert!(first.edges_indexed > 0);
        assert!(second.files_discovered > 0);
    }

    #[tokio::test]
    async fn index_api_run_and_status_return_summary() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(root.join("src").join("lib.rs"), "pub fn run() {}\n").expect("fixture");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);
        let run_body = response_json(run_response).await;
        assert!(run_body["files_discovered"].as_u64().unwrap_or_default() > 0);
        assert!(run_body["symbols_indexed"].as_u64().unwrap_or_default() > 0);

        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/index/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(status_response.status(), StatusCode::OK);
        let status_body = response_json(status_response).await;
        assert_eq!(status_body["status"], "completed");
        assert!(status_body["files_indexed"].as_u64().unwrap_or_default() > 0);
    }

    #[tokio::test]
    async fn capabilities_include_lsp_metadata() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/capabilities")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["language_backend"]["lsp"]["available"], true);
        assert_eq!(body["language_backend"]["lsp"]["enabled"], false);
        assert_eq!(body["language_backend"]["lsp"]["local_only"], true);
    }

    #[tokio::test]
    async fn languages_endpoint_reports_phase_9_2_web_language_support() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/languages")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["lsp_enabled"], false);
        assert_eq!(body["languages"][0]["language_id"], "rust");
        assert_eq!(body["languages"][1]["language_id"], "typescript");
        assert_eq!(body["languages"][1]["support"], "basic");
        assert!(body["languages"][1]["notes"]
            .as_str()
            .unwrap_or_default()
            .contains("Next.js static routes"));
        assert_eq!(body["languages"][3]["language_id"], "jsx");
        assert_eq!(body["languages"][5]["support"], "basic");
        assert!(body["languages"][5]["notes"]
            .as_str()
            .unwrap_or_default()
            .contains("ASP.NET Core Web API"));
    }

    #[tokio::test]
    async fn lsp_status_endpoint_is_disabled_by_default() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/api/lsp/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["enabled"], false);
        assert_eq!(body["status"], "disabled");
        assert_eq!(body["auto_start"], false);
        assert_eq!(body["missing_servers_fatal"], false);
    }

    #[tokio::test]
    async fn routes_endpoint_returns_indexed_rest_routes_and_filters() {
        let response = route_app()
            .oneshot(
                Request::builder()
                    .uri("/api/routes?framework=express&method=GET")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["routes"].as_array().expect("routes").len(), 1);
        assert_eq!(body["routes"][0]["framework"], "express");
        assert_eq!(body["routes"][0]["route_kind"], "api");
        assert_eq!(body["routes"][0]["method"], "GET");
        assert_eq!(body["routes"][0]["path"], "/users");
    }

    #[tokio::test]
    async fn routes_endpoint_includes_nextjs_routes_and_filters() {
        let response = route_app()
            .oneshot(
                Request::builder()
                    .uri("/api/routes?framework=nextjs")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["routes"].as_array().expect("routes").len(), 1);
        assert_eq!(body["routes"][0]["framework"], "nextjs");
        assert_eq!(body["routes"][0]["route_kind"], "page");
        assert_eq!(body["routes"][0]["path"], "/dashboard");
    }

    #[tokio::test]
    async fn routes_endpoint_includes_angular_routes_and_filters() {
        let response = route_app()
            .oneshot(
                Request::builder()
                    .uri("/api/routes?framework=angular")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["routes"].as_array().expect("routes").len(), 1);
        assert_eq!(body["routes"][0]["framework"], "angular");
        assert_eq!(body["routes"][0]["route_kind"], "route");
        assert_eq!(body["routes"][0]["path"], "/users/:id");
        assert_eq!(body["routes"][0]["handler_name"], "UserDetailComponent");
    }

    #[tokio::test]
    async fn routes_endpoint_includes_aspnetcore_routes_and_filters() {
        let response = route_app()
            .oneshot(
                Request::builder()
                    .uri("/api/routes?framework=aspnetcore")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["routes"].as_array().expect("routes").len(), 1);
        assert_eq!(body["routes"][0]["framework"], "aspnetcore");
        assert_eq!(body["routes"][0]["route_kind"], "api");
        assert_eq!(body["routes"][0]["method"], "GET");
        assert_eq!(body["routes"][0]["path"], "/api/users/{id}");
        assert_eq!(body["routes"][0]["class_name"], "UsersController");
    }

    #[tokio::test]
    async fn index_api_exposes_static_aspnetcore_routes() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("Controllers")).expect("controllers");
        fs::write(
            root.join("Controllers").join("UsersController.cs"),
            r#"
                using Microsoft.AspNetCore.Mvc;
                [ApiController]
                [Route("api/[controller]")]
                public class UsersController : ControllerBase
                {
                    public UsersController(IUserService service) {}
                    [HttpGet("{id}")]
                    public IActionResult Get(int id) { return Ok(); }
                }
            "#,
        )
        .expect("controller");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let routes_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/routes?framework=aspnetcore")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(routes_response.status(), StatusCode::OK);
        let body = response_json(routes_response).await;
        assert_eq!(body["routes"][0]["path"], "/api/users/{id}");
        assert_eq!(body["routes"][0]["handler_name"], "Get");
    }

    #[tokio::test]
    async fn index_api_exposes_static_go_symbols_and_routes() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("cmd").join("server")).expect("server");
        fs::write(
            root.join("go.mod"),
            "module github.com/acme/orders\n\ngo 1.22\nrequire github.com/go-chi/chi/v5 v5.0.0\n",
        )
        .expect("go.mod");
        fs::write(
            root.join("cmd").join("server").join("main.go"),
            r#"
                package main
                import "net/http"
                type Server struct {}
                func main() { http.HandleFunc("/health", healthHandler) }
                func healthHandler(w http.ResponseWriter, r *http.Request) {}
            "#,
        )
        .expect("main.go");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let route_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/routes?framework=go_net_http&method=GET")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(route_response.status(), StatusCode::OK);
        let route_body = response_json(route_response).await;
        assert_eq!(route_body["routes"][0]["path"], "/health");
        assert_eq!(route_body["routes"][0]["handler_name"], "healthHandler");

        let symbol_response = post_json(
            app,
            "/api/query/find-symbol",
            r#"{"symbol":"Server","scope":{"project_id":"default"},"limit":10}"#,
        )
        .await;
        assert_eq!(symbol_response.status(), StatusCode::OK);
        let symbol_body = response_json(symbol_response).await;
        assert!(symbol_body["matches"]
            .as_array()
            .expect("matches")
            .iter()
            .any(|record| record["name"] == "Server"));
    }

    #[tokio::test]
    async fn index_api_exposes_static_data_access_metadata() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::create_dir_all(root.join("Repositories")).expect("repositories");
        fs::write(
            root.join("Repositories").join("UserRepository.cs"),
            r#"
                using Microsoft.EntityFrameworkCore;
                using Dapper;
                public class AppDbContext : DbContext
                {
                    public DbSet<User> Users { get; set; }
                }
                public class UserRepository
                {
                    public Task<List<User>> List() => _context.Users.ToListAsync();
                    public Task<User> Find(SqlConnection connection) =>
                        connection.QueryFirstOrDefaultAsync<User>("SELECT * FROM Users");
                }
            "#,
        )
        .expect("csharp fixture");
        fs::write(
            root.join("src").join("data.ts"),
            r#"
                import { PrismaClient } from "@prisma/client";
                import { Entity } from "typeorm";
                import { Model } from "sequelize";
                const prisma = new PrismaClient();
                export async function load(repository) {
                    await prisma.user.findMany();
                    await repository.save(user);
                    await User.findAll();
                }
                @Entity()
                export class User {}
                class AuditLog extends Model {}
            "#,
        )
        .expect("web fixture");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let ef_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/data-access?technology=ef_core")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(ef_response.status(), StatusCode::OK);
        let ef_body = response_json(ef_response).await;
        assert!(ef_body["data_access"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["kind"] == "DbContext" || record["operation"] == "read"));

        let prisma_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/data-access?technology=prisma&operation=read")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(prisma_response.status(), StatusCode::OK);
        let prisma_body = response_json(prisma_response).await;
        assert!(prisma_body["data_access"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["entity_name"] == "user"));
    }

    #[tokio::test]
    async fn index_api_exposes_static_realtime_metadata() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::create_dir_all(root.join("Hubs")).expect("hubs");
        fs::write(
            root.join("src").join("socket.ts"),
            r#"
                import { Server } from "socket.io";
                import * as signalR from "@microsoft/signalr";
                const ws = new WebSocket("ws://localhost/ws");
                ws.addEventListener("message", handler);
                ws.send("hello");
                const io = new Server();
                io.on("connection", socket => {
                    socket.on("join-room", handler);
                    socket.emit("room-joined", data);
                });
                const connection = new signalR.HubConnectionBuilder().withUrl("/chatHub").build();
                connection.on("ReceiveMessage", handler);
                connection.invoke("SendMessage", "u", "m");
            "#,
        )
        .expect("web realtime");
        fs::write(
            root.join("Hubs").join("ChatHub.cs"),
            r#"
                using Microsoft.AspNetCore.SignalR;
                public class ChatHub : Hub
                {
                    public async Task SendMessage(string user, string message)
                    {
                        await Clients.All.SendAsync("ReceiveMessage", user, message);
                    }
                }
            "#,
        )
        .expect("signalr hub");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let socketio_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/realtime?technology=socketio&event=join-room")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(socketio_response.status(), StatusCode::OK);
        let socketio_body = response_json(socketio_response).await;
        assert!(socketio_body["realtime"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["kind"] == "Listener"));

        let signalr_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/realtime?technology=signalr&event=ReceiveMessage")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(signalr_response.status(), StatusCode::OK);
        let signalr_body = response_json(signalr_response).await;
        assert!(signalr_body["realtime"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["source_kind"] == "SignalRSendAsync"));
    }

    #[tokio::test]
    async fn index_api_exposes_static_messaging_metadata() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::create_dir_all(root.join("Messaging")).expect("messaging");
        fs::write(
            root.join("src").join("messaging.ts"),
            r#"
                import amqp from "amqplib";
                import { Kafka } from "kafkajs";
                import { PubSub } from "@google-cloud/pubsub";
                import { MessagePattern } from "@nestjs/microservices";
                export async function run(channel, producer, consumer) {
                    channel.publish("orders.exchange", "order.created", Buffer.from("{}"));
                    channel.consume("orders.queue", handler);
                    await producer.send({ topic: "orders", messages: [] });
                    await consumer.subscribe({ topic: "orders" });
                    const topic = new PubSub().topic("orders");
                    await topic.publishMessage({ json: {} });
                }
                export class OrdersController {
                    @MessagePattern("order.created")
                    handleOrderCreated() {}
                }
            "#,
        )
        .expect("web messaging");
        fs::write(
            root.join("Messaging").join("Worker.cs"),
            r#"
                using RabbitMQ.Client;
                using Confluent.Kafka;
                using Google.Cloud.PubSub.V1;
                public class Worker
                {
                    public async Task Run(IModel channel, IProducer<string,string> producer, IConsumer<string,string> consumer)
                    {
                        channel.BasicPublish(exchange: "orders.exchange", routingKey: "order.created", body: body);
                        channel.BasicConsume(queue: "orders.queue", autoAck: true, consumer: handler);
                        await producer.ProduceAsync("orders", message);
                        consumer.Subscribe("orders");
                        var subscriber = await SubscriberClient.CreateAsync("projects/demo/subscriptions/orders-sub");
                    }
                }
            "#,
        )
        .expect("csharp messaging");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let kafka_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/messaging?technology=kafka&topic=orders")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(kafka_response.status(), StatusCode::OK);
        let kafka_body = response_json(kafka_response).await;
        assert!(kafka_body["messaging"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["kind"] == "Producer"));

        let rabbit_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/messaging?technology=rabbitmq&routing_key=order.created")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(rabbit_response.status(), StatusCode::OK);
        let rabbit_body = response_json(rabbit_response).await;
        assert!(rabbit_body["messaging"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["source_kind"] == "RabbitMqPublish"));
    }

    #[tokio::test]
    async fn index_api_exposes_static_infrastructure_metadata() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("deploy")).expect("deploy");
        fs::write(
            root.join("Dockerfile"),
            "FROM node:20\nENV NODE_ENV=production\nEXPOSE 3000\nCMD [\"npm\", \"start\"]\n",
        )
        .expect("dockerfile");
        fs::write(
            root.join("compose.yaml"),
            r#"
services:
  api:
    image: my-api:latest
    ports:
      - "8080:8080"
    environment:
      - NODE_ENV=development
    depends_on:
      - db
"#,
        )
        .expect("compose");
        fs::write(
            root.join("deploy").join("k8s.yaml"),
            r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: default
  labels:
    app: api
spec:
  selector:
    matchLabels:
      app: api
  template:
    spec:
      containers:
        - name: api
          image: my-api:latest
          ports:
            - containerPort: 8080
---
apiVersion: v1
kind: Service
metadata:
  name: api
spec:
  selector:
    app: api
  ports:
    - port: 80
      targetPort: 8080
"#,
        )
        .expect("kubernetes");
        fs::write(
            root.join("main.tf"),
            r#"
provider "google" {
  project = "demo"
  region = "asia-southeast1"
}

resource "google_container_cluster" "primary" {
  name = "b3-cluster"
  location = "asia-southeast1"
}

module "network" {
  source = "./modules/network"
}

variable "project_id" {}
output "cluster_name" {
  value = google_container_cluster.primary.name
}
"#,
        )
        .expect("terraform");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let compose_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/infrastructure?technology=docker_compose&name=api")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(compose_response.status(), StatusCode::OK);
        let compose_body = response_json(compose_response).await;
        assert!(compose_body["infrastructure"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["image"] == "my-api:latest"));

        let gke_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/infrastructure?technology=gke&kind=cluster&name=primary")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(gke_response.status(), StatusCode::OK);
        let gke_body = response_json(gke_response).await;
        assert!(gke_body["infrastructure"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["source_kind"] == "GkeTerraformCluster"));
    }

    #[tokio::test]
    async fn index_api_exposes_static_wpf_metadata_and_scoped_preview() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("Views")).expect("views");
        fs::create_dir_all(root.join("ViewModels")).expect("viewmodels");
        fs::write(
            root.join("App.csproj"),
            r#"<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>WinExe</OutputType><TargetFramework>net8.0-windows</TargetFramework><UseWPF>true</UseWPF></PropertyGroup></Project>"#,
        )
        .expect("csproj");
        fs::write(
            root.join("Views").join("MainWindow.xaml"),
            r#"
                <Window x:Class="App.Views.MainWindow"
                        xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
                        xmlns:vm="clr-namespace:App.ViewModels">
                    <Window.DataContext>
                        <vm:MainViewModel />
                    </Window.DataContext>
                    <TextBox Text="{Binding UserName}" />
                    <Button Command="{Binding SaveCommand}" />
                    <ResourceDictionary Source="Themes/Colors.xaml" />
                </Window>
            "#,
        )
        .expect("xaml");
        fs::write(
            root.join("Views").join("MainWindow.xaml.cs"),
            "public partial class MainWindow : Window { public MainWindow() { DataContext = new MainViewModel(); } }",
        )
        .expect("code behind");
        fs::write(
            root.join("ViewModels").join("MainViewModel.cs"),
            "using System.Windows.Input; public class MainViewModel { public ICommand SaveCommand { get; } }",
        )
        .expect("viewmodel");
        let database_path = dir.path().join("b3.db");
        let storage = SqliteStorage::open(&database_path).expect("storage");
        let app = app(ControlState::from_storage(root, database_path, storage));

        let run_response = post_json(app.clone(), "/api/index/run", "{}").await;
        assert_eq!(run_response.status(), StatusCode::OK);

        let wpf_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/wpf?kind=window&binding=UserName&command=SaveCommand")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(wpf_response.status(), StatusCode::OK);
        let wpf_body = response_json(wpf_response).await;
        assert!(wpf_body["wpf"]
            .as_array()
            .expect("records")
            .iter()
            .any(|record| record["x_class"] == "App.Views.MainWindow"));

        let preview_response = post_json(
            app,
            "/api/index/preview",
            r#"{"scope":"framework:wpf","dry_run":true}"#,
        )
        .await;
        assert_eq!(preview_response.status(), StatusCode::OK);
        let preview_body = response_json(preview_response).await;
        assert!(
            preview_body["matched_files"]
                .as_u64()
                .expect("matched files")
                >= 2
        );
        assert!(preview_body["matched_frameworks"]
            .as_array()
            .expect("frameworks")
            .iter()
            .any(|framework| framework == "wpf"));
    }

    #[tokio::test]
    async fn components_endpoint_returns_indexed_react_components_and_filters() {
        let response = component_app()
            .oneshot(
                Request::builder()
                    .uri("/api/components?framework=react&name=ProductCard")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["components"].as_array().expect("components").len(), 1);
        assert_eq!(body["components"][0]["name"], "ProductCard");
        assert_eq!(body["components"][0]["framework"], "react");
        assert_eq!(body["components"][0]["props_type_name"], "ProductCardProps");
        assert_eq!(body["components"][0]["hooks"][0], "useState");
        assert_eq!(body["components"][0]["usages"][0], "Badge");
    }

    #[tokio::test]
    async fn components_endpoint_includes_angular_components_and_filters() {
        let response = component_app()
            .oneshot(
                Request::builder()
                    .uri("/api/components?framework=angular&name=UserCardComponent")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["components"].as_array().expect("components").len(), 1);
        assert_eq!(body["components"][0]["name"], "UserCardComponent");
        assert_eq!(body["components"][0]["framework"], "angular");
        assert_eq!(body["components"][0]["component_kind"], "component");
    }

    #[tokio::test]
    async fn realtime_endpoint_returns_indexed_socket_metadata_and_filters() {
        let response = realtime_app()
            .oneshot(
                Request::builder()
                    .uri("/api/realtime?technology=socketio&kind=listener&event=message")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["realtime"].as_array().expect("realtime").len(), 1);
        assert_eq!(body["realtime"][0]["technology"], "socketio");
        assert_eq!(body["realtime"][0]["kind"], "Listener");
        assert_eq!(body["realtime"][0]["event_name"], "message");
        assert_eq!(body["realtime"][0]["source_kind"], "SocketIoOn");
    }

    #[tokio::test]
    async fn messaging_endpoint_returns_indexed_metadata_and_filters() {
        let response = messaging_app()
            .oneshot(
                Request::builder()
                    .uri("/api/messaging?technology=kafka&kind=producer&topic=orders&routing_key=order.created")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["messaging"].as_array().expect("messaging").len(), 1);
        assert_eq!(body["messaging"][0]["technology"], "kafka");
        assert_eq!(body["messaging"][0]["kind"], "Producer");
        assert_eq!(body["messaging"][0]["topic"], "orders");
        assert_eq!(body["messaging"][0]["source_kind"], "KafkaProducerSend");
    }

    #[tokio::test]
    async fn infrastructure_endpoint_returns_indexed_metadata_and_filters() {
        let response = infrastructure_app()
            .oneshot(
                Request::builder()
                    .uri("/api/infrastructure?technology=kubernetes&kind=deployment&name=api")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(
            body["infrastructure"]
                .as_array()
                .expect("infrastructure")
                .len(),
            1
        );
        assert_eq!(body["infrastructure"][0]["technology"], "kubernetes");
        assert_eq!(body["infrastructure"][0]["kind"], "Deployment");
        assert_eq!(body["infrastructure"][0]["name"], "api");
        assert_eq!(
            body["infrastructure"][0]["source_kind"],
            "KubernetesDeployment"
        );
    }

    #[tokio::test]
    async fn wpf_endpoint_returns_indexed_metadata_and_filters() {
        let response = wpf_app()
            .oneshot(
                Request::builder()
                    .uri("/api/wpf?kind=window&binding=UserName&command=SaveCommand")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["wpf"].as_array().expect("wpf").len(), 1);
        assert_eq!(body["wpf"][0]["technology"], "wpf");
        assert_eq!(body["wpf"][0]["kind"], "Window");
        assert_eq!(body["wpf"][0]["x_class"], "App.Views.MainWindow");
        assert_eq!(body["wpf"][0]["code_behind"], "Views/MainWindow.xaml.cs");
        assert_eq!(body["wpf"][0]["binding_paths"][0], "UserName");
        assert_eq!(body["wpf"][0]["command_bindings"][0], "SaveCommand");
        assert_eq!(body["wpf"][0]["resource_sources"][0], "Themes/Colors.xaml");
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
    async fn edit_preview_is_dry_run_and_apply_requires_explicit_mode() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(root.join("src").join("lib.rs"), "pub fn run() {}\n").expect("source");
        let app = edit_app(root.clone(), dir.path().join("b3.db"));

        let preview = post_json(
            app.clone(),
            "/api/edit/preview",
            r#"{
                "target":{"kind":"symbol","file_path":"src/lib.rs","symbol_name":"run"},
                "operation":"replace_symbol_body",
                "new_text":"pub fn run() { 1 }\n"
            }"#,
        )
        .await;
        assert_eq!(preview.status(), StatusCode::OK);
        let preview_body = response_json(preview).await;
        assert_eq!(preview_body["dry_run"], true);
        assert!(preview_body["preview"]["patch"]
            .as_str()
            .expect("patch")
            .contains("+pub fn run() { 1 }"));
        assert_eq!(
            fs::read_to_string(root.join("src").join("lib.rs")).expect("source"),
            "pub fn run() {}\n"
        );

        let rejected = post_json(
            app.clone(),
            "/api/edit/apply",
            r#"{
                "target":{"kind":"symbol","file_path":"src/lib.rs","symbol_name":"run"},
                "operation":"replace_symbol_body",
                "new_text":"pub fn run() { 1 }\n"
            }"#,
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

        let applied = post_json(
            app,
            "/api/edit/apply",
            r#"{
                "target":{"kind":"symbol","file_path":"src/lib.rs","symbol_name":"run"},
                "operation":"replace_symbol_body",
                "mode":"apply",
                "dry_run":false,
                "new_text":"pub fn run() { 1 }\n"
            }"#,
        )
        .await;
        assert_eq!(applied.status(), StatusCode::OK);
        let applied_body = response_json(applied).await;
        assert_eq!(applied_body["applied"], true);
        assert_eq!(
            fs::read_to_string(root.join("src").join("lib.rs")).expect("source"),
            "pub fn run() { 1 }\n"
        );
        assert_eq!(
            applied_body["backup_paths"]
                .as_array()
                .expect("backups")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn rename_preview_is_dry_run_and_apply_requires_explicit_mode() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("src")).expect("src");
        fs::write(
            root.join("src").join("lib.rs"),
            "pub fn old_name() {}\nfn caller() { old_name(); }\n",
        )
        .expect("source");
        let app = rename_app(root.clone(), dir.path().join("b3.db"));

        let preview = post_json(
            app.clone(),
            "/api/refactor/rename/preview",
            r#"{
                "target":{"kind":"symbol","file_path":"src/lib.rs","symbol_name":"old_name"},
                "old_name":"old_name",
                "new_name":"new_name",
                "scope":"single_file"
            }"#,
        )
        .await;
        assert_eq!(preview.status(), StatusCode::OK);
        let preview_body = response_json(preview).await;
        assert_eq!(preview_body["dry_run"], true);
        assert_eq!(preview_body["preview"]["occurrence_count"], 2);
        assert!(preview_body["preview"]["patch"]
            .as_str()
            .expect("patch")
            .contains("+pub fn new_name()"));
        assert!(fs::read_to_string(root.join("src").join("lib.rs"))
            .expect("source")
            .contains("old_name"));

        let rejected = post_json(
            app.clone(),
            "/api/refactor/rename/apply",
            r#"{
                "target":{"kind":"symbol","file_path":"src/lib.rs","symbol_name":"old_name"},
                "old_name":"old_name",
                "new_name":"new_name",
                "scope":"single_file"
            }"#,
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

        let applied = post_json(
            app,
            "/api/refactor/rename/apply",
            r#"{
                "target":{"kind":"symbol","file_path":"src/lib.rs","symbol_name":"old_name"},
                "old_name":"old_name",
                "new_name":"new_name",
                "scope":"single_file",
                "mode":"apply",
                "dry_run":false
            }"#,
        )
        .await;
        assert_eq!(applied.status(), StatusCode::OK);
        let applied_body = response_json(applied).await;
        assert_eq!(applied_body["applied"], true);
        assert_eq!(applied_body["reindex_recommended"], true);
        assert_eq!(
            applied_body["backup_paths"]
                .as_array()
                .expect("backups")
                .len(),
            1
        );
        let source = fs::read_to_string(root.join("src").join("lib.rs")).expect("source");
        assert!(source.contains("new_name"));
        assert!(!source.contains("old_name()"));
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
