//! Incremental indexing pipeline boundary and local implementation.
//!
//! This crate owns repository discovery, ignore filtering, file hashing,
//! incremental indexing decisions, parser worker isolation contracts, symbol and
//! relationship extraction contracts, queueing, cancellation, and index events.
//! It does not implement retrieval ranking, embedding generation, UI features,
//! or MCP request handling.

mod backend_languages;
mod config_files;
mod csharp;
mod data_access;
mod data_files;
mod dispatch;
mod dotnet_desktop;
pub mod embedding;
mod go;
mod infrastructure;
pub mod lsp;
mod messaging;
mod metadata_helpers;
mod parser_worker;
mod path_detection;
mod pipeline;
mod realtime;
pub mod scope;
mod systems_languages;
mod web;
mod web_files;

pub(crate) use dispatch::{
    collect_contains_relationships, collect_import_relationships, has_parent_kind, index_edge,
    node_text, one_based_row,
};
pub use dispatch::{DefaultLanguagePack, NoopTreeSitterParser, RustLanguagePack, WebLanguagePack};
pub(crate) use metadata_helpers::{
    edge_kind_name, edge_provenance_name, escape_metadata, escape_metadata_semicolon,
    node_kind_name, parse_edge_kind, parse_edge_provenance, parse_node_kind,
    prefixed_metadata_value, prefixed_metadata_value_semicolon, to_contract_error,
};
pub use parser_worker::{
    parse_worker_json_line, ParserErrorDto, ParserFailure, ParserFailureKind, ParserJobRequest,
    ParserJobResponse, ParserRelationshipDto, ParserSymbolDto, ParserWorkerManager,
    ParserWorkerOutput,
};
pub(crate) use path_detection::{
    hash_file, language_from_path, now_unix_ms, relative_path, stable_id,
};
pub use pipeline::{
    LocalIndexer, NoopRelationshipExtractor, NoopSymbolExtractor, TreeSitterPipelineParser,
};

#[cfg(test)]
pub(crate) use b3_core::{
    DomainEvent, FileRecord, IndexStore, IndexedFileRecord, ParseFailureRecord, SymbolRecord,
};
#[cfg(test)]
pub(crate) use backend_languages::{
    backend_metadata_value, route_metadata_value as backend_route_metadata_value,
};
pub use csharp::detect_csproj_technologies as detect_dotnet_project_technologies;
#[cfg(test)]
pub(crate) use csharp::{aspnet_metadata_value, detect_csproj_technologies};
#[cfg(test)]
pub(crate) use data_access::data_access_metadata_value;
pub use data_access::{
    detect_csproj_data_access_technologies, detect_package_json_data_access_technologies,
};
pub use dotnet_desktop::detect_wpf_project_technologies;
#[cfg(test)]
pub(crate) use dotnet_desktop::wpf_metadata_value;
#[cfg(test)]
pub(crate) use go::{detect_go_mod_technologies, go_metadata_value};
#[cfg(test)]
pub(crate) use infrastructure::infrastructure_metadata_value;
#[cfg(test)]
pub(crate) use messaging::messaging_metadata_value;
pub use messaging::{
    detect_csproj_messaging_technologies, detect_package_json_messaging_technologies,
};
#[cfg(test)]
pub(crate) use realtime::realtime_metadata_value;
pub use realtime::{
    detect_csproj_realtime_technologies, detect_package_json_realtime_technologies,
};
#[cfg(test)]
pub(crate) use web::{angular_metadata_value, component_metadata_value, route_metadata_value};
pub use web::{
    detect_angular_config_path, detect_nextjs_config_path, detect_package_json_technologies,
    resolve_web_import_path,
};
#[cfg(test)]
mod tests;

use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::mpsc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use b3_core::{
    BranchId, BranchMetadata, ContractError, ContractResult, EdgeId, EdgeKind, EdgeProvenance,
    FileId, GraphEdgeMetadata, IndexJob, IndexJobId, NodeKind, ProjectId, SymbolId,
};
pub use b3_core::{
    IndexJobQueue, IndexStore as CoreIndexStore, IndexSummary as CoreIndexSummary,
    Indexer as CoreIndexer, LanguageBackendMetadata,
};
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
    EventKind as NotifyEventKind, RecursiveMode, Watcher,
};
pub(crate) use tree_sitter::{Node, Parser, Point};

