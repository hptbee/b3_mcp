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

mod backend_languages;
mod data_realtime_messaging;
mod dotnet_wpf;
mod go;
mod infrastructure;
mod parser_isolation;
mod regression;
mod scoped_indexing;
mod systems_config_web;
mod web;
