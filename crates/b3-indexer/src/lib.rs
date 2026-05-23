//! Incremental indexing pipeline boundary and local implementation.
//!
//! This crate owns repository discovery, ignore filtering, file hashing,
//! incremental indexing decisions, parser worker isolation contracts, symbol and
//! relationship extraction contracts, queueing, cancellation, and index events.
//! It does not implement retrieval ranking, embedding generation, UI features,
//! or MCP request handling.

use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use b3_core::{
    BranchId, BranchMetadata, ContractError, ContractResult, DomainEvent, EdgeConfidence, EdgeId,
    EdgeKind, EdgeProvenance, EventBus, FileDiscovered, FileId, FileParsed, FileRecord,
    FileSkipped, GraphEdgeMetadata, IndexCompleted, IndexJob, IndexJobId, IndexStarted, IndexStore,
    IndexSummary, IndexedEdgeRecord, IndexedFileRecord, Indexer, NodeKind, ProjectId, SymbolId,
    SymbolRecord,
};
use notify::{
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
    EventKind as NotifyEventKind, RecursiveMode, Watcher,
};
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser, Point};

pub use b3_core::{
    IndexJobQueue, IndexStore as CoreIndexStore, IndexSummary as CoreIndexSummary,
    Indexer as CoreIndexer,
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
            parser_isolation: ParserIsolation::SubprocessWorker,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserIsolation {
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
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            ignore: IgnoreRules::default(),
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            max_workers: 1,
            branch_id: BranchId::new("default"),
            parser_isolation: ParserIsolation::SubprocessWorker,
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
        let parsed = self.parser.parse(input)?;
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
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
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
