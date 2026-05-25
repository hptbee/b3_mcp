//! Incremental indexing pipeline boundary and local implementation.
//!
//! This crate owns repository discovery, ignore filtering, file hashing,
//! incremental indexing decisions, parser worker isolation contracts, symbol and
//! relationship extraction contracts, queueing, cancellation, and index events.
//! It does not implement retrieval ranking, embedding generation, UI features,
//! or MCP request handling.

pub mod lsp;

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

#[derive(Debug, Clone, Default)]
pub struct DefaultLanguagePack;

impl TreeSitterParser for DefaultLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        match language_from_path(&input.path).as_deref() {
            Some("rs") => RustLanguagePack.parse(input),
            Some("javascript" | "jsx" | "typescript" | "tsx") => WebLanguagePack.parse(input),
            _ => NoopTreeSitterParser.parse(input),
        }
    }
}

impl TreeSitterParser for WebLanguagePack {
    fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
        let Some(language) = language_from_path(&input.path) else {
            return NoopTreeSitterParser.parse(input);
        };
        if !matches!(
            language.as_str(),
            "javascript" | "jsx" | "typescript" | "tsx"
        ) {
            return NoopTreeSitterParser.parse(input);
        }

        let mut parser = Parser::new();
        let tree_sitter_language = match language.as_str() {
            "javascript" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
            "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => unreachable!("language checked above"),
        };
        parser
            .set_language(&tree_sitter_language)
            .map_err(to_contract_error)?;
        let tree = parser
            .parse(&input.source, None)
            .ok_or_else(|| ContractError::new("tree-sitter web language parse failed"))?;

        let root = tree.root_node();
        let mut symbols = vec![module_symbol(&input)];
        collect_web_symbols(root, &input, &mut symbols);
        annotate_react_components(root, &input, &mut symbols);
        let routes = collect_node_rest_routes(root, &input, &symbols);
        symbols.extend(routes);
        let relationships = collect_web_relationships(&symbols);

        Ok(ParsedFile {
            file_id: input.file_id,
            language: Some(language),
            symbols,
            relationships,
        })
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

fn module_symbol(input: &ParseInput) -> ExtractedSymbol {
    let end_line = input.source.lines().count().max(1);
    ExtractedSymbol {
        id: SymbolId::new(stable_id(
            "symbol",
            &format!("{}:module:{}", input.file_id.as_str(), input.path.display()),
        )),
        file_id: input.file_id.clone(),
        name: input
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("module")
            .to_string(),
        kind: NodeKind::Module,
        start_byte: 0,
        end_byte: input.source.len(),
        start_line: 1,
        start_column: 0,
        end_line,
        end_column: input.source.lines().last().unwrap_or_default().len(),
        visibility: None,
    }
}

fn collect_web_symbols(node: Node<'_>, input: &ParseInput, symbols: &mut Vec<ExtractedSymbol>) {
    if let Some((name, kind, visibility)) = web_symbol_name_kind_and_visibility(node, &input.source)
    {
        symbols.push(symbol_from_node(input, node, name, kind, visibility));
    }

    if let Some(import_name) = web_import_specifier(node, &input.source) {
        symbols.push(symbol_from_node(
            input,
            node,
            import_name,
            NodeKind::Package,
            None,
        ));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_web_symbols(child, input, symbols);
    }
}

fn collect_web_relationships(symbols: &[ExtractedSymbol]) -> Vec<ExtractedRelationship> {
    let mut relationships = Vec::new();
    collect_contains_relationships(symbols, &mut relationships);
    collect_import_relationships(symbols, &mut relationships);
    collect_route_handler_relationships(symbols, &mut relationships);
    collect_component_relationships(symbols, &mut relationships);
    relationships
}

fn collect_component_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for component in symbols.iter().filter(|symbol| {
        component_metadata_value(
            symbol.visibility.as_deref().unwrap_or_default(),
            "framework",
        )
        .as_deref()
            == Some("react")
    }) {
        let metadata = component.visibility.as_deref().unwrap_or_default();
        if let Some(props_type) = component_metadata_value(metadata, "props") {
            if let Some(target) = symbols.iter().find(|symbol| {
                matches!(symbol.kind, NodeKind::Interface | NodeKind::Variable)
                    && symbol.name == props_type
                    && symbol.id != component.id
            }) {
                relationships.push(index_edge(
                    &component.id,
                    &target.id,
                    EdgeKind::References,
                    EdgeProvenance::Ast,
                    8_000,
                ));
            }
        }

        if let Some(usages) = component_metadata_value(metadata, "usages") {
            for usage in usages.split(',').filter(|usage| !usage.is_empty()) {
                let usage_name = usage.rsplit('.').next().unwrap_or(usage).trim().to_string();
                if let Some(target) = symbols.iter().find(|symbol| {
                    symbol.name == usage_name
                        && symbol.id != component.id
                        && component_metadata_value(
                            symbol.visibility.as_deref().unwrap_or_default(),
                            "framework",
                        )
                        .as_deref()
                            == Some("react")
                }) {
                    relationships.push(index_edge(
                        &component.id,
                        &target.id,
                        EdgeKind::References,
                        EdgeProvenance::Ast,
                        8_000,
                    ));
                }
            }
        }
    }
}

fn collect_route_handler_relationships(
    symbols: &[ExtractedSymbol],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    for route in symbols
        .iter()
        .filter(|symbol| symbol.kind == NodeKind::Route)
    {
        let Some(metadata) = route.visibility.as_deref() else {
            continue;
        };
        let handler = route_metadata_value(metadata, "handler")
            .or_else(|| route_metadata_value(metadata, "function"));
        let Some(handler) = handler else {
            continue;
        };
        let Some(target) = symbols.iter().find(|symbol| {
            matches!(symbol.kind, NodeKind::Function | NodeKind::Method)
                && symbol.name == handler
                && symbol.id != route.id
        }) else {
            continue;
        };
        relationships.push(index_edge(
            &route.id,
            &target.id,
            EdgeKind::References,
            EdgeProvenance::Ast,
            8_500,
        ));
    }
}

fn symbol_from_node(
    input: &ParseInput,
    node: Node<'_>,
    name: String,
    kind: NodeKind,
    visibility: Option<String>,
) -> ExtractedSymbol {
    let start = node.start_position();
    let end = node.end_position();
    ExtractedSymbol {
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
        visibility,
    }
}

fn web_symbol_name_kind_and_visibility(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let exported =
        has_parent_kind(node, "export_statement") || has_parent_kind(node, "export_clause");
    let visibility = exported.then(|| "export".to_string());
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            node.child_by_field_name("name").map(|name| {
                (
                    node_text(name, source).to_string(),
                    NodeKind::Function,
                    visibility,
                )
            })
        }
        "class_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Class,
                visibility,
            )
        }),
        "method_definition" | "method_signature" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Method,
                visibility,
            )
        }),
        "interface_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Interface,
                visibility,
            )
        }),
        "type_alias_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Variable,
                visibility,
            )
        }),
        "enum_declaration" => node.child_by_field_name("name").map(|name| {
            (
                node_text(name, source).to_string(),
                NodeKind::Enum,
                visibility,
            )
        }),
        "variable_declarator" => web_variable_symbol(node, source, visibility),
        "export_statement" => web_default_export_symbol(node, source),
        "assignment_expression" => web_module_exports_symbol(node, source),
        _ => None,
    }
}

