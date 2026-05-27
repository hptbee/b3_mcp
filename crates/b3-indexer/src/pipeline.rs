use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use b3_core::{
    ContractResult, DomainEvent, EventBus, FileDiscovered, FileId, FileParsed, FileRecord,
    FileSkipped, GitIndexSnapshot, IndexCompleted, IndexJob, IndexStarted, IndexStore,
    IndexSummary, IndexedEdgeRecord, IndexedFileRecord, Indexer, ParseFailed, ParseFailureRecord,
    ParseFailureRecorded, ParserCrashed, ProjectId, SymbolRecord,
};

use crate::{
    hash_file, language_from_path, now_unix_ms, relative_path, scope, stable_id, to_contract_error,
    BoundedWorkerPool, CancellationToken, DiscoveredFile, ExtractedRelationship, ExtractedSymbol,
    IndexerConfig, ParseInput, ParsedFile, ParserWorkerManager, RelationshipExtractor,
    SymbolExtractor, TreeSitterParser,
};

pub struct LocalIndexer<P, S, B> {
    pub(crate) parser: P,
    pub(crate) store: S,
    pub(crate) event_bus: B,
    pub(crate) config: IndexerConfig,
    pub(crate) cancellation: CancellationToken,
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

        let root_path = root.to_string_lossy().to_string();
        self.store
            .ensure_project_branch(project_id, &self.config.branch_id, &root_path)?;
        self.record_git_snapshot(root, project_id)?;

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

    pub fn index_scope(&self, plan: scope::ScopePlan) -> ContractResult<IndexSummary> {
        let project_id = ProjectId::new(
            plan.scope
                .project_id
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        );
        self.publish(DomainEvent::IndexStarted(IndexStarted {
            project_id: project_id.clone(),
            branch_id: self.config.branch_id.clone(),
            root_path: String::new(),
        }))?;

        let files_seen = plan.files.len();
        let mut files_parsed = 0;
        let mut symbols_indexed = 0;
        let worker_pool = BoundedWorkerPool::new(self.config.max_workers);

        for range in worker_pool.batches(&plan.files) {
            for file in &plan.files[range] {
                if self.cancellation.is_cancelled() {
                    break;
                }

                if let Some(parsed) =
                    self.index_discovered_with_force(&project_id, file.clone(), plan.scope.force)?
                {
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
        self.index_discovered_with_force(project_id, file, false)
    }

    fn index_discovered_with_force(
        &self,
        project_id: &ProjectId,
        file: DiscoveredFile,
        force: bool,
    ) -> ContractResult<Option<ParsedFile>> {
        if let Some(existing) = self.store.existing_file(&file.id)? {
            if existing.content_hash == file.content_hash && !force {
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: Some(file.id),
                    path: file.relative_path,
                    reason: "unchanged content hash".to_string(),
                }))?;
                return Ok(None);
            }
        }

        let source = match fs::read_to_string(&file.path) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::InvalidData => {
                self.publish(DomainEvent::FileSkipped(FileSkipped {
                    project_id: project_id.clone(),
                    file_id: Some(file.id),
                    path: file.relative_path,
                    reason: "file is not valid UTF-8".to_string(),
                }))?;
                return Ok(None);
            }
            Err(error) => return Err(to_contract_error(error)),
        };
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

    fn record_git_snapshot(&self, root: &Path, project_id: &ProjectId) -> ContractResult<()> {
        let status = b3_git::read_git_status(root, b3_git::GitReaderConfig::default());
        let snapshot = GitIndexSnapshot::from_status(
            project_id.clone(),
            self.config.branch_id.clone(),
            status,
            now_unix_ms(),
        );
        self.store.record_git_index_snapshot(snapshot)
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
        self.record_git_snapshot(&root, &project_id)?;

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
