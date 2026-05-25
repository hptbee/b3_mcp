//! Incremental indexing pipeline boundary and local implementation.
//!
//! This crate owns repository discovery, ignore filtering, file hashing,
//! incremental indexing decisions, parser worker isolation contracts, symbol and
//! relationship extraction contracts, queueing, cancellation, and index events.
//! It does not implement retrieval ranking, embedding generation, UI features,
//! or MCP request handling.

mod csharp;
mod data_access;
pub mod lsp;
mod realtime;
mod web;

pub use csharp::detect_csproj_technologies as detect_dotnet_project_technologies;
#[cfg(test)]
pub(crate) use csharp::{aspnet_metadata_value, detect_csproj_technologies};
#[cfg(test)]
pub(crate) use data_access::data_access_metadata_value;
pub use data_access::{
    detect_csproj_data_access_technologies, detect_package_json_data_access_technologies,
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
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use b3_core::{
    BranchId, BranchMetadata, ContractError, ContractResult, DomainEvent, EdgeConfidence, EdgeId,
    EdgeKind, EdgeProvenance, EventBus, FileDiscovered, FileId, FileParsed, FileRecord,
    FileSkipped, GraphEdgeMetadata, IndexCompleted, IndexJob, IndexJobId, IndexStarted, IndexStore,
    IndexSummary, IndexedEdgeRecord, IndexedFileRecord, Indexer, NodeKind, ParseFailed,
    ParseFailureRecord, ParseFailureRecorded, ParseRetried, ParserCrashed, ParserWorkerCompleted,
    ParserWorkerCrashed, ParserWorkerStarted, ParserWorkerTimeout, ProjectId, SymbolId,
    SymbolRecord,
};
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
    EventKind as NotifyEventKind, RecursiveMode, Watcher,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser, Point};