fn web_variable_symbol(
    node: Node<'_>,
    source: &str,
    visibility: Option<String>,
) -> Option<(String, NodeKind, Option<String>)> {
    let name = node.child_by_field_name("name")?;
    let value = node.child_by_field_name("value");
    let value_kind = value.map(|value| value.kind());
    let exported = visibility.is_some();
    let should_index = exported
        || matches!(
            value_kind,
            Some(
                "arrow_function"
                    | "function"
                    | "function_expression"
                    | "class"
                    | "class_expression"
            )
        );
    if !should_index {
        return None;
    }

    let kind = if matches!(value_kind, Some("class" | "class_expression")) {
        NodeKind::Class
    } else if matches!(
        value_kind,
        Some("arrow_function" | "function" | "function_expression")
    ) {
        NodeKind::Function
    } else {
        NodeKind::Variable
    };
    Some((node_text(name, source).to_string(), kind, visibility))
}

fn web_default_export_symbol(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let text = node_text(node, source).trim_start();
    if !text.starts_with("export default") {
        return None;
    }
    if node
        .named_child(0)
        .map(|child| matches!(child.kind(), "function_declaration" | "class_declaration"))
        .unwrap_or(false)
    {
        return None;
    }
    Some((
        "default".to_string(),
        NodeKind::Variable,
        Some("export default".to_string()),
    ))
}

fn web_module_exports_symbol(
    node: Node<'_>,
    source: &str,
) -> Option<(String, NodeKind, Option<String>)> {
    let left = node.child_by_field_name("left")?;
    let text = node_text(left, source).replace(' ', "");
    if text == "module.exports"
        || text.starts_with("module.exports.")
        || text.starts_with("exports.")
    {
        Some((
            node_text(left, source).trim().to_string(),
            NodeKind::Variable,
            Some("commonjs export".to_string()),
        ))
    } else {
        None
    }
}

fn annotate_react_components(root: Node<'_>, input: &ParseInput, symbols: &mut [ExtractedSymbol]) {
    let mut candidates = Vec::new();
    collect_react_component_candidates(root, input, &mut candidates);
    for (name, node, metadata) in candidates {
        if let Some(symbol) = symbols.iter_mut().find(|symbol| {
            symbol.name == name
                && symbol.start_byte <= node.start_byte()
                && symbol.end_byte >= node.end_byte()
                && matches!(symbol.kind, NodeKind::Function | NodeKind::Class)
        }) {
            symbol.visibility = merge_visibility(
                symbol.visibility.take(),
                encode_component_metadata(&metadata),
            );
        }
    }
}

fn collect_react_component_candidates<'a>(
    node: Node<'a>,
    input: &ParseInput,
    candidates: &mut Vec<(String, Node<'a>, ComponentMetadata)>,
) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) =
                    react_function_component_metadata(&name, node, input, "FunctionDeclaration")
                {
                    candidates.push((name, node, metadata));
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) = react_class_component_metadata(&name, node, input) {
                    candidates.push((name, node, metadata));
                }
            }
        }
        "variable_declarator" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, &input.source).to_string();
                if let Some(metadata) = react_variable_component_metadata(&name, node, input) {
                    candidates.push((name, node, metadata));
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_react_component_candidates(child, input, candidates);
    }
}

fn react_function_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
    source_kind: &str,
) -> Option<ComponentMetadata> {
    if !is_pascal_case(name) || !node_contains_jsx(node, &input.source) {
        return None;
    }
    let text = node_text(node, &input.source);
    Some(ComponentMetadata {
        framework: "react".to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: "function".to_string(),
        props_type_name: props_type_from_function_text(text),
        hooks: detect_hook_names(text),
        usages: detect_jsx_component_usages(text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_500,
        source_kind: source_kind.to_string(),
    })
}

fn react_class_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
) -> Option<ComponentMetadata> {
    let text = node_text(node, &input.source);
    if !is_pascal_case(name)
        || !(text.contains("React.Component") || text.contains("Component<"))
        || !node_contains_jsx(node, &input.source)
    {
        return None;
    }
    Some(ComponentMetadata {
        framework: "react".to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: "class".to_string(),
        props_type_name: type_between_after(text, "Component<"),
        hooks: Vec::new(),
        usages: detect_jsx_component_usages(text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_000,
        source_kind: "ClassComponent".to_string(),
    })
}

fn react_variable_component_metadata(
    name: &str,
    node: Node<'_>,
    input: &ParseInput,
) -> Option<ComponentMetadata> {
    if !is_pascal_case(name) {
        return None;
    }
    let text = node_text(node, &input.source);
    let value = node.child_by_field_name("value")?;
    let value_text = node_text(value, &input.source);
    let (component_kind, source_kind, confidence) = if value.kind() == "call_expression"
        && text.contains("memo")
        && node_contains_jsx(node, &input.source)
    {
        ("memo", "ReactMemo", 8_500)
    } else if value.kind() == "call_expression"
        && text.contains("forwardRef")
        && node_contains_jsx(node, &input.source)
    {
        ("forward_ref", "ReactForwardRef", 8_500)
    } else if matches!(
        value.kind(),
        "arrow_function" | "function" | "function_expression"
    ) && node_contains_jsx(value, &input.source)
    {
        ("arrow_function", "ArrowFunction", 9_500)
    } else {
        return None;
    };

    Some(ComponentMetadata {
        framework: "react".to_string(),
        export_kind: export_kind_for_node(node, &input.source),
        component_kind: component_kind.to_string(),
        props_type_name: props_type_from_variable_text(text),
        hooks: detect_hook_names(value_text),
        usages: detect_jsx_component_usages(value_text),
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence,
        source_kind: source_kind.to_string(),
    })
}

fn export_kind_for_node(node: Node<'_>, source: &str) -> Option<String> {
    if has_parent_kind(node, "export_statement") {
        let parent_text = ancestor_text(node, source, "export_statement").unwrap_or_default();
        if parent_text.trim_start().starts_with("export default") {
            Some("default".to_string())
        } else {
            Some("named".to_string())
        }
    } else {
        None
    }
}

fn ancestor_text<'a>(node: Node<'a>, source: &'a str, kind: &str) -> Option<&'a str> {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == kind {
            return Some(node_text(current, source));
        }
        parent = current.parent();
    }
    None
}

fn node_contains_jsx(node: Node<'_>, source: &str) -> bool {
    let text = node_text(node, source);
    text.contains("</")
        || text.contains("/>")
        || text.contains("React.createElement")
        || text.contains("jsx(")
}

fn is_pascal_case(name: &str) -> bool {
    name.chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
}

fn props_type_from_function_text(text: &str) -> Option<String> {
    text.split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(params, _)| props_type_from_params(params))
}

fn props_type_from_variable_text(text: &str) -> Option<String> {
    type_between_after(text, "React.FC<")
        .or_else(|| type_between_after(text, "FC<"))
        .or_else(|| {
            text.split_once("=>")
                .and_then(|(before_arrow, _)| before_arrow.rsplit_once('('))
                .and_then(|(_, params)| props_type_from_params(params.trim_end_matches(')')))
        })
}

