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
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use b3_core::{
    BranchId, BranchMetadata, ContractError, ContractResult, DomainEvent, EdgeKind, EventBus,
    FileDiscovered, FileId, FileParsed, FileRecord, FileSkipped, IndexCompleted, IndexJob,
    IndexJobId, IndexStarted, IndexSummary, Indexer, NodeKind, ProjectId, SymbolRecord,
};
use sha2::{Digest, Sha256};

pub use b3_core::{IndexJobQueue, IndexSummary as CoreIndexSummary, Indexer as CoreIndexer};

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
    pub id: b3_core::SymbolId,
    pub file_id: FileId,
    pub name: String,
    pub kind: NodeKind,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedRelationship {
    pub from_symbol: b3_core::SymbolId,
    pub to_symbol: b3_core::SymbolId,
    pub kind: EdgeKind,
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
    ) -> ContractResult<Vec<ExtractedRelationship>>;
}

pub trait IndexStore: Send + Sync {
    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>>;
    fn upsert_file(&self, branch_id: &BranchId, file: FileRecord) -> ContractResult<()>;
    fn upsert_symbols(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        symbols: Vec<SymbolRecord>,
    ) -> ContractResult<()>;
}

pub trait FileWatcher: Send + Sync {
    fn watch(&self, root: PathBuf) -> ContractResult<()>;
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
            source,
        };
        let parsed = self.parser.parse(input)?;
        self.store.upsert_file(
            &self.config.branch_id,
            FileRecord {
                id: file.id.clone(),
                project_id: project_id.clone(),
                path: file.relative_path.clone(),
                content_hash: file.content_hash,
            },
        )?;
        self.store.upsert_symbols(
            project_id,
            &self.config.branch_id,
            parsed
                .symbols
                .iter()
                .map(|symbol| SymbolRecord {
                    id: symbol.id.clone(),
                    file_id: symbol.file_id.clone(),
                    name: symbol.name.clone(),
                })
                .collect(),
        )?;
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

        self.publish(DomainEvent::IndexStarted(IndexStarted {
            project_id: project_id.clone(),
            branch_id: self.config.branch_id.clone(),
            root_path: job.root_path,
        }))?;

        let files = self.discover(&root, &project_id)?;
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
            .extract_relationships(&symbols)?;

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
    ) -> ContractResult<Vec<ExtractedRelationship>> {
        Ok(Vec::new())
    }
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
        fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
            Ok(self
                .files
                .lock()
                .map_err(|_| ContractError::new("files lock poisoned"))?
                .get(file_id.as_str())
                .cloned())
        }

        fn upsert_file(&self, _branch_id: &BranchId, file: FileRecord) -> ContractResult<()> {
            self.files
                .lock()
                .map_err(|_| ContractError::new("files lock poisoned"))?
                .insert(file.id.as_str().to_string(), file);
            Ok(())
        }

        fn upsert_symbols(
            &self,
            _project_id: &ProjectId,
            _branch_id: &BranchId,
            symbols: Vec<SymbolRecord>,
        ) -> ContractResult<()> {
            self.symbols
                .lock()
                .map_err(|_| ContractError::new("symbols lock poisoned"))?
                .extend(symbols);
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
    fn event_bus_contract_accepts_domain_events() {
        let bus = MemoryBus::default();
        bus.publish(DomainEvent::ConfigReloaded(ConfigReloaded {
            project_id: None,
            source: "test".to_string(),
        }))
        .expect("publish");
    }
}