pub use b3_core::{
    IndexJobQueue, IndexStore as CoreIndexStore, IndexSummary as CoreIndexSummary,
    Indexer as CoreIndexer, LanguageBackendMetadata,
};

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
    ExtractRoutes,
    ExtractComponents,
    ExtractRealtime,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserJobRequest {
    pub project_id: String,
    pub branch_id: String,
    pub file_id: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserJobResponse {
    pub file_id: String,
    pub language: Option<String>,
    pub symbols: Vec<ParserSymbolDto>,
    pub relationships: Vec<ParserRelationshipDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserSymbolDto {
    pub id: String,
    pub file_id: String,
    pub name: String,
    pub kind: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserRelationshipDto {
    pub id: String,
    pub from_symbol: String,
    pub to_symbol: String,
    pub kind: String,
    pub confidence_bps: u16,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserErrorDto {
    pub kind: String,
    pub message: String,
    pub stderr_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ParserWorkerOutput {
    Parsed(ParserJobResponse),
    Failed(ParserErrorDto),
}

impl ParserJobRequest {
    fn from_input(project_id: &ProjectId, branch_id: &BranchId, input: &ParseInput) -> Self {
        Self {
            project_id: project_id.as_str().to_string(),
            branch_id: branch_id.as_str().to_string(),
            file_id: input.file_id.as_str().to_string(),
            path: input.path.to_string_lossy().to_string(),
            source: input.source.clone(),
        }
    }
}

impl From<ParsedFile> for ParserJobResponse {
    fn from(value: ParsedFile) -> Self {
        Self {
            file_id: value.file_id.as_str().to_string(),
            language: value.language,
            symbols: value
                .symbols
                .into_iter()
                .map(ParserSymbolDto::from)
                .collect(),
            relationships: value
                .relationships
                .into_iter()
                .map(ParserRelationshipDto::from)
                .collect(),
        }
    }
}

impl From<ExtractedSymbol> for ParserSymbolDto {
    fn from(value: ExtractedSymbol) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            file_id: value.file_id.as_str().to_string(),
            name: value.name,
            kind: node_kind_name(value.kind).to_string(),
            start_byte: value.start_byte,
            end_byte: value.end_byte,
            start_line: value.start_line,
            start_column: value.start_column,
            end_line: value.end_line,
            end_column: value.end_column,
            visibility: value.visibility,
        }
    }
}

impl From<ParserSymbolDto> for ExtractedSymbol {
    fn from(value: ParserSymbolDto) -> Self {
        Self {
            id: SymbolId::new(value.id),
            file_id: FileId::new(value.file_id),
            name: value.name,
            kind: parse_node_kind(&value.kind),
            start_byte: value.start_byte,
            end_byte: value.end_byte,
            start_line: value.start_line,
            start_column: value.start_column,
            end_line: value.end_line,
            end_column: value.end_column,
            visibility: value.visibility,
        }
    }
}

impl From<ExtractedRelationship> for ParserRelationshipDto {
    fn from(value: ExtractedRelationship) -> Self {
        Self {
            id: value.id.as_str().to_string(),
            from_symbol: value.from_symbol.as_str().to_string(),
            to_symbol: value.to_symbol.as_str().to_string(),
            kind: edge_kind_name(value.kind).to_string(),
            confidence_bps: value.metadata.confidence.basis_points(),
            provenance: edge_provenance_name(value.metadata.provenance).to_string(),
        }
    }
}

impl From<ParserRelationshipDto> for ExtractedRelationship {
    fn from(value: ParserRelationshipDto) -> Self {
        Self {
            id: EdgeId::new(value.id),
            from_symbol: SymbolId::new(value.from_symbol),
            to_symbol: SymbolId::new(value.to_symbol),
            kind: parse_edge_kind(&value.kind),
            metadata: GraphEdgeMetadata {
                confidence: EdgeConfidence::from_basis_points(value.confidence_bps),
                provenance: parse_edge_provenance(&value.provenance),
                created_at_unix_ms: 0,
                updated_at_unix_ms: 0,
            },
        }
    }
}

impl From<ParserJobResponse> for ParsedFile {
    fn from(value: ParserJobResponse) -> Self {
        Self {
            file_id: FileId::new(value.file_id),
            language: value.language,
            symbols: value
                .symbols
                .into_iter()
                .map(ExtractedSymbol::from)
                .collect(),
            relationships: value
                .relationships
                .into_iter()
                .map(ExtractedRelationship::from)
                .collect(),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParserFailureKind {
    ParseError,
    Timeout,
    WorkerCrash,
    WorkerIo,
}

impl ParserFailureKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ParseError => "parse_error",
            Self::Timeout => "timeout",
            Self::WorkerCrash => "worker_crash",
            Self::WorkerIo => "worker_io",
        }
    }

    fn retryable(&self) -> bool {
        matches!(self, Self::Timeout | Self::WorkerCrash | Self::WorkerIo)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserFailure {
    pub kind: ParserFailureKind,
    pub message: String,
    pub stderr_excerpt: Option<String>,
    pub exit_code: Option<i32>,
    pub retry_count: usize,
}

impl ParserFailure {
    fn parse_error(error: impl Into<String>) -> Self {
        Self {
            kind: ParserFailureKind::ParseError,
            message: error.into(),
            stderr_excerpt: None,
            exit_code: None,
            retry_count: 0,
        }
    }

    fn timeout(timeout_ms: u64) -> Self {
        Self {
            kind: ParserFailureKind::Timeout,
            message: format!("parser worker exceeded {timeout_ms}ms timeout"),
            stderr_excerpt: None,
            exit_code: None,
            retry_count: 0,
        }
    }

    fn worker_crash(exit_code: Option<i32>, stderr_excerpt: String) -> Self {
        Self {
            kind: ParserFailureKind::WorkerCrash,
            message: "parser worker exited unsuccessfully".to_string(),
            stderr_excerpt: Some(stderr_excerpt),
            exit_code,
            retry_count: 0,
        }
    }

    fn worker_io(error: impl Into<String>) -> Self {
        Self {
            kind: ParserFailureKind::WorkerIo,
            message: error.into(),
            stderr_excerpt: None,
            exit_code: None,
            retry_count: 0,
        }
    }
}

pub struct ParserWorkerManager<'a, P, B> {
    parser: &'a P,
    event_bus: &'a B,
    config: &'a IndexerConfig,
}

impl<'a, P, B> ParserWorkerManager<'a, P, B>
where
    P: TreeSitterParser,
    B: EventBus,
{
    pub fn new(parser: &'a P, event_bus: &'a B, config: &'a IndexerConfig) -> Self {
        Self {
            parser,
            event_bus,
            config,
        }
    }

    pub fn parse(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        input: ParseInput,
        path_for_events: &str,
    ) -> Result<ParsedFile, ParserFailure> {
        let mut last_failure = None;
        let max_attempts = self.config.parser_max_retries.saturating_add(1);

        for attempt in 0..max_attempts {
            let _ = self
                .event_bus
                .publish(DomainEvent::ParserWorkerStarted(ParserWorkerStarted {
                    project_id: project_id.clone(),
                    branch_id: branch_id.clone(),
                    file_id: input.file_id.clone(),
                    path: path_for_events.to_string(),
                    attempt,
                }));
            let started = Instant::now();
            let result = match self.config.parser_isolation {
                ParserIsolation::InProcess => self.parse_in_process(input.clone()),
                ParserIsolation::SubprocessWorker => {
                    self.parse_in_subprocess(project_id, branch_id, input.clone())
                }
            };

            match result {
                Ok(parsed) => {
                    let _ = self.event_bus.publish(DomainEvent::ParserWorkerCompleted(
                        ParserWorkerCompleted {
                            project_id: project_id.clone(),
                            branch_id: branch_id.clone(),
                            file_id: input.file_id.clone(),
                            path: path_for_events.to_string(),
                            elapsed_ms: started.elapsed().as_millis() as u64,
                        },
                    ));
                    return Ok(parsed);
                }
                Err(mut failure) => {
                    failure.retry_count = attempt;
                    self.publish_failure_event(
                        project_id,
                        branch_id,
                        &input,
                        path_for_events,
                        &failure,
                        attempt,
                    );
                    let should_retry = failure.kind.retryable() && attempt + 1 < max_attempts;
                    if should_retry {
                        let _ = self
                            .event_bus
                            .publish(DomainEvent::ParseRetried(ParseRetried {
                                project_id: project_id.clone(),
                                branch_id: branch_id.clone(),
                                file_id: input.file_id.clone(),
                                path: path_for_events.to_string(),
                                attempt: attempt + 1,
                                reason: failure.message.clone(),
                            }));
                        last_failure = Some(failure);
                        continue;
                    }
                    return Err(failure);
                }
            }
        }

        Err(last_failure.unwrap_or_else(|| ParserFailure::parse_error("parser failed")))
    }

    fn parse_in_process(&self, input: ParseInput) -> Result<ParsedFile, ParserFailure> {
        self.parser
            .parse(input)
            .map_err(|error| ParserFailure::parse_error(error.message))
    }

    fn parse_in_subprocess(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        input: ParseInput,
    ) -> Result<ParsedFile, ParserFailure> {
        let worker_path = self
            .config
            .parser_worker_path
            .clone()
            .or_else(default_parser_worker_path)
            .ok_or_else(|| ParserFailure::worker_io("parser worker path could not be resolved"))?;
        let request = ParserJobRequest::from_input(project_id, branch_id, &input);
        run_parser_worker(
            &worker_path,
            request,
            Duration::from_millis(self.config.parser_timeout_ms),
        )
    }

    fn publish_failure_event(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        input: &ParseInput,
        path_for_events: &str,
        failure: &ParserFailure,
        attempt: usize,
    ) {
        match failure.kind {
            ParserFailureKind::Timeout => {
                let _ =
                    self.event_bus
                        .publish(DomainEvent::ParserWorkerTimeout(ParserWorkerTimeout {
                            project_id: project_id.clone(),
                            branch_id: branch_id.clone(),
                            file_id: input.file_id.clone(),
                            path: path_for_events.to_string(),
                            timeout_ms: self.config.parser_timeout_ms,
                            attempt,
                        }));
            }
            ParserFailureKind::WorkerCrash => {
                let _ =
                    self.event_bus
                        .publish(DomainEvent::ParserWorkerCrashed(ParserWorkerCrashed {
                            project_id: project_id.clone(),
                            branch_id: branch_id.clone(),
                            file_id: input.file_id.clone(),
                            path: path_for_events.to_string(),
                            exit_code: failure.exit_code,
                            stderr_excerpt: failure.stderr_excerpt.clone().unwrap_or_default(),
                            attempt,
                        }));
            }
            ParserFailureKind::ParseError | ParserFailureKind::WorkerIo => {}
        }
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

pub fn parse_worker_json_line(line: &str) -> ParserWorkerOutput {
    match serde_json::from_str::<ParserJobRequest>(line) {
        Ok(request) => {
            let parser = DefaultLanguagePack;
            let input = ParseInput {
                file_id: FileId::new(request.file_id),
                path: PathBuf::from(request.path),
                source: request.source,
            };
            match parser.parse(input) {
                Ok(parsed) => ParserWorkerOutput::Parsed(ParserJobResponse::from(parsed)),
                Err(error) => ParserWorkerOutput::Failed(ParserErrorDto {
                    kind: "parse_error".to_string(),
                    message: error.message,
                    stderr_excerpt: None,
                }),
            }
        }
        Err(error) => ParserWorkerOutput::Failed(ParserErrorDto {
            kind: "invalid_request".to_string(),
            message: error.to_string(),
            stderr_excerpt: None,
        }),
    }
}

fn run_parser_worker(
    worker_path: &Path,
    request: ParserJobRequest,
    timeout: Duration,
) -> Result<ParsedFile, ParserFailure> {
    let mut child = Command::new(worker_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ParserFailure::worker_io(error.to_string()))?;

    if let Some(stdin) = child.stdin.as_mut() {
        serde_json::to_writer(&mut *stdin, &request)
            .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
        stdin
            .write_all(b"\n")
            .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
    }
    drop(child.stdin.take());

    let started = Instant::now();
    while started.elapsed() < timeout {
        match child
            .try_wait()
            .map_err(|error| ParserFailure::worker_io(error.to_string()))?
        {
            Some(_status) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
                let stderr_excerpt = bounded_excerpt(&String::from_utf8_lossy(&output.stderr));
                if !output.status.success() {
                    return Err(ParserFailure::worker_crash(
                        output.status.code(),
                        stderr_excerpt,
                    ));
                }
                let stdout = String::from_utf8(output.stdout)
                    .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
                let first_line = stdout
                    .lines()
                    .next()
                    .ok_or_else(|| ParserFailure::worker_io("parser worker returned no output"))?;
                let output = serde_json::from_str::<ParserWorkerOutput>(first_line)
                    .map_err(|error| ParserFailure::worker_io(error.to_string()))?;
                return match output {
                    ParserWorkerOutput::Parsed(response) => Ok(ParsedFile::from(response)),
                    ParserWorkerOutput::Failed(error) => Err(ParserFailure {
                        kind: ParserFailureKind::ParseError,
                        message: error.message,
                        stderr_excerpt: error.stderr_excerpt,
                        exit_code: None,
                        retry_count: 0,
                    }),
                };
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(ParserFailure::timeout(timeout.as_millis() as u64))
}

fn default_parser_worker_path() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let suffix = std::env::consts::EXE_SUFFIX;
    Some(current.with_file_name(format!("b3-parser-worker{suffix}")))
}

fn bounded_excerpt(value: &str) -> String {
    const MAX_EXCERPT_BYTES: usize = 2048;
    value.chars().take(MAX_EXCERPT_BYTES).collect()
}

pub struct LocalIndexer<P, S, B> {
    parser: P,
    store: S,
    event_bus: B,
    config: IndexerConfig,
    cancellation: CancellationToken,
}

impl<P, S, B> LocalIndexer<P, S, B>
where
    P: TreeSitterParser,
    S: IndexStore,
    B: EventBus,
{
    pub fn new(parser: P, store: S, event_bus: B, config: IndexerConfig) -> Self {
        Self {
            parser,
            store,
            event_bus,
            config,
            cancellation: CancellationToken::default(),
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub fn discover(
        &self,
        root: &Path,
        project_id: &ProjectId,
    ) -> ContractResult<Vec<DiscoveredFile>> {
        let mut files = Vec::new();
        self.discover_inner(root, root, project_id, &mut files)?;
        Ok(files)
    }

    pub fn index_paths(
        &self,
        root: &Path,
        project_id: &ProjectId,
        paths: &[PathBuf],
    ) -> ContractResult<IndexSummary> {
        self.publish(DomainEvent::IndexStarted(IndexStarted {
            project_id: project_id.clone(),
            branch_id: self.config.branch_id.clone(),
            root_path: root.to_string_lossy().to_string(),
        }))?;

        let mut files_seen = 0;
        let mut files_parsed = 0;
        let mut symbols_indexed = 0;

        for path in paths {
            if self.cancellation.is_cancelled() {
                break;
            }

            if let Some(reason) = self.config.ignore.should_skip(path) {
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: None,
                    path: relative_path(root, path),
                    reason,
                }))?;
                continue;
            }

            if !path.exists() {
                self.store.remove_file(
                    project_id,
                    &self.config.branch_id,
                    &relative_path(root, path),
                )?;
                continue;
            }

            let metadata = fs::metadata(path).map_err(to_contract_error)?;
            if !metadata.is_file() {
                continue;
            }

            if metadata.len() > self.config.max_file_bytes {
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: None,
                    path: relative_path(root, path),
                    reason: "file exceeds max_file_bytes".to_string(),
                }))?;
                continue;
            }

            let relative_path = relative_path(root, path);
            let file = DiscoveredFile {
                id: FileId::new(stable_id("file", &relative_path)),
                path: path.clone(),
                relative_path,
                content_hash: hash_file(path)?,
                size_bytes: metadata.len(),
            };
            files_seen += 1;
            if let Some(parsed) = self.index_discovered(project_id, file)? {
                files_parsed += 1;
                symbols_indexed += parsed.symbols.len();
            }
        }

        self.publish(DomainEvent::IndexCompleted(IndexCompleted {
            project_id: project_id.clone(),
            branch_id: self.config.branch_id.clone(),
            files_seen,
            files_parsed,
        }))?;

        Ok(IndexSummary {
            files_seen,
            files_parsed,
            symbols_indexed,
        })
    }

    fn discover_inner(
        &self,
        root: &Path,
        current: &Path,
        project_id: &ProjectId,
        files: &mut Vec<DiscoveredFile>,
    ) -> ContractResult<()> {
        if self.cancellation.is_cancelled() {
            return Ok(());
        }

        for entry in fs::read_dir(current).map_err(to_contract_error)? {
            let entry = entry.map_err(to_contract_error)?;
            let path = entry.path();

            if let Some(reason) = self.config.ignore.should_skip(&path) {
                let relative_path = relative_path(root, &path);
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: None,
                    path: relative_path,
                    reason,
                }))?;
                continue;
            }

            let metadata = entry.metadata().map_err(to_contract_error)?;
            if metadata.is_dir() {
                self.discover_inner(root, &path, project_id, files)?;
                continue;
            }

            if !metadata.is_file() {
                continue;
            }

            let relative_path = relative_path(root, &path);
            if metadata.len() > self.config.max_file_bytes {
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: None,
                    path: relative_path,
                    reason: "file exceeds max_file_bytes".to_string(),
                }))?;
                continue;
            }

            let content_hash = hash_file(&path)?;
            let file_id = FileId::new(stable_id("file", &relative_path));
            self.publish(DomainEvent::FileDiscovered(FileDiscovered {
                project_id: project_id.clone(),
                file_id: file_id.clone(),
                path: relative_path.clone(),
            }))?;
            files.push(DiscoveredFile {
                id: file_id,
                path,
                relative_path,
                content_hash,
                size_bytes: metadata.len(),
            });
        }

        Ok(())
    }

    fn index_discovered(
        &self,
        project_id: &ProjectId,
        file: DiscoveredFile,
    ) -> ContractResult<Option<ParsedFile>> {
        if let Some(existing) = self.store.existing_file(&file.id)? {
            if existing.content_hash == file.content_hash {
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: Some(file.id),
                    path: file.relative_path,
                    reason: "unchanged content hash".to_string(),
                }))?;
                return Ok(None);
            }
        }

        let source = fs::read_to_string(&file.path).map_err(to_contract_error)?;
        let input = ParseInput {
            file_id: file.id.clone(),
            path: file.path,
            source: source.clone(),
        };
        let parser_manager = ParserWorkerManager::new(&self.parser, &self.event_bus, &self.config);
        let parsed = match parser_manager.parse(
            project_id,
            &self.config.branch_id,
            input,
            &file.relative_path,
        ) {
            Ok(parsed) => parsed,
            Err(failure) => {
                let record = ParseFailureRecord {
                    failure_id: stable_id(
                        "parse-failure",
                        &format!(
                            "{}:{}:{}",
                            project_id.as_str(),
                            self.config.branch_id.as_str(),
                            file.relative_path
                        ),
                    ),
                    project_id: project_id.clone(),
                    branch_id: self.config.branch_id.clone(),
                    file_id: file.id.clone(),
                    file_path: file.relative_path.clone(),
                    file_hash: file.content_hash,
                    language: language_from_path(&PathBuf::from(&file.relative_path)),
                    error_kind: failure.kind.as_str().to_string(),
                    error_message: failure.message.clone(),
                    stderr_excerpt: failure.stderr_excerpt.clone(),
                    failed_at_unix_ms: now_unix_ms(),
                    retry_count: failure.retry_count,
                };
                self.store.record_parse_failure(record.clone())?;
                self.publish(DomainEvent::ParseFailed(ParseFailed {
                    project_id: project_id.clone(),
                    branch_id: self.config.branch_id.clone(),
                    file_id: file.id.clone(),
                    path: file.relative_path.clone(),
                    error_kind: record.error_kind.clone(),
                    error_message: record.error_message.clone(),
                    retry_count: record.retry_count,
                }))?;
                self.publish(DomainEvent::ParserCrashed(ParserCrashed {
                    project_id: project_id.clone(),
                    file_id: Some(file.id.clone()),
                    path: file.relative_path.clone(),
                    reason: record.error_message.clone(),
                }))?;
                self.publish(DomainEvent::ParseFailureRecorded(ParseFailureRecorded {
                    project_id: project_id.clone(),
                    branch_id: self.config.branch_id.clone(),
                    file_id: file.id,
                    path: file.relative_path,
                    error_kind: record.error_kind,
                }))?;
                return Ok(None);
            }
        };
        let indexed_file = IndexedFileRecord {
            file: FileRecord {
                id: file.id.clone(),
                project_id: project_id.clone(),
                path: file.relative_path.clone(),
                content_hash: file.content_hash,
            },
            language: parsed.language.clone(),
            size_bytes: file.size_bytes,
            content: source,
            symbols: parsed
                .symbols
                .iter()
                .map(|symbol| SymbolRecord {
                    id: symbol.id.clone(),
                    file_id: symbol.file_id.clone(),
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    start_byte: symbol.start_byte,
                    end_byte: symbol.end_byte,
                    start_line: symbol.start_line,
                    start_column: symbol.start_column,
                    end_line: symbol.end_line,
                    end_column: symbol.end_column,
                    visibility: symbol.visibility.clone(),
                })
                .collect(),
            edges: parsed
                .relationships
                .iter()
                .map(|relationship| IndexedEdgeRecord {
                    id: relationship.id.clone(),
                    from_symbol: relationship.from_symbol.clone(),
                    to_symbol: relationship.to_symbol.clone(),
                    kind: relationship.kind,
                    metadata: relationship.metadata.clone(),
                })
                .collect(),
        };
        self.store
            .upsert_indexed_file(project_id, &self.config.branch_id, indexed_file)?;
        self.publish(DomainEvent::FileParsed(FileParsed {
            project_id: project_id.clone(),
            file_id: file.id,
            symbols_found: parsed.symbols.len(),
        }))?;
        Ok(Some(parsed))
    }

    fn publish(&self, event: DomainEvent) -> ContractResult<()> {
        self.event_bus.publish(event)
    }
}