fn props_type_from_params(params: &str) -> Option<String> {
    params
        .split(':')
        .nth(1)
        .map(|value| {
            value
                .split(|character: char| {
                    character == ','
                        || character == '='
                        || character == ')'
                        || character.is_whitespace()
                })
                .find(|part| !part.is_empty())
                .unwrap_or_default()
                .trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn type_between_after(text: &str, marker: &str) -> Option<String> {
    let start = text.find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest.find('>')?;
    let value = rest[..end]
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    (!value.is_empty()).then_some(value)
}

fn detect_hook_names(text: &str) -> Vec<String> {
    let mut hooks = Vec::new();
    for token in text.split(|character: char| !character.is_alphanumeric() && character != '_') {
        if is_hook_name(token) && !hooks.iter().any(|hook| hook == token) {
            hooks.push(token.to_string());
        }
    }
    hooks
}

fn is_hook_name(token: &str) -> bool {
    matches!(
        token,
        "useState"
            | "useEffect"
            | "useMemo"
            | "useCallback"
            | "useRef"
            | "useContext"
            | "useReducer"
    ) || token
        .strip_prefix("use")
        .and_then(|rest| rest.chars().next())
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false)
}

fn detect_jsx_component_usages(text: &str) -> Vec<String> {
    let mut usages = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(offset) = text[index..].find('<') {
        index += offset + 1;
        if index >= bytes.len() || bytes[index] == b'/' || bytes[index] == b'>' {
            continue;
        }
        let start = index;
        while index < bytes.len()
            && ((bytes[index] as char).is_ascii_alphanumeric()
                || bytes[index] == b'_'
                || bytes[index] == b'.')
        {
            index += 1;
        }
        let tag = &text[start..index];
        if tag
            .chars()
            .next()
            .map(|character| character.is_ascii_uppercase())
            .unwrap_or(false)
            && !usages.iter().any(|usage| usage == tag)
        {
            usages.push(tag.to_string());
        }
    }
    usages
}

fn encode_component_metadata(metadata: &ComponentMetadata) -> String {
    [
        ("component.framework", Some(metadata.framework.as_str())),
        ("component.export", metadata.export_kind.as_deref()),
        ("component.kind", Some(metadata.component_kind.as_str())),
        ("component.props", metadata.props_type_name.as_deref()),
        ("component.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", value.replace(';', "%3B"))))
    .chain([
        format!("component.hooks={}", metadata.hooks.join(",")),
        format!("component.usages={}", metadata.usages.join(",")),
        format!("component.line_start={}", metadata.line_start),
        format!("component.line_end={}", metadata.line_end),
        format!("component.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

fn component_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("component.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

fn merge_visibility(existing: Option<String>, metadata: String) -> Option<String> {
    match existing {
        Some(existing) if !existing.is_empty() => Some(format!("{existing};{metadata}")),
        _ => Some(metadata),
    }
}

fn web_import_specifier(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "import_statement" => node
            .child_by_field_name("source")
            .and_then(|source_node| string_literal_value(source_node, source)),
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            if node_text(function, source) != "require" {
                return None;
            }
            let arguments = node.child_by_field_name("arguments")?;
            first_string_child(arguments, source)
        }
        _ => None,
    }
}

pub fn resolve_web_import_path(importer_path: &Path, specifier: &str) -> Option<PathBuf> {
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return None;
    }
    let base = importer_path.parent()?.join(specifier);
    web_import_candidates(&base)
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn web_import_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(base.to_path_buf());
    } else {
        for extension in ["js", "jsx", "ts", "tsx"] {
            candidates.push(base.with_extension(extension));
        }
        for extension in ["js", "jsx", "ts", "tsx"] {
            candidates.push(base.join(format!("index.{extension}")));
        }
    }
    candidates
}

fn collect_node_rest_routes(
    root: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
) -> Vec<ExtractedSymbol> {
    let mut routes = Vec::new();
    collect_call_routes(root, input, &mut routes);
    collect_nest_routes(root, input, symbols, &mut routes);
    routes
}

fn collect_call_routes(node: Node<'_>, input: &ParseInput, routes: &mut Vec<ExtractedSymbol>) {
    if node.kind() == "call_expression" {
        if let Some(metadata) = express_or_fastify_route_metadata(node, input) {
            routes.push(route_symbol(input, node, &metadata));
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_call_routes(child, input, routes);
    }
}

fn express_or_fastify_route_metadata(node: Node<'_>, input: &ParseInput) -> Option<RouteMetadata> {
    let function = node.child_by_field_name("function")?;
    let function_text = compact_member_text(node_text(function, &input.source));
    if function_text.ends_with(".route") && function_text.contains("fastify") {
        return fastify_route_object_metadata(node, input);
    }
    let route_like_receiver = function_text.starts_with("app.")
        || function_text.starts_with("router.")
        || function_text.starts_with("fastify.")
        || function_text.contains(".route(");
    if !route_like_receiver {
        return None;
    }

    let method = route_method_from_function_text(&function_text)?;
    let (framework, source_kind, path) =
        if let Some(route_path) = route_path_from_chained_route_call(&function_text) {
            ("express", "ExpressRouterCall", route_path)
        } else {
            let arguments = node.child_by_field_name("arguments")?;
            let path = first_string_child(arguments, &input.source)?;
            let framework = if function_text.contains("fastify") {
                "fastify"
            } else {
                "express"
            };
            let source_kind = if framework == "fastify" {
                "FastifyShorthandCall"
            } else if function_text.contains("router.") {
                "ExpressRouterCall"
            } else {
                "ExpressCall"
            };
            (framework, source_kind, path)
        };
    let handler_name = node
        .child_by_field_name("arguments")
        .and_then(|arguments| nth_argument_name(arguments, &input.source, 1));
    let confidence = if source_kind == "ExpressRouterCall" {
        9_000
    } else {
        9_500
    };
    Some(RouteMetadata {
        framework: framework.to_string(),
        method,
        path: normalize_route_path("", &path),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: handler_name.clone(),
        class_name: None,
        function_name: handler_name,
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence,
        source_kind: source_kind.to_string(),
    })
}

fn fastify_route_object_metadata(node: Node<'_>, input: &ParseInput) -> Option<RouteMetadata> {
    let arguments = node.child_by_field_name("arguments")?;
    let object = first_child_kind(arguments, "object")?;
    let object_text = node_text(object, &input.source);
    let method = object_property_string(object_text, "method")
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let path = object_property_string(object_text, "url")
        .or_else(|| object_property_string(object_text, "path"))?;
    let handler_name = object_property_identifier(object_text, "handler");
    Some(RouteMetadata {
        framework: "fastify".to_string(),
        method,
        path: normalize_route_path("", &path),
        file_path: input.path.to_string_lossy().replace('\\', "/"),
        symbol_id: None,
        handler_name: handler_name.clone(),
        class_name: None,
        function_name: handler_name,
        line_start: one_based_row(node.start_position()),
        line_end: one_based_row(node.end_position()),
        confidence: 9_000,
        source_kind: "FastifyRouteCall".to_string(),
    })
}

fn collect_nest_routes(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    routes: &mut Vec<ExtractedSymbol>,
) {
    if node.kind() == "class_declaration" {
        collect_nest_class_routes(node, input, symbols, routes);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nest_routes(child, input, symbols, routes);
    }
}

fn collect_nest_class_routes(
    class_node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    routes: &mut Vec<ExtractedSymbol>,
) {
    let class_text = node_text(class_node, &input.source);
    let leading_text = leading_decorator_text(class_node, &input.source);
    let Some(controller_path) = decorator_argument(&leading_text, "Controller")
        .or_else(|| decorator_argument(class_text, "Controller"))
    else {
        return;
    };
    let class_name = class_node
        .child_by_field_name("name")
        .map(|name| node_text(name, &input.source).to_string());
    let mut cursor = class_node.walk();
    for child in class_node.named_children(&mut cursor) {
        collect_nest_method_routes(
            child,
            input,
            symbols,
            &controller_path,
            class_name.as_deref(),
            routes,
        );
    }
}

fn collect_nest_method_routes(
    node: Node<'_>,
    input: &ParseInput,
    symbols: &[ExtractedSymbol],
    controller_path: &str,
    class_name: Option<&str>,
    routes: &mut Vec<ExtractedSymbol>,
) {
    if matches!(node.kind(), "method_definition" | "method_signature") {
        let text = format!(
            "{}\n{}",
            leading_decorator_text(node, &input.source),
            node_text(node, &input.source)
        );
        if let Some((method, method_path)) = nest_method_decorator(&text) {
            let function_name = node
                .child_by_field_name("name")
                .map(|name| node_text(name, &input.source).to_string());
            let symbol_id = function_name.as_ref().and_then(|name| {
                symbols
                    .iter()
                    .find(|symbol| {
                        symbol.kind == NodeKind::Method
                            && &symbol.name == name
                            && symbol.start_byte <= node.start_byte()
                            && symbol.end_byte >= node.end_byte()
                    })
                    .map(|symbol| symbol.id.clone())
            });
            let metadata = RouteMetadata {
                framework: "nestjs".to_string(),
                method,
                path: normalize_route_path(controller_path, &method_path),
                file_path: input.path.to_string_lossy().replace('\\', "/"),
                symbol_id,
                handler_name: function_name.clone(),
                class_name: class_name.map(str::to_string),
                function_name,
                line_start: one_based_row(node.start_position()),
                line_end: one_based_row(node.end_position()),
                confidence: 9_500,
                source_kind: "NestMethodDecorator".to_string(),
            };
            routes.push(route_symbol(input, node, &metadata));
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nest_method_routes(child, input, symbols, controller_path, class_name, routes);
    }
}

fn route_symbol(input: &ParseInput, node: Node<'_>, metadata: &RouteMetadata) -> ExtractedSymbol {
    let mut symbol = symbol_from_node(
        input,
        node,
        format!("{} {}", metadata.method, metadata.path),
        NodeKind::Route,
        Some(encode_route_metadata(metadata)),
    );
    symbol.id = SymbolId::new(stable_id(
        "symbol",
        &format!(
            "{}:route:{}:{}:{}:{}",
            input.file_id.as_str(),
            metadata.framework,
            metadata.method,
            metadata.path,
            node.start_byte()
        ),
    ));
    symbol
}

pub fn detect_package_json_technologies(source: &str) -> ContractResult<Vec<DetectedTechnology>> {
    let value = serde_json::from_str::<serde_json::Value>(source)
        .map_err(|error| ContractError::new(format!("invalid package.json: {error}")))?;
    let mut technologies = Vec::new();
    let dependencies = ["dependencies", "devDependencies", "peerDependencies"];
    for section in dependencies {
        let Some(object) = value.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for package_name in object.keys() {
            if let Some(technology) = package_technology(package_name, section) {
                if !technologies
                    .iter()
                    .any(|existing: &DetectedTechnology| existing.id == technology.id)
                {
                    technologies.push(technology);
                }
            }
        }
    }
    Ok(technologies)
}

fn package_technology(package_name: &str, section: &str) -> Option<DetectedTechnology> {
    let (id, name, kind, support_level, capabilities) = match package_name {
        "express" => (
            "express",
            "Express",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "@nestjs/core" | "@nestjs/common" => (
            "nestjs",
            "NestJS",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "fastify" => (
            "fastify",
            "Fastify",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
                TechnologyCapability::ExtractRoutes,
            ],
        ),
        "typescript" | "ts-node" => (
            "typescript",
            "TypeScript",
            TechnologyKind::Language,
            TechnologySupportLevel::Basic,
            vec![TechnologyCapability::DetectPackage],
        ),
        "react" | "react-dom" | "@types/react" => (
            "react",
            "React",
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::Basic,
            vec![
                TechnologyCapability::DetectPackage,
                TechnologyCapability::DetectImport,
            ],
        ),
        "next" | "vite" | "@angular/core" => (
            package_name,
            package_name,
            TechnologyKind::WebFrontend,
            TechnologySupportLevel::DetectOnly,
            vec![TechnologyCapability::DetectPackage],
        ),
        name if name.starts_with("@fastify/") => (
            "fastify",
            "Fastify",
            TechnologyKind::WebBackend,
            TechnologySupportLevel::Basic,
            vec![TechnologyCapability::DetectPackage],
        ),
        _ => return None,
    };
    Some(DetectedTechnology {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        support_level,
        capabilities,
        source: format!("package.json:{section}:{package_name}"),
    })
}

fn route_method_from_function_text(function_text: &str) -> Option<String> {
    for (suffix, method) in [
        (".get", "GET"),
        (".post", "POST"),
        (".put", "PUT"),
        (".patch", "PATCH"),
        (".delete", "DELETE"),
        (".options", "OPTIONS"),
        (".head", "HEAD"),
        (".all", "ALL"),
        (".use", "ALL"),
    ] {
        if function_text.ends_with(suffix) {
            return Some(method.to_string());
        }
    }
    None
}

fn route_path_from_chained_route_call(function_text: &str) -> Option<String> {
    let route_start = function_text.find(".route(")?;
    let after_route = &function_text[route_start + ".route(".len()..];
    let quote = after_route
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_route.split_once(quote)?.1;
    let path = after_quote.split_once(quote)?.0;
    Some(path.to_string())
}

fn nth_argument_name(arguments: Node<'_>, source: &str, index: usize) -> Option<String> {
    let mut cursor = arguments.walk();
    let value = arguments
        .named_children(&mut cursor)
        .filter(|child| child.kind() != "comment")
        .nth(index)
        .and_then(|node| match node.kind() {
            "identifier" => Some(node_text(node, source).to_string()),
            "member_expression" => Some(node_text(node, source).to_string()),
            "arrow_function" | "function" | "function_expression" => None,
            _ => Some(node_text(node, source).trim().to_string()).filter(|value| !value.is_empty()),
        });
    value
}

fn first_child_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    value
}

fn object_property_string(object_text: &str, key: &str) -> Option<String> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    let after_colon = after_key[colon_position + 1..].trim_start();
    let quote = after_colon
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_colon.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

fn object_property_identifier(object_text: &str, key: &str) -> Option<String> {
    let key_position = object_text.find(key)?;
    let after_key = &object_text[key_position + key.len()..];
    let colon_position = after_key.find(':')?;
    let after_colon = after_key[colon_position + 1..].trim_start();
    let value = after_colon
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '.'
        })
        .next()
        .unwrap_or_default();
    (!value.is_empty()).then(|| value.to_string())
}

fn decorator_argument(text: &str, decorator_name: &str) -> Option<String> {
    let needle = format!("@{decorator_name}");
    let position = text.find(&needle)?;
    let after = &text[position + needle.len()..];
    let open = after.find('(')?;
    let after_open = after[open + 1..].trim_start();
    if after_open.starts_with(')') {
        return Some(String::new());
    }
    let quote = after_open
        .chars()
        .find(|value| *value == '"' || *value == '\'')?;
    let after_quote = after_open.split_once(quote)?.1;
    Some(after_quote.split_once(quote)?.0.to_string())
}

fn leading_decorator_text(node: Node<'_>, source: &str) -> String {
    let mut parts = Vec::new();
    let mut sibling = node.prev_named_sibling();
    while let Some(value) = sibling {
        let text = node_text(value, source).trim();
        if !text.starts_with('@') {
            break;
        }
        parts.push(text.to_string());
        sibling = value.prev_named_sibling();
    }
    if parts.is_empty() {
        if let Some(parent) = node.parent() {
            let mut parent_sibling = parent.prev_named_sibling();
            while let Some(value) = parent_sibling {
                let text = node_text(value, source).trim();
                if !text.starts_with('@') {
                    break;
                }
                parts.push(text.to_string());
                parent_sibling = value.prev_named_sibling();
            }
        }
    }
    parts.reverse();
    parts.join("\n")
}

fn nest_method_decorator(text: &str) -> Option<(String, String)> {
    for (decorator, method) in [
        ("Get", "GET"),
        ("Post", "POST"),
        ("Put", "PUT"),
        ("Patch", "PATCH"),
        ("Delete", "DELETE"),
        ("Options", "OPTIONS"),
        ("Head", "HEAD"),
        ("All", "ALL"),
    ] {
        let needle = format!("@{decorator}");
        if text.contains(&needle) {
            return Some((
                method.to_string(),
                decorator_argument(text, decorator).unwrap_or_default(),
            ));
        }
    }
    None
}

fn normalize_route_path(base: &str, path: &str) -> String {
    let clean_base = base.trim_matches('/');
    let clean_path = path.trim_matches('/');
    match (clean_base.is_empty(), clean_path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{clean_path}"),
        (false, true) => format!("/{clean_base}"),
        (false, false) => format!("/{clean_base}/{clean_path}"),
    }
}

fn encode_route_metadata(metadata: &RouteMetadata) -> String {
    [
        ("route.framework", Some(metadata.framework.as_str())),
        ("route.method", Some(metadata.method.as_str())),
        ("route.path", Some(metadata.path.as_str())),
        ("route.file", Some(metadata.file_path.as_str())),
        ("route.handler", metadata.handler_name.as_deref()),
        ("route.class", metadata.class_name.as_deref()),
        ("route.function", metadata.function_name.as_deref()),
        ("route.source", Some(metadata.source_kind.as_str())),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| format!("{key}={}", value.replace(';', "%3B"))))
    .chain([
        format!("route.line_start={}", metadata.line_start),
        format!("route.line_end={}", metadata.line_end),
        format!("route.confidence={}", metadata.confidence),
    ])
    .collect::<Vec<_>>()
    .join(";")
}

fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("route.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

fn compact_member_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn first_string_child(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let value = node
        .named_children(&mut cursor)
        .find_map(|child| string_literal_value(child, source));
    value
}

fn string_literal_value(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(node.kind(), "string" | "string_fragment") {
        return None;
    }
    let text = node_text(node, source).trim();
    Some(
        text.trim_matches('"')
            .trim_matches('\'')
            .trim_matches('`')
            .to_string(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{ConfigReloaded, EventBus};
    use std::{collections::HashMap, fs};

    #[derive(Default)]
    struct MemoryStore {
        files: Mutex<HashMap<String, FileRecord>>,
        symbols: Mutex<Vec<SymbolRecord>>,
        failures: Mutex<Vec<ParseFailureRecord>>,
    }

    impl IndexStore for MemoryStore {
        fn ensure_project_branch(
            &self,
            _project_id: &ProjectId,
            _branch_id: &BranchId,
            _root_path: &str,
        ) -> ContractResult<()> {
            Ok(())
        }

        fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
            Ok(self
                .files
                .lock()
                .map_err(|_| ContractError::new("files lock poisoned"))?
                .get(file_id.as_str())
                .cloned())
        }

        fn cleanup_deleted_files(
            &self,
            _project_id: &ProjectId,
            _branch_id: &BranchId,
            _live_file_ids: &[FileId],
        ) -> ContractResult<()> {
            Ok(())
        }

        fn upsert_indexed_file(
            &self,
            _project_id: &ProjectId,
            _branch_id: &BranchId,
            file: IndexedFileRecord,
        ) -> ContractResult<()> {
            self.files
                .lock()
                .map_err(|_| ContractError::new("files lock poisoned"))?
                .insert(file.file.id.as_str().to_string(), file.file);
            self.symbols
                .lock()
                .map_err(|_| ContractError::new("symbols lock poisoned"))?
                .extend(file.symbols);
            Ok(())
        }

        fn remove_file(
            &self,
            _project_id: &ProjectId,
            _branch_id: &BranchId,
            path: &str,
        ) -> ContractResult<()> {
            self.files
                .lock()
                .map_err(|_| ContractError::new("files lock poisoned"))?
                .retain(|_, file| file.path != path);
            Ok(())
        }

        fn record_parse_failure(&self, failure: ParseFailureRecord) -> ContractResult<()> {
            self.failures
                .lock()
                .map_err(|_| ContractError::new("failures lock poisoned"))?
                .push(failure);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryBus {
        events: Mutex<Vec<DomainEvent>>,
    }

    impl EventBus for MemoryBus {
        fn publish(&self, event: DomainEvent) -> ContractResult<()> {
            self.events
                .lock()
                .map_err(|_| ContractError::new("events lock poisoned"))?
                .push(event);
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct FailingParser {
        remaining_failures: Arc<AtomicUsize>,
    }

    impl FailingParser {
        fn new(failures: usize) -> Self {
            Self {
                remaining_failures: Arc::new(AtomicUsize::new(failures)),
            }
        }
    }

    impl TreeSitterParser for FailingParser {
        fn parse(&self, input: ParseInput) -> ContractResult<ParsedFile> {
            if self.remaining_failures.load(Ordering::SeqCst) > 0 {
                self.remaining_failures.fetch_sub(1, Ordering::SeqCst);
                return Err(ContractError::new("synthetic parse failure"));
            }
            NoopTreeSitterParser.parse(input)
        }
    }

    #[test]
    fn queue_is_bounded() {
        let queue = LocalIndexJobQueue::new(1);
        let job = IndexJob {
            project_id: ProjectId::new("project"),
            root_path: ".".to_string(),
        };

        assert!(queue.enqueue(job.clone()).is_ok());
        assert!(queue.enqueue(job).is_err());
        assert!(queue.pop().expect("pop").is_some());
    }

    #[test]
    fn cancellation_token_can_cancel() {
        let token = CancellationToken::default();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn worker_pool_is_bounded() {
        let pool = BoundedWorkerPool::new(2);
        let items = [1, 2, 3, 4, 5];
        let batches = pool.batches(&items);

        assert_eq!(pool.max_workers(), 2);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn local_indexer_skips_ignored_and_unchanged_files() {
        let root = std::env::temp_dir().join(format!("b3-indexer-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".git")).expect("create ignored dir");
        fs::write(root.join("lib.rs"), "fn main() {}\n").expect("write file");
        fs::write(root.join(".git").join("HEAD"), "ignored").expect("write ignored file");

        let indexer = LocalIndexer::new(
            NoopTreeSitterParser,
            MemoryStore::default(),
            MemoryBus::default(),
            IndexerConfig {
                branch_id: BranchId::new("main"),
                ..IndexerConfig::default()
            },
        );

        let summary = indexer
            .index(IndexJob {
                project_id: ProjectId::new("project"),
                root_path: root.to_string_lossy().to_string(),
            })
            .expect("index");

        assert_eq!(summary.files_seen, 1);
        assert_eq!(summary.files_parsed, 1);

        let summary = indexer
            .index(IndexJob {
                project_id: ProjectId::new("project"),
                root_path: root.to_string_lossy().to_string(),
            })
            .expect("second index");

        assert_eq!(summary.files_seen, 1);
        assert_eq!(summary.files_parsed, 0);

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn tree_sitter_pipeline_uses_extractor_contracts() {
        let parser = TreeSitterPipelineParser::new(NoopSymbolExtractor, NoopRelationshipExtractor);
        let parsed = parser
            .parse(ParseInput {
                file_id: FileId::new("file"),
                path: PathBuf::from("lib.rs"),
                source: "fn main() {}".to_string(),
            })
            .expect("parse");

        assert_eq!(parsed.language.as_deref(), Some("rs"));
        assert!(parsed.symbols.is_empty());
        assert!(parsed.relationships.is_empty());
    }

    #[test]
    fn rust_language_pack_extracts_basic_symbols_and_calls() {
        let parsed = RustLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("file"),
                path: PathBuf::from("lib.rs"),
                source: r#"
                    use std::fmt;

                    pub struct Runner;

                    impl Runner {
                        pub fn run(&self) {
                            helper();
                        }
                    }

                    fn helper() {}

                    #[test]
                    fn helper_test() {}
                "#
                .to_string(),
            })
            .expect("parse rust");

        assert_eq!(parsed.language.as_deref(), Some("rust"));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Runner" && symbol.kind == NodeKind::Struct));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "run" && symbol.kind == NodeKind::Method));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "helper_test" && symbol.kind == NodeKind::Test));
        assert!(parsed
            .relationships
            .iter()
            .any(|edge| edge.kind == EdgeKind::Calls));
        assert!(!parsed
            .relationships
            .iter()
            .any(|edge| edge.kind == EdgeKind::References));
    }

    #[test]
    fn rust_language_pack_reports_backend_metadata() {
        let metadata = RustLanguagePack::backend_metadata();

        assert_eq!(metadata.backend_id.0, "tree-sitter-rust");
        assert_eq!(metadata.language_id.as_str(), "rust");
        assert!(metadata.available);
        assert!(metadata
            .capabilities
            .contains(&b3_core::LanguageBackendCapability::ExtractSymbols));
    }

    #[test]
    fn web_language_detection_maps_js_ts_jsx_tsx_and_csharp() {
        assert_eq!(
            language_from_path(Path::new("app.js")).as_deref(),
            Some("javascript")
        );
        assert_eq!(
            language_from_path(Path::new("app.mjs")).as_deref(),
            Some("javascript")
        );
        assert_eq!(
            language_from_path(Path::new("app.cjs")).as_deref(),
            Some("javascript")
        );
        assert_eq!(
            language_from_path(Path::new("app.ts")).as_deref(),
            Some("typescript")
        );
        assert_eq!(
            language_from_path(Path::new("app.jsx")).as_deref(),
            Some("jsx")
        );
        assert_eq!(
            language_from_path(Path::new("app.tsx")).as_deref(),
            Some("tsx")
        );
        assert_eq!(
            language_from_path(Path::new("Program.cs")).as_deref(),
            Some("csharp")
        );
    }

    #[test]
    fn web_language_pack_extracts_javascript_symbols_imports_and_commonjs_exports() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("file"),
                path: PathBuf::from("src/app.js"),
                source: r#"
                    import helper from "./helper";
                    const fs = require("fs");

                    export function run() {}

                    class Runner {
                        start() {}
                    }

                    const Widget = () => null;
                    module.exports = { run };
                "#
                .to_string(),
            })
            .expect("parse javascript");

        assert_eq!(parsed.language.as_deref(), Some("javascript"));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "run" && symbol.kind == NodeKind::Function));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Runner" && symbol.kind == NodeKind::Class));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "start" && symbol.kind == NodeKind::Method));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Widget" && symbol.kind == NodeKind::Function));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "./helper" && symbol.kind == NodeKind::Package));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "fs" && symbol.kind == NodeKind::Package));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "module.exports" && symbol.kind == NodeKind::Variable));
        assert!(parsed
            .relationships
            .iter()
            .any(|edge| edge.kind == EdgeKind::Imports));
    }

    #[test]
    fn detects_package_json_node_rest_technologies() {
        let detected = detect_package_json_technologies(
            r#"{
                "dependencies": {
                    "express": "^4.18.0",
                    "@nestjs/common": "^10.0.0",
                    "fastify": "^4.0.0",
                    "react": "^18.0.0",
                    "react-dom": "^18.0.0"
                },
                "devDependencies": {
                    "typescript": "^5.0.0",
                    "@types/react": "^18.0.0"
                }
            }"#,
        )
        .expect("detect package technologies");
        assert!(detected.iter().any(|tech| tech.id == "express"));
        assert!(detected.iter().any(|tech| tech.id == "nestjs"));
        assert!(detected.iter().any(|tech| tech.id == "fastify"));
        assert!(detected
            .iter()
            .any(|tech| tech.id == "react" && tech.kind == TechnologyKind::WebFrontend));
        assert!(detected
            .iter()
            .any(|tech| tech.id == "typescript" && tech.kind == TechnologyKind::Language));
        assert!(detect_package_json_technologies("{not-json").is_err());
    }

    #[test]
    fn detects_express_routes_and_handler_edges() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("express"),
                path: PathBuf::from("src/server.js"),
                source: r#"
                    const express = require("express");
                    const app = express();
                    const router = express.Router();

                    function listUsers(req, res) {}
                    function createUser(req, res) {}

                    app.get("/users", listUsers);
                    app.post("/users", createUser);
                    router.route("/users/:id").get(listUsers).post(createUser);
                    app.use("/users", router);
                "#
                .to_string(),
            })
            .expect("parse express");

        let routes = parsed
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == NodeKind::Route)
            .collect::<Vec<_>>();
        assert!(routes.iter().any(|route| route.name == "GET /users"));
        assert!(routes.iter().any(|route| route.name == "POST /users"));
        assert!(routes.iter().any(|route| route.name == "GET /users/:id"));
        assert!(routes.iter().any(|route| route.name == "ALL /users"));
        assert!(routes.iter().any(|route| route
            .visibility
            .as_deref()
            .unwrap_or_default()
            .contains("route.framework=express")));
        assert!(parsed
            .relationships
            .iter()
            .any(|edge| edge.kind == EdgeKind::References));
    }

    #[test]
    fn detects_nestjs_controller_routes_with_composed_paths() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("nest"),
                path: PathBuf::from("src/users.controller.ts"),
                source: r#"
                    import { Controller, Get, Post } from "@nestjs/common";

                    @Controller("users")
                    export class UsersController {
                        @Get()
                        findAll() {}

                        @Get(":id")
                        findOne() {}

                        @Post()
                        create() {}
                    }
                "#
                .to_string(),
            })
            .expect("parse nest");

        let route_names = parsed
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == NodeKind::Route)
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>();
        assert!(route_names.contains(&"GET /users"));
        assert!(route_names.contains(&"GET /users/:id"));
        assert!(route_names.contains(&"POST /users"));
        assert!(parsed.symbols.iter().any(|symbol| {
            symbol.kind == NodeKind::Route
                && symbol
                    .visibility
                    .as_deref()
                    .unwrap_or_default()
                    .contains("route.framework=nestjs")
        }));
    }

    #[test]
    fn detects_fastify_shorthand_and_route_object() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("fastify"),
                path: PathBuf::from("src/server.ts"),
                source: r#"
                    import fastify from "fastify";
                    const app = fastify();
                    function listUsers() {}
                    app.get("/users", listUsers);
                    fastify.route({
                        method: "POST",
                        url: "/users",
                        handler: listUsers
                    });
                "#
                .to_string(),
            })
            .expect("parse fastify");

        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "GET /users"));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.kind == NodeKind::Route && symbol.name == "POST /users"));
    }

    #[test]
    fn detects_react_tsx_components_props_hooks_and_usages() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("react"),
                path: PathBuf::from("src/ProductCard.tsx"),
                source: r#"
                    import React, { useEffect, useState, memo } from "react";

                    interface ProductCardProps {
                        name: string;
                    }

                    type BadgeProps = {
                        label: string;
                    };

                    export function ProductCard(props: ProductCardProps) {
                        const [open, setOpen] = useState(false);
                        useEffect(() => {}, []);
                        return <Badge label={props.name} />;
                    }

                    const Badge = ({ label }: BadgeProps) => <span>{label}</span>;
                    export default memo(ProductCard);

                    function helper() {
                        return "not jsx";
                    }
                "#
                .to_string(),
            })
            .expect("parse react tsx");

        let product = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.name == "ProductCard")
            .expect("ProductCard symbol");
        let product_metadata = product.visibility.as_deref().unwrap_or_default();
        assert_eq!(
            component_metadata_value(product_metadata, "framework").as_deref(),
            Some("react")
        );
        assert_eq!(
            component_metadata_value(product_metadata, "props").as_deref(),
            Some("ProductCardProps")
        );
        assert!(component_metadata_value(product_metadata, "hooks")
            .unwrap_or_default()
            .contains("useState"));
        assert!(component_metadata_value(product_metadata, "usages")
            .unwrap_or_default()
            .contains("Badge"));

        let badge = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.name == "Badge")
            .expect("Badge symbol");
        assert_eq!(
            component_metadata_value(badge.visibility.as_deref().unwrap_or_default(), "props")
                .as_deref(),
            Some("BadgeProps")
        );
        assert!(!parsed.symbols.iter().any(|symbol| {
            symbol.name == "helper"
                && component_metadata_value(
                    symbol.visibility.as_deref().unwrap_or_default(),
                    "framework",
                )
                .is_some()
        }));
        assert!(parsed
            .relationships
            .iter()
            .any(|edge| edge.kind == EdgeKind::References));
    }

    #[test]
    fn detects_react_jsx_components_and_class_components() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("jsx"),
                path: PathBuf::from("src/App.jsx"),
                source: r#"
                    import * as React from "react";

                    class ProductCard extends React.Component {
                        render() {
                            return <section />;
                        }
                    }

                    export const App = () => <ProductCard />;
                    const value = () => "plain";
                "#
                .to_string(),
            })
            .expect("parse react jsx");

        let app = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.name == "App")
            .expect("App symbol");
        assert_eq!(
            component_metadata_value(app.visibility.as_deref().unwrap_or_default(), "framework")
                .as_deref(),
            Some("react")
        );
        let product = parsed
            .symbols
            .iter()
            .find(|symbol| symbol.name == "ProductCard")
            .expect("ProductCard symbol");
        assert_eq!(
            component_metadata_value(product.visibility.as_deref().unwrap_or_default(), "kind")
                .as_deref(),
            Some("class")
        );
        assert!(!parsed.symbols.iter().any(|symbol| {
            symbol.name == "value"
                && component_metadata_value(
                    symbol.visibility.as_deref().unwrap_or_default(),
                    "framework",
                )
                .is_some()
        }));
    }

    #[test]
    fn web_language_pack_extracts_typescript_symbols_and_exports() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("file"),
                path: PathBuf::from("src/app.ts"),
                source: r#"
                    import { helper } from "./helper";

                    export interface User {
                        id: string;
                    }

                    export type UserId = string;
                    export enum Role { Admin }
                    export const makeUser = (): User => ({ id: "1" });

                    export class Service {
                        load(): User { return makeUser(); }
                    }
                "#
                .to_string(),
            })
            .expect("parse typescript");

        assert_eq!(parsed.language.as_deref(), Some("typescript"));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "User" && symbol.kind == NodeKind::Interface));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "UserId" && symbol.kind == NodeKind::Variable));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Role" && symbol.kind == NodeKind::Enum));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "makeUser" && symbol.kind == NodeKind::Function));
        assert!(parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Service" && symbol.kind == NodeKind::Class));
    }

    #[test]
    fn web_language_pack_extracts_jsx_and_tsx_component_like_symbols() {
        let jsx = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("jsx-file"),
                path: PathBuf::from("src/App.jsx"),
                source: r#"
                    import React from "react";
                    export default function App() {
                        return <main>Hello</main>;
                    }
                "#
                .to_string(),
            })
            .expect("parse jsx");
        assert_eq!(jsx.language.as_deref(), Some("jsx"));
        assert!(jsx
            .symbols
            .iter()
            .any(|symbol| symbol.name == "App" && symbol.kind == NodeKind::Function));

        let tsx = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("tsx-file"),
                path: PathBuf::from("src/Button.tsx"),
                source: r#"
                    import { Icon } from "./Icon";
                    export const Button = () => <button><Icon /></button>;
                "#
                .to_string(),
            })
            .expect("parse tsx");
        assert_eq!(tsx.language.as_deref(), Some("tsx"));
        assert!(tsx
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Button" && symbol.kind == NodeKind::Function));
    }

    #[test]
    fn web_import_resolution_handles_relative_extensions_and_index_files() {
        let root =
            std::env::temp_dir().join(format!("b3-web-import-resolution-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src").join("feature")).expect("create dirs");
        fs::write(
            root.join("src").join("helper.ts"),
            "export const helper = 1;",
        )
        .expect("write helper");
        fs::write(
            root.join("src").join("feature").join("index.tsx"),
            "export const Feature = () => null;",
        )
        .expect("write index");

        let importer = root.join("src").join("app.tsx");
        let helper_path = root.join("src").join("helper.ts");
        let feature_path = root.join("src").join("feature").join("index.tsx");
        assert_eq!(
            resolve_web_import_path(&importer, "./helper").as_deref(),
            Some(helper_path.as_path())
        );
        assert_eq!(
            resolve_web_import_path(&importer, "./feature").as_deref(),
            Some(feature_path.as_path())
        );
        assert!(resolve_web_import_path(&importer, "react").is_none());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn default_language_pack_keeps_rust_and_fallback_behavior() {
        let rust = DefaultLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("rust"),
                path: PathBuf::from("src/lib.rs"),
                source: "fn run() {}".to_string(),
            })
            .expect("parse rust");
        assert_eq!(rust.language.as_deref(), Some("rust"));
        assert!(rust.symbols.iter().any(|symbol| symbol.name == "run"));

        let unsupported = DefaultLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("txt"),
                path: PathBuf::from("README.txt"),
                source: "hello".to_string(),
            })
            .expect("parse unsupported");
        assert_eq!(unsupported.language.as_deref(), Some("txt"));
        assert!(unsupported.symbols.is_empty());
    }

    #[test]
    fn local_indexer_indexes_small_js_and_tsx_project() {
        let root = std::env::temp_dir().join(format!("b3-web-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("src").join("helper.js"),
            "export function helper() { return 1; }",
        )
        .expect("write js");
        fs::write(
            root.join("src").join("App.tsx"),
            r#"import { helper } from "./helper";
               export const App = () => <main>{helper()}</main>;"#,
        )
        .expect("write tsx");

        let store = MemoryStore::default();
        let indexer = LocalIndexer::new(
            DefaultLanguagePack,
            store,
            MemoryBus::default(),
            IndexerConfig {
                branch_id: BranchId::new("main"),
                ..IndexerConfig::default()
            },
        );

        let summary = indexer
            .index(IndexJob {
                project_id: ProjectId::new("project"),
                root_path: root.to_string_lossy().to_string(),
            })
            .expect("index web project");
        assert_eq!(summary.files_seen, 2);
        assert_eq!(summary.files_parsed, 2);
        assert!(summary.symbols_indexed > 0);

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn web_language_pack_handles_invalid_syntax_without_panic() {
        let parsed = WebLanguagePack
            .parse(ParseInput {
                file_id: FileId::new("bad"),
                path: PathBuf::from("src/bad.ts"),
                source: "export function broken(".to_string(),
            })
            .expect("parse invalid syntax as partial tree");
        assert_eq!(parsed.language.as_deref(), Some("typescript"));
    }

    #[test]
    fn event_bus_contract_accepts_domain_events() {
        let bus = MemoryBus::default();
        bus.publish(DomainEvent::ConfigReloaded(ConfigReloaded {
            project_id: None,
            source: "test".to_string(),
        }))
        .expect("publish");
    }

    #[test]
    fn debounce_coalesces_same_path() {
        let mut debouncer = WatchDebouncer::new(Duration::from_millis(500), 10);
        let path = PathBuf::from("src/lib.rs");
        assert!(debouncer
            .push(WatchEvent {
                kind: WatchEventKind::Changed,
                path: path.clone(),
                new_path: None,
            })
            .is_none());
        assert!(debouncer
            .push(WatchEvent {
                kind: WatchEventKind::Changed,
                path,
                new_path: None,
            })
            .is_none());
        let batch = debouncer.flush().expect("batch");
        assert_eq!(batch.events.len(), 1);
    }

    #[test]
    fn watch_config_defaults_are_disabled_and_bounded() {
        let config = WatchConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.debounce_ms, 500);
        assert_eq!(config.max_batch_size, 100);
    }

    #[test]
    fn parser_isolation_config_defaults_are_bounded() {
        let config = IndexerConfig::default();
        assert_eq!(config.parser_isolation, ParserIsolation::InProcess);
        assert_eq!(config.parser_timeout_ms, 10_000);
        assert_eq!(config.parser_max_retries, 1);
        assert!(config.parser_worker_path.is_none());
    }

    #[test]
    fn subprocess_worker_request_response_serializes() {
        let request = ParserJobRequest {
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            file_id: "file".to_string(),
            path: "src/lib.rs".to_string(),
            source: "fn run() {}".to_string(),
        };
        let json = serde_json::to_string(&request).expect("request json");
        let output = parse_worker_json_line(&json);
        let json = serde_json::to_string(&output).expect("response json");
        assert!(json.contains("parsed"));
        assert!(json.contains("run"));
    }

    #[test]
    fn parser_failure_is_recorded_and_events_are_emitted() {
        let root = std::env::temp_dir().join(format!(
            "b3-indexer-parser-failure-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("lib.rs"), "fn main() {}\n").expect("write");

        let store = MemoryStore::default();
        let bus = MemoryBus::default();
        let indexer = LocalIndexer::new(
            FailingParser::new(2),
            store,
            bus,
            IndexerConfig {
                branch_id: BranchId::new("main"),
                parser_max_retries: 1,
                ..IndexerConfig::default()
            },
        );

        let summary = indexer
            .index(IndexJob {
                project_id: ProjectId::new("project"),
                root_path: root.to_string_lossy().to_string(),
            })
            .expect("index");

        assert_eq!(summary.files_seen, 1);
        assert_eq!(summary.files_parsed, 0);
        assert_eq!(
            indexer
                .store
                .failures
                .lock()
                .expect("failures")
                .first()
                .expect("failure")
                .retry_count,
            0
        );
        let events = indexer.event_bus.events.lock().expect("events");
        assert!(events
            .iter()
            .any(|event| matches!(event, DomainEvent::ParseFailed(_))));
        assert!(events
            .iter()
            .any(|event| matches!(event, DomainEvent::ParseFailureRecorded(_))));

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn retry_policy_retries_only_worker_failures() {
        assert!(ParserFailureKind::WorkerCrash.retryable());
        assert!(ParserFailureKind::Timeout.retryable());
        assert!(ParserFailureKind::WorkerIo.retryable());
        assert!(!ParserFailureKind::ParseError.retryable());
    }

    #[test]
    fn parser_timeout_and_crash_failures_are_structured() {
        let timeout = ParserFailure::timeout(10);
        assert_eq!(timeout.kind, ParserFailureKind::Timeout);
        assert!(timeout.message.contains("10ms"));

        let crash = ParserFailure::worker_crash(Some(1), "boom".to_string());
        assert_eq!(crash.kind, ParserFailureKind::WorkerCrash);
        assert_eq!(crash.exit_code, Some(1));
        assert_eq!(crash.stderr_excerpt.as_deref(), Some("boom"));
    }

    #[test]
    fn indexing_continues_after_one_parser_failure() {
        let root = std::env::temp_dir().join(format!(
            "b3-indexer-parser-continue-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("a.rs"), "fn a() {}\n").expect("write a");
        fs::write(root.join("b.rs"), "fn b() {}\n").expect("write b");

        let indexer = LocalIndexer::new(
            FailingParser::new(1),
            MemoryStore::default(),
            MemoryBus::default(),
            IndexerConfig {
                branch_id: BranchId::new("main"),
                parser_max_retries: 0,
                ..IndexerConfig::default()
            },
        );

        let summary = indexer
            .index(IndexJob {
                project_id: ProjectId::new("project"),
                root_path: root.to_string_lossy().to_string(),
            })
            .expect("index");

        assert_eq!(summary.files_seen, 2);
        assert_eq!(summary.files_parsed, 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn ignore_rules_skip_generated_and_local_data() {
        let ignore = IgnoreRules::default();
        assert!(ignore.should_skip(Path::new("target/debug/app")).is_some());
        assert!(ignore
            .should_skip(Path::new("node_modules/pkg/index.js"))
            .is_some());
        assert!(ignore.should_skip(Path::new(".b3/b3.db")).is_some());
        assert!(ignore.should_skip(Path::new("src/lib.rs")).is_none());
    }

    #[test]
    fn event_classification_handles_create_modify_delete() {
        let create = notify::Event::new(NotifyEventKind::Create(CreateKind::File))
            .add_path(PathBuf::from("src/lib.rs"));
        assert_eq!(
            classify_notify_event(&create)[0].kind,
            WatchEventKind::Created
        );

        let modify = notify::Event::new(NotifyEventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Content,
        )))
        .add_path(PathBuf::from("src/lib.rs"));
        assert_eq!(
            classify_notify_event(&modify)[0].kind,
            WatchEventKind::Changed
        );

        let delete = notify::Event::new(NotifyEventKind::Remove(RemoveKind::File))
            .add_path(PathBuf::from("src/lib.rs"));
        assert_eq!(
            classify_notify_event(&delete)[0].kind,
            WatchEventKind::Deleted
        );
    }

    #[test]
    fn deleted_file_cleanup_path_removes_record() {
        let root =
            std::env::temp_dir().join(format!("b3-indexer-delete-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let path = root.join("lib.rs");
        fs::write(&path, "fn main() {}\n").expect("write");

        let store = MemoryStore::default();
        let indexer = LocalIndexer::new(
            NoopTreeSitterParser,
            store,
            MemoryBus::default(),
            IndexerConfig {
                branch_id: BranchId::new("main"),
                ..IndexerConfig::default()
            },
        );
        let project_id = ProjectId::new("project");
        indexer
            .index_paths(&root, &project_id, std::slice::from_ref(&path))
            .expect("index path");
        fs::remove_file(&path).expect("delete");
        let summary = indexer
            .index_paths(&root, &project_id, std::slice::from_ref(&path))
            .expect("cleanup");
        assert_eq!(summary.files_parsed, 0);
        fs::remove_dir_all(root).expect("cleanup dir");
    }

    #[test]
    fn unchanged_file_skip_works_for_changed_path_indexing() {
        let root =
            std::env::temp_dir().join(format!("b3-indexer-unchanged-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        let path = root.join("lib.rs");
        fs::write(&path, "fn main() {}\n").expect("write");

        let indexer = LocalIndexer::new(
            NoopTreeSitterParser,
            MemoryStore::default(),
            MemoryBus::default(),
            IndexerConfig {
                branch_id: BranchId::new("main"),
                ..IndexerConfig::default()
            },
        );
        let project_id = ProjectId::new("project");
        assert_eq!(
            indexer
                .index_paths(&root, &project_id, std::slice::from_ref(&path))
                .expect("first")
                .files_parsed,
            1
        );
        assert_eq!(
            indexer
                .index_paths(&root, &project_id, std::slice::from_ref(&path))
                .expect("second")
                .files_parsed,
            0
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