const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRequest {
    pub project_id: ProjectId,
    pub root_path: String,
    pub branch: BranchMetadata,
    pub parser_isolation: ParserIsolation,
}

impl IndexRequest {
    pub fn new(
        project_id: ProjectId,
        root_path: impl Into<String>,
        branch: BranchMetadata,
    ) -> Self {
        Self {
            project_id,
            root_path: root_path.into(),
            branch,
            parser_isolation: ParserIsolation::InProcess,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserIsolation {
    InProcess,
    SubprocessWorker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStage {
    Discovery,
    IgnoreFiltering,
    LanguageDetection,
    FileHashing,
    TreeSitterParsing,
    SymbolExtraction,
    RelationshipExtraction,
    GraphUpdate,
    FtsUpdate,
    EmbeddingQueue,
    CacheUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerConfig {
    pub ignore: IgnoreRules,
    pub max_file_bytes: u64,
    pub max_workers: usize,
    pub branch_id: BranchId,
    pub parser_isolation: ParserIsolation,
    pub parser_timeout_ms: u64,
    pub parser_max_retries: usize,
    pub parser_worker_path: Option<PathBuf>,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            ignore: IgnoreRules::default(),
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_workers: 1,
            branch_id: BranchId::new("default"),
            parser_isolation: ParserIsolation::InProcess,
            parser_timeout_ms: 10_000,
            parser_max_retries: 1,
            parser_worker_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreRules {
    ignored_dirs: HashSet<String>,
    ignored_extensions: HashSet<String>,
}

impl Default for IgnoreRules {
    fn default() -> Self {
        Self {
            ignored_dirs: [
                ".git",
                "target",
                "node_modules",
                ".next",
                "dist",
                "out",
                ".b3",
                ".qdrant",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            ignored_extensions: ["db", "sqlite", "sqlite3", "log"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }
}

impl IgnoreRules {
    pub fn should_skip(&self, path: &Path) -> Option<String> {
        for component in path.components() {
            let value = component.as_os_str().to_string_lossy();
            if self.ignored_dirs.contains(value.as_ref()) {
                return Some(format!("ignored directory: {value}"));
            }
        }

        let extension = path.extension()?.to_string_lossy().to_lowercase();
        if self.ignored_extensions.contains(extension.as_str()) {
            return Some(format!("ignored extension: {extension}"));
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub id: FileId,
    pub path: PathBuf,
    pub relative_path: String,
    pub content_hash: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFile {
    pub file_id: FileId,
    pub language: Option<String>,
    pub symbols: Vec<ExtractedSymbol>,
    pub relationships: Vec<ExtractedRelationship>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSymbol {
    pub id: SymbolId,
    pub file_id: FileId,
    pub name: String,
    pub kind: NodeKind,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedRelationship {
    pub id: EdgeId,
    pub from_symbol: SymbolId,
    pub to_symbol: SymbolId,
    pub kind: EdgeKind,
    pub metadata: GraphEdgeMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMetadata {
    pub framework: String,
    pub route_kind: String,
    pub method: String,
    pub path: String,
    pub file_path: String,
    pub symbol_id: Option<SymbolId>,
    pub handler_name: Option<String>,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentMetadata {
    pub framework: String,
    pub export_kind: Option<String>,
    pub component_kind: String,
    pub props_type_name: Option<String>,
    pub hooks: Vec<String>,
    pub usages: Vec<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataAccessMetadata {
    pub technology: String,
    pub kind: String,
    pub file_path: String,
    pub symbol_id: Option<SymbolId>,
    pub class_name: Option<String>,
    pub method_name: Option<String>,
    pub entity_name: Option<String>,
    pub context_name: Option<String>,
    pub repository_name: Option<String>,
    pub operation: Option<String>,
    pub query_text: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeMetadata {
    pub technology: String,
    pub kind: String,
    pub direction: String,
    pub event_name: Option<String>,
    pub channel_name: Option<String>,
    pub hub_name: Option<String>,
    pub method_name: Option<String>,
    pub endpoint: Option<String>,
    pub file_path: String,
    pub symbol_id: Option<SymbolId>,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessagingMetadata {
    pub technology: String,
    pub kind: String,
    pub direction: String,
    pub topic: Option<String>,
    pub queue: Option<String>,
    pub exchange: Option<String>,
    pub routing_key: Option<String>,
    pub pattern: Option<String>,
    pub consumer_group: Option<String>,
    pub file_path: String,
    pub symbol_id: Option<SymbolId>,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub method_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfrastructureMetadata {
    pub technology: String,
    pub kind: String,
    pub name: Option<String>,
    pub resource_type: Option<String>,
    pub provider: Option<String>,
    pub image: Option<String>,
    pub service_name: Option<String>,
    pub container_name: Option<String>,
    pub namespace: Option<String>,
    pub ports: Vec<String>,
    pub env_keys: Vec<String>,
    pub labels: Vec<String>,
    pub selectors: Vec<String>,
    pub file_path: String,
    pub symbol_id: Option<SymbolId>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TechnologyKind {
    Language,
    WebBackend,
    WebFrontend,
    Realtime,
    Messaging,
    Orm,
    Cloud,
    Infrastructure,
    BuildTool,
    Testing,
    Runtime,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TechnologySupportLevel {
    DetectOnly,
    Basic,
    Good,
    Advanced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TechnologyCapability {
    DetectPackage,
    DetectImport,
    ExtractSymbols,
    ExtractRoutes,
    ExtractComponents,
    ExtractRealtime,
    ExtractMessaging,
    ExtractInfrastructure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedTechnology {
    pub id: String,
    pub name: String,
    pub kind: TechnologyKind,
    pub support_level: TechnologySupportLevel,
    pub capabilities: Vec<TechnologyCapability>,
    pub source: String,
}

const NEXTJS_HTTP_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseInput {
    pub file_id: FileId,
    pub path: PathBuf,
    pub source: String,
}

pub trait TreeSitterParser: Send + Sync {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile>;
}

pub trait SymbolExtractor: Send + Sync {
    fn extract_symbols(&self, input: &ParseInput) -> ContractResult<Vec<ExtractedSymbol>>;
}

pub trait RelationshipExtractor: Send + Sync {
    fn extract_relationships(
        &self,
        symbols: &[ExtractedSymbol],
        input: &ParseInput,
    ) -> ContractResult<Vec<ExtractedRelationship>>;
}

pub trait FileWatcher: Send + Sync {
    fn watch(&self, root: PathBuf) -> ContractResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchConfig {
    pub enabled: bool,
    pub debounce_ms: u64,
    pub max_batch_size: usize,
    pub ignore: IgnoreRules,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            debounce_ms: 500,
            max_batch_size: 100,
            ignore: IgnoreRules::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Changed,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchEvent {
    pub kind: WatchEventKind,
    pub path: PathBuf,
    pub new_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebouncedBatch {
    pub events: Vec<WatchEvent>,
}

#[derive(Debug)]
pub struct WatchDebouncer {
    delay: Duration,
    max_batch_size: usize,
    pending: Vec<WatchEvent>,
    last_event_at: Option<Instant>,
}

impl WatchDebouncer {
    pub fn new(delay: Duration, max_batch_size: usize) -> Self {
        Self {
            delay,
            max_batch_size: max_batch_size.max(1),
            pending: Vec::new(),
            last_event_at: None,
        }
    }

    pub fn push(&mut self, event: WatchEvent) -> Option<DebouncedBatch> {
        self.pending.retain(|pending| pending.path != event.path);
        self.pending.push(event);
        self.last_event_at = Some(Instant::now());
        if self.pending.len() >= self.max_batch_size {
            return self.flush();
        }
        None
    }

    pub fn flush_if_ready(&mut self) -> Option<DebouncedBatch> {
        if self
            .last_event_at
            .is_some_and(|last_event_at| last_event_at.elapsed() >= self.delay)
        {
            return self.flush();
        }
        None
    }

    pub fn flush(&mut self) -> Option<DebouncedBatch> {
        if self.pending.is_empty() {
            return None;
        }
        self.last_event_at = None;
        Some(DebouncedBatch {
            events: std::mem::take(&mut self.pending),
        })
    }
}

pub fn classify_notify_event(event: &notify::Event) -> Vec<WatchEvent> {
    match &event.kind {
        NotifyEventKind::Create(CreateKind::File) | NotifyEventKind::Create(CreateKind::Any) => {
            event
                .paths
                .iter()
                .cloned()
                .map(|path| WatchEvent {
                    kind: WatchEventKind::Created,
                    path,
                    new_path: None,
                })
                .collect()
        }
        NotifyEventKind::Modify(ModifyKind::Data(_)) | NotifyEventKind::Modify(ModifyKind::Any) => {
            event
                .paths
                .iter()
                .cloned()
                .map(|path| WatchEvent {
                    kind: WatchEventKind::Changed,
                    path,
                    new_path: None,
                })
                .collect()
        }
        NotifyEventKind::Modify(ModifyKind::Name(RenameMode::Both)) if event.paths.len() >= 2 => {
            vec![WatchEvent {
                kind: WatchEventKind::Renamed,
                path: event.paths[0].clone(),
                new_path: Some(event.paths[1].clone()),
            }]
        }
        NotifyEventKind::Remove(RemoveKind::File) | NotifyEventKind::Remove(RemoveKind::Any) => {
            event
                .paths
                .iter()
                .cloned()
                .map(|path| WatchEvent {
                    kind: WatchEventKind::Deleted,
                    path,
                    new_path: None,
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone)]
pub struct NotifyFileWatcher {
    config: WatchConfig,
}

impl NotifyFileWatcher {
    pub fn new(config: WatchConfig) -> Self {
        Self { config }
    }

    pub fn collect_batch(
        &self,
        root: &Path,
        timeout: Duration,
    ) -> ContractResult<Option<DebouncedBatch>> {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(sender).map_err(to_contract_error)?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(to_contract_error)?;
        let mut debouncer = WatchDebouncer::new(
            Duration::from_millis(self.config.debounce_ms),
            self.config.max_batch_size,
        );
        let started = Instant::now();

        while started.elapsed() < timeout {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(event)) => {
                    for event in classify_notify_event(&event) {
                        if self.config.ignore.should_skip(&event.path).is_none() {
                            if let Some(batch) = debouncer.push(event) {
                                return Ok(Some(batch));
                            }
                        }
                    }
                }
                Ok(Err(error)) => return Err(to_contract_error(error)),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(batch) = debouncer.flush_if_ready() {
                        return Ok(Some(batch));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        Ok(debouncer.flush())
    }
}

impl FileWatcher for NotifyFileWatcher {
    fn watch(&self, root: PathBuf) -> ContractResult<()> {
        let _watcher = notify::recommended_watcher(|_event: notify::Result<notify::Event>| {})
            .map_err(to_contract_error)?;
        if self.config.ignore.should_skip(&root).is_some() {
            return Ok(());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct LocalIndexJobQueue {
    capacity: usize,
    next_id: AtomicUsize,
    jobs: Mutex<VecDeque<(IndexJobId, IndexJob)>>,
}

impl LocalIndexJobQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_id: AtomicUsize::new(1),
            jobs: Mutex::new(VecDeque::new()),
        }
    }

    pub fn pop(&self) -> ContractResult<Option<(IndexJobId, IndexJob)>> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| ContractError::new("index queue lock poisoned"))?;
        Ok(jobs.pop_front())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWorkerPool {
    max_workers: usize,
}

impl BoundedWorkerPool {
    pub fn new(max_workers: usize) -> Self {
        Self {
            max_workers: max_workers.max(1),
        }
    }

    pub fn max_workers(&self) -> usize {
        self.max_workers
    }

    pub fn batches<T>(&self, items: &[T]) -> Vec<std::ops::Range<usize>> {
        if items.is_empty() {
            return Vec::new();
        }

        let batch_size = items.len().div_ceil(self.max_workers);
        (0..items.len())
            .step_by(batch_size.max(1))
            .map(|start| {
                let end = (start + batch_size).min(items.len());
                start..end
            })
            .collect()
    }
}

impl b3_core::IndexJobQueue for LocalIndexJobQueue {
    fn enqueue(&self, job: IndexJob) -> ContractResult<IndexJobId> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| ContractError::new("index queue lock poisoned"))?;

        if jobs.len() >= self.capacity {
            return Err(ContractError::new("index queue is full"));
        }

        let id = IndexJobId::new(format!(
            "index-job-{}",
            self.next_id.fetch_add(1, Ordering::SeqCst)
        ));
        jobs.push_back((id.clone(), job));
        Ok(id)
    }
}