impl<P, S, B> Indexer for LocalIndexer<P, S, B>
where
    P: TreeSitterParser,
    S: IndexStore,
    B: EventBus,
{
    fn index(&self, job: IndexJob) -> ContractResult<IndexSummary> {
        let root = PathBuf::from(&job.root_path);
        let project_id = job.project_id;
        let root_path = job.root_path;

        self.publish(DomainEvent::IndexStarted(IndexStarted {
            project_id: project_id.clone(),
            branch_id: self.config.branch_id.clone(),
            root_path: root_path.clone(),
        }))?;

        self.store
            .ensure_project_branch(&project_id, &self.config.branch_id, &root_path)?;

        let files = self.discover(&root, &project_id)?;
        let live_file_ids: Vec<FileId> = files.iter().map(|file| file.id.clone()).collect();
        self.store
            .cleanup_deleted_files(&project_id, &self.config.branch_id, &live_file_ids)?;
        let files_seen = files.len();
        let mut files_parsed = 0;
        let mut symbols_indexed = 0;
        let worker_pool = BoundedWorkerPool::new(self.config.max_workers);

        for range in worker_pool.batches(&files) {
            for file in &files[range] {
                if self.cancellation.is_cancelled() {
                    break;
                }

                if let Some(parsed) = self.index_discovered(&project_id, file.clone())? {
                    files_parsed += 1;
                    symbols_indexed += parsed.symbols.len();
                }
            }
        }

        self.publish(DomainEvent::IndexCompleted(IndexCompleted {
            project_id,
            branch_id: self.config.branch_id.clone(),
            files_seen,
            files_parsed,
        }))?;

        Ok(IndexSummary {
            files_seen,
            files_parsed,
            symbols_indexed,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoopTreeSitterParser;

impl TreeSitterParser for NoopTreeSitterParser {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        let _tree_sitter_anchor = std::mem::size_of::<tree_sitter::Parser>();
        Ok(ParsedFile {
            file_id: input.file_id,
            language: language_from_path(&input.path),
            symbols: Vec::new(),
            relationships: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct RustLanguagePack;

impl RustLanguagePack {
    pub fn backend_metadata() -> LanguageBackendMetadata {
        b3_core::rust_tree_sitter_backend_metadata()
    }
}

impl TreeSitterParser for RustLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        if language_from_path(&input.path).as_deref() != Some("rs") {
            return NoopTreeSitterParser.parse(input);
        }

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(to_contract_error)?;
        let tree = parser
            .parse(&input.source, None)
            .ok_or_else(|| ContractError::new("tree-sitter rust parse failed"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();
        collect_rust_symbols(root, &input, &mut symbols);
        let relationships = collect_rust_relationships(root, &input, &symbols);

        Ok(ParsedFile {
            file_id: input.file_id,
            language: Some("rust".to_string()),
            symbols,
            relationships,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct WebLanguagePack;

impl TreeSitterParser for WebLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        web::parse(input)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DefaultLanguagePack;

impl TreeSitterParser for DefaultLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        match language_from_path(&input.path).as_deref() {
            Some("rs") => RustLanguagePack.parse(input),
            Some("javascript" | "jsx" | "typescript" | "tsx") => WebLanguagePack.parse(input),
            Some("csharp" | "csproj") => csharp::parse(input),
            _ => NoopTreeSitterParser.parse(input),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeSitterPipelineParser<S, R> {
    symbol_extractor: S,
    relationship_extractor: R,
}

impl<S, R> TreeSitterPipelineParser<S, R> {
    pub fn new(symbol_extractor: S, relationship_extractor: R) -> Self {
        Self {
            symbol_extractor,
            relationship_extractor,
        }
    }
}

impl<S, R> TreeSitterParser for TreeSitterPipelineParser<S, R>
where
    S: SymbolExtractor,
    R: RelationshipExtractor,
{
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        let _tree_sitter_anchor = std::mem::size_of::<tree_sitter::Parser>();
        let language = language_from_path(&input.path);
        let symbols = self.symbol_extractor.extract_symbols(&input)?;
        let relationships = self
            .relationship_extractor
            .extract_relationships(&symbols, &input)?;

        Ok(ParsedFile {
            file_id: input.file_id,
            language,
            symbols,
            relationships,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoopSymbolExtractor;

impl SymbolExtractor for NoopSymbolExtractor {
    fn extract_symbols(&self, _input: &ParseInput) -> ContractResult<Vec<ExtractedSymbol>> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoopRelationshipExtractor;

impl RelationshipExtractor for NoopRelationshipExtractor {
    fn extract_relationships(
        &self,
        _symbols: &[ExtractedSymbol],
        _input: &ParseInput,
    ) -> ContractResult<Vec<ExtractedRelationship>> {
        Ok(Vec::new())
    }
}

fn collect_rust_symbols(node: Node<'_>, input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    if let Some((name, kind)) = rust_symbol_name_and_kind(node, &input.source) {
        let start = node.start_position();
        let end = node.end_position();
        symbols.push(ExtractedSymbol {
            id: SymbolId::new(stable_id(
                "symbol",
                &format!(
                    "{}:{kind:?}:{name}:{}:{}",
                    input.file_id.as_str(),
                    node.start_byte(),
                    node.end_byte()
                ),
            )),
            file_id: input.file_id.clone(),
            name,
            kind,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: one_based_row(start),
            start_column: start.column,
            end_line: one_based_row(end),
            end_column: end.column,
            visibility: rust_visibility(node, &input.source),
        });
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_rust_symbols(child, input, symbols);
    }
}

fn collect_rust_relationships(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    collect_call_relationships(root, input, symbols, &mut relationships);
    // Phase 4.1 policy: do not emit REFERENCES edges yet. Rust reference
    // extraction needs name resolution to avoid noisy or misleading edges, so
    // it is deferred until a later graph-analysis phase.
    relationships
}

fn collect_contains_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for child in symbols {
        let parent = symbols
            .iter()
            .filter(|candidate| candidate.id != child.id)
            .filter(|candidate| {
                candidate.start_byte <= child.start_byte && candidate.end_byte >= child.end_byte
            })
            .min_by_key(|candidate| candidate.end_byte - candidate.start_byte);

        if let Some(parent) = parent {
            relationships.push(index_edge(
                &parent.id,
                &child.id,
                EdgeKind::Contains,
                EdgeProvenance::Ast,
                10_000,
            ));
        }
    }
}

fn collect_import_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    let containers: Vec<&ExtractedSymbol> = symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                NodeKind::Module
                    | NodeKind::Function
                    | NodeKind::Method
                    | NodeKind::Struct
                    | NodeKind::Enum
                    | NodeKind::Interface
            )
        })
        .collect();

    for import in symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Package)
    {
        let owner = containers
            .iter()
            .copied()
            .filter(|candidate| {
                candidate.start_byte <= import.start_byte && candidate.end_byte >= import.end_byte
            })
            .min_by_key(|candidate| candidate.end_byte - candidate.start_byte);

        if let Some(owner) = owner {
            relationships.push(index_edge(
                &owner.id,
                &import.id,
                EdgeKind::Imports,
                EdgeProvenance::ImportAnalysis,
                9_000,
            ));
        }
    }
}

fn collect_call_relationships(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    if node.kind() == "call_expression" {
        if let Some(function) = node.child_by_field_name("function") {
            let call_name = rust_call_name(function, &input.source);
            if let Some(call_name) = call_name {
                let caller = containing_callable(node, symbols);
                let callee = symbols.iter().find(|symbol| {
                    matches!(symbol.kind, NodeKind::Function | NodeKind::Method)
                        && symbol.name == call_name
                });

                if let (Some(caller), Some(callee)) = (caller, callee) {
                    if caller.id != callee.id {
                        relationships.push(index_edge(
                            &caller.id,
                            &callee.id,
                            EdgeKind::Calls,
                            EdgeProvenance::Ast,
                            8_500,
                        ));
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_relationships(child, input, symbols, relationships);
    }
}

fn rust_symbol_name_and_kind(node: Node<'_>, source: &str) -> Option<(String, NodeKind)> {
    let kind = match node.kind() {
        "mod_item" => NodeKind::Module,
        "struct_item" => NodeKind::Struct,
        "enum_item" => NodeKind::Enum,
        "trait_item" => NodeKind::Interface,
        "impl_item" => NodeKind::Class,
        "function_item" => {
            if has_parent_kind(node, "impl_item") || has_parent_kind(node, "trait_item") {
                NodeKind::Method
            } else if has_test_attribute(node, source) {
                NodeKind::Test
            } else {
                NodeKind::Function
            }
        }
        "use_declaration" => NodeKind::Package,
        _ => return None,
    };

    let name = if node.kind() == "impl_item" {
        rust_impl_name(node, source)
    } else if node.kind() == "use_declaration" {
        Some(
            node_text(node, source)
                .trim_end_matches(';')
                .trim()
                .to_string(),
        )
    } else {
        node.child_by_field_name("name")
            .map(|name| node_text(name, source).to_string())
    }?;

    Some((name, kind))
}

fn rust_impl_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .map(|value| format!("impl {}", node_text(value, source)))
}

fn rust_visibility(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let visibility = node
        .children(&mut cursor)
        .find(|child| child.kind() == "visibility_modifier")
        .map(|child| node_text(child, source).to_string());
    visibility
}

fn rust_call_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source).to_string()),
        "field_expression" => node
            .child_by_field_name("field")
            .map(|field| node_text(field, source).to_string()),
        _ => None,
    }
}

fn containing_callable<'a>(
    node: Node<'_>,
    symbols: &'a [ExtractedSymbol],
) -> Option<&'a ExtractedSymbol> {
    symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, NodeKind::Function | NodeKind::Method))
        .filter(|symbol| {
            symbol.start_byte <= node.start_byte() && symbol.end_byte >= node.end_byte()
        })
        .min_by_key(|symbol| symbol.end_byte - symbol.start_byte)
}

fn index_edge(
    from_symbol: &SymbolId,
    to_symbol: &SymbolId,
    kind: EdgeKind,
    provenance: EdgeProvenance,
    confidence_bps: u16,
) -> ExtractedRelationship {
    ExtractedRelationship {
        id: EdgeId::new(stable_id(
            "edge",
            &format!(
                "{}:{}:{kind:?}:{}",
                from_symbol.as_str(),
                to_symbol.as_str(),
                confidence_bps
            ),
        )),
        from_symbol: from_symbol.clone(),
        to_symbol: to_symbol.clone(),
        kind,
        metadata: GraphEdgeMetadata {
            confidence: EdgeConfidence::from_basis_points(confidence_bps),
            provenance,
            created_at_unix_ms: 0,
            updated_at_unix_ms: 0,
        },
    }
}

fn has_parent_kind(node: Node<'_>, kind: &str) -> bool {
    let mut parent = node.parent();
    while let Some(value) = parent {
        if value.kind() == kind {
            return true;
        }
        parent = value.parent();
    }
    false
}

fn has_test_attribute(node: Node<'_>, source: &str) -> bool {
    let mut previous = node.prev_named_sibling();
    while let Some(sibling) = previous {
        if sibling.kind() != "attribute_item" {
            return false;
        }
        if node_text(sibling, source).contains("#[test]") {
            return true;
        }
        previous = sibling.prev_named_sibling();
    }
    false
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or_default()
}

fn one_based_row(point: Point) -> usize {
    point.row + 1
}

fn hash_file(path: &Path) -> ContractResult<String> {
    let bytes = fs::read(path).map_err(to_contract_error)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn stable_id(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{prefix}-{:x}", hasher.finalize())
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn language_from_path(path: &Path) -> Option<String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)?;
    Some(
        match extension.as_str() {
            "js" | "mjs" | "cjs" => "javascript",
            "jsx" => "jsx",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "tsx",
            "cs" => "csharp",
            "csproj" => "csproj",
            other => other,
        }
        .to_string(),
    )
}

fn node_kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Project => "project",
        NodeKind::File => "file",
        NodeKind::Module => "module",
        NodeKind::Namespace => "namespace",
        NodeKind::Class => "class",
        NodeKind::Struct => "struct",
        NodeKind::Interface => "interface",
        NodeKind::Enum => "enum",
        NodeKind::Function => "function",
        NodeKind::Method => "method",
        NodeKind::Variable => "variable",
        NodeKind::Route => "route",
        NodeKind::Endpoint => "endpoint",
        NodeKind::ConfigKey => "config_key",
        NodeKind::Test => "test",
        NodeKind::Package => "package",
        NodeKind::Decision => "decision",
        NodeKind::CodeArea => "code_area",
    }
}

fn parse_node_kind(value: &str) -> NodeKind {
    match value {
        "project" => NodeKind::Project,
        "file" => NodeKind::File,
        "module" => NodeKind::Module,
        "namespace" => NodeKind::Namespace,
        "class" => NodeKind::Class,
        "struct" => NodeKind::Struct,
        "interface" => NodeKind::Interface,
        "enum" => NodeKind::Enum,
        "function" => NodeKind::Function,
        "method" => NodeKind::Method,
        "route" => NodeKind::Route,
        "endpoint" => NodeKind::Endpoint,
        "config_key" => NodeKind::ConfigKey,
        "test" => NodeKind::Test,
        "package" => NodeKind::Package,
        "decision" => NodeKind::Decision,
        "code_area" => NodeKind::CodeArea,
        _ => NodeKind::Variable,
    }
}

fn edge_kind_name(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Contains => "contains",
        EdgeKind::Imports => "imports",
        EdgeKind::Calls => "calls",
        EdgeKind::References => "references",
        EdgeKind::Implements => "implements",
        EdgeKind::Inherits => "inherits",
        EdgeKind::DependsOn => "depends_on",
        EdgeKind::Tests => "tests",
        EdgeKind::RoutesTo => "routes_to",
        EdgeKind::ReadsConfig => "reads_config",
        EdgeKind::WritesConfig => "writes_config",
        EdgeKind::SimilarTo => "similar_to",
        EdgeKind::Touches => "touches",
        EdgeKind::Decides => "decides",
    }
}

fn parse_edge_kind(value: &str) -> EdgeKind {
    match value {
        "contains" => EdgeKind::Contains,
        "imports" => EdgeKind::Imports,
        "calls" => EdgeKind::Calls,
        "references" => EdgeKind::References,
        "implements" => EdgeKind::Implements,
        "inherits" => EdgeKind::Inherits,
        "depends_on" => EdgeKind::DependsOn,
        "tests" => EdgeKind::Tests,
        "routes_to" => EdgeKind::RoutesTo,
        "reads_config" => EdgeKind::ReadsConfig,
        "writes_config" => EdgeKind::WritesConfig,
        "similar_to" => EdgeKind::SimilarTo,
        "touches" => EdgeKind::Touches,
        "decides" => EdgeKind::Decides,
        _ => EdgeKind::References,
    }
}

fn edge_provenance_name(provenance: EdgeProvenance) -> &'static str {
    match provenance {
        EdgeProvenance::Ast => "ast",
        EdgeProvenance::ImportAnalysis => "import_analysis",
        EdgeProvenance::TextHeuristic => "text_heuristic",
        EdgeProvenance::SemanticSimilarity => "semantic_similarity",
        EdgeProvenance::UserRecorded => "user_recorded",
    }
}

fn parse_edge_provenance(value: &str) -> EdgeProvenance {
    match value {
        "ast" => EdgeProvenance::Ast,
        "import_analysis" => EdgeProvenance::ImportAnalysis,
        "semantic_similarity" => EdgeProvenance::SemanticSimilarity,
        "user_recorded" => EdgeProvenance::UserRecorded,
        _ => EdgeProvenance::TextHeuristic,
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn to_contract_error(error: impl std::fmt::Display) -> ContractError {
    ContractError::new(error.to_string())
}
