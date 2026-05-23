//! Storage boundary.
//!
//! This crate hosts the offline-first SQLite/libSQL storage foundation: WAL
//! setup, migrations, graph tables, FTS5 tables, token savings storage, and
//! repository implementations. It does not index files, generate embeddings,
//! rank retrieval results, or serve UI/MCP requests.

use std::path::Path;

use b3_core::{
    BranchId, BranchMetadata, ContractError, ContractResult, EdgeConfidence, EdgeId, EdgeKind,
    EdgeProvenance, FileId, FileRecord, FileRepository, FtsSearchHit, GraphDirection, GraphEdge,
    GraphEdgeMetadata, GraphNeighbor, GraphNode, GraphRepository, IndexStore, IndexedFileRecord,
    NodeId, NodeKind, ProjectId, QueryFile, QueryRepository, QueryScope, QuerySymbol,
    StorageProvider, SymbolId, SymbolRecord, SymbolRepository, TokenSavingsRecord,
    TokenSavingsRepository,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

pub use b3_core::{
    FileRepository as FileRepositoryContract, GraphRepository as GraphRepositoryContract,
    QueryRepository as QueryRepositoryContract, StorageProvider as StorageProviderContract,
    SymbolRepository as SymbolRepositoryContract,
    TokenSavingsRepository as TokenSavingsRepositoryContract,
};

const MIGRATION_001: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS branches (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    commit_hash TEXT,
    worktree_dirty INTEGER NOT NULL DEFAULT 0,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    language TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS symbols (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'unknown',
    documentation TEXT NOT NULL DEFAULT '',
    snippet TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    start_byte INTEGER NOT NULL DEFAULT 0,
    end_byte INTEGER NOT NULL DEFAULT 0,
    start_line INTEGER NOT NULL DEFAULT 0,
    start_column INTEGER NOT NULL DEFAULT 0,
    end_line INTEGER NOT NULL DEFAULT 0,
    end_column INTEGER NOT NULL DEFAULT 0,
    visibility TEXT,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE,
    FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    symbol_id TEXT,
    file_id TEXT,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE,
    FOREIGN KEY(symbol_id) REFERENCES symbols(id) ON DELETE SET NULL,
    FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS edges (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    from_node_id TEXT NOT NULL,
    to_node_id TEXT NOT NULL,
    confidence_bps INTEGER NOT NULL,
    provenance TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE,
    FOREIGN KEY(from_node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(to_node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    source_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    vector BLOB,
    content_hash TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT,
    started_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS decisions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    session_id TEXT,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS code_areas (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS savings_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_call_id TEXT,
    estimated_tokens_saved INTEGER NOT NULL,
    returned_tokens INTEGER NOT NULL,
    avoided_file_reads INTEGER NOT NULL,
    avoided_search_calls INTEGER NOT NULL,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS tool_logs (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    tool_name TEXT NOT NULL,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    created_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE SET NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS file_content_fts USING fts5(
    file_id UNINDEXED,
    path,
    content
);

CREATE VIRTUAL TABLE IF NOT EXISTS symbol_fts USING fts5(
    symbol_id UNINDEXED,
    name,
    documentation,
    snippet
);

CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_node_id);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_node_id);
CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(edge_type);
CREATE INDEX IF NOT EXISTS idx_nodes_kind ON nodes(kind);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_files_branch_id ON files(branch_id);
CREATE INDEX IF NOT EXISTS idx_symbols_branch_id ON symbols(branch_id);
CREATE INDEX IF NOT EXISTS idx_nodes_branch_id ON nodes(branch_id);
CREATE INDEX IF NOT EXISTS idx_edges_branch_id ON edges(branch_id);
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageConfig {
    pub project_id: ProjectId,
    pub database_path: String,
    pub fts5_path: String,
    pub wal_enabled: bool,
    pub branch: BranchMetadata,
}

impl StorageConfig {
    pub fn new(
        project_id: ProjectId,
        database_path: impl Into<String>,
        branch: BranchMetadata,
    ) -> Self {
        let database_path = database_path.into();

        Self {
            project_id,
            fts5_path: database_path.clone(),
            database_path,
            wal_enabled: true,
            branch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVectorSearchConfig {
    pub qdrant_url: String,
    pub collection: String,
    pub hosted_qdrant_enabled: bool,
}

impl LocalVectorSearchConfig {
    pub fn local_qdrant(collection: impl Into<String>) -> Self {
        Self {
            qdrant_url: "http://127.0.0.1:6334".to_string(),
            collection: collection.into(),
            hosted_qdrant_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSavingsLedgerPath {
    pub database_path: String,
    pub table_name: String,
}

impl TokenSavingsLedgerPath {
    pub fn new(database_path: impl Into<String>) -> Self {
        Self {
            database_path: database_path.into(),
            table_name: "savings_ledger".to_string(),
        }
    }
}

pub struct SqliteStorage {
    connection: Connection,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> ContractResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(to_contract_error)?;
            }
        }

        let connection = Connection::open(path).map_err(to_contract_error)?;
        let mut storage = Self { connection };
        storage.configure_connection()?;
        storage.migrate()?;
        Ok(storage)
    }

    pub fn open_in_memory() -> ContractResult<Self> {
        let connection = Connection::open_in_memory().map_err(to_contract_error)?;
        let mut storage = Self { connection };
        storage.configure_connection()?;
        storage.migrate()?;
        Ok(storage)
    }

    pub fn migrate(&mut self) -> ContractResult<()> {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at_unix_ms INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )
            .map_err(to_contract_error)?;

        let applied = self
            .connection
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                [1_i64],
                |_| Ok(()),
            )
            .optional()
            .map_err(to_contract_error)?
            .is_some();

        if !applied {
            let transaction = self.connection.transaction().map_err(to_contract_error)?;
            transaction
                .execute_batch(MIGRATION_001)
                .map_err(to_contract_error)?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![1_i64, "initial_storage_schema"],
                )
                .map_err(to_contract_error)?;
            transaction.commit().map_err(to_contract_error)?;
        }

        self.ensure_phase4_columns()?;

        Ok(())
    }

    pub fn upsert_project(
        &self,
        project_id: &ProjectId,
        name: &str,
        root_path: &str,
    ) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO projects (id, name, root_path)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    root_path = excluded.root_path",
                params![project_id.as_str(), name, root_path],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn upsert_branch(
        &self,
        branch_id: &BranchId,
        project_id: &ProjectId,
        branch: &BranchMetadata,
    ) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO branches (id, project_id, name, commit_hash, worktree_dirty)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    commit_hash = excluded.commit_hash,
                    worktree_dirty = excluded.worktree_dirty",
                params![
                    branch_id.as_str(),
                    project_id.as_str(),
                    branch.branch_name.as_str(),
                    branch.commit_hash.as_deref(),
                    bool_to_i64(branch.worktree_dirty)
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn upsert_file(&self, record: &FileRecord, branch_id: &BranchId) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO files (id, project_id, branch_id, path, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    branch_id = excluded.branch_id,
                    path = excluded.path,
                    content_hash = excluded.content_hash",
                params![
                    record.id.as_str(),
                    record.project_id.as_str(),
                    branch_id.as_str(),
                    record.path.as_str(),
                    record.content_hash.as_str()
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn upsert_symbol(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        record: &SymbolRecord,
    ) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO symbols (
                    id, project_id, branch_id, file_id, name, kind, start_byte, end_byte,
                    start_line, start_column, end_line, end_column, visibility
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                    project_id = excluded.project_id,
                    branch_id = excluded.branch_id,
                    file_id = excluded.file_id,
                    name = excluded.name,
                    kind = excluded.kind,
                    start_byte = excluded.start_byte,
                    end_byte = excluded.end_byte,
                    start_line = excluded.start_line,
                    start_column = excluded.start_column,
                    end_line = excluded.end_line,
                    end_column = excluded.end_column,
                    visibility = excluded.visibility",
                params![
                    record.id.as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    record.file_id.as_str(),
                    record.name.as_str(),
                    node_kind(record.kind),
                    record.start_byte as i64,
                    record.end_byte as i64,
                    record.start_line as i64,
                    record.start_column as i64,
                    record.end_line as i64,
                    record.end_column as i64,
                    record.visibility.as_deref()
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn upsert_node(
        &self,
        node: &GraphNode,
        branch_id: &BranchId,
        kind: NodeKind,
    ) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO nodes (id, project_id, branch_id, kind, label)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    branch_id = excluded.branch_id,
                    kind = excluded.kind,
                    label = excluded.label",
                params![
                    node.id.as_str(),
                    node.project_id.as_str(),
                    branch_id.as_str(),
                    node_kind(kind),
                    node.label.as_str()
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn upsert_edge(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        edge: &GraphEdge,
        edge_type: EdgeKind,
    ) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO edges (
                    id, project_id, branch_id, edge_type, from_node_id, to_node_id,
                    confidence_bps, provenance, created_at_unix_ms, updated_at_unix_ms
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    edge_type = excluded.edge_type,
                    from_node_id = excluded.from_node_id,
                    to_node_id = excluded.to_node_id,
                    confidence_bps = excluded.confidence_bps,
                    provenance = excluded.provenance,
                    updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![
                    edge.id.as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    edge_kind(edge_type),
                    edge.from.as_str(),
                    edge.to.as_str(),
                    i64::from(edge.metadata.confidence.basis_points()),
                    edge_provenance(edge.metadata.provenance),
                    edge.metadata.created_at_unix_ms as i64,
                    edge.metadata.updated_at_unix_ms as i64
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn table_exists(&self, table_name: &str) -> ContractResult<bool> {
        let exists = self
            .connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name = ?1 LIMIT 1",
                [table_name],
                |_| Ok(()),
            )
            .optional()
            .map_err(to_contract_error)?
            .is_some();
        Ok(exists)
    }

    pub fn index_exists(&self, index_name: &str) -> ContractResult<bool> {
        let exists = self
            .connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1 LIMIT 1",
                [index_name],
                |_| Ok(()),
            )
            .optional()
            .map_err(to_contract_error)?
            .is_some();
        Ok(exists)
    }

    pub fn migration_applied(&self, version: i64) -> ContractResult<bool> {
        let applied = self
            .connection
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1 LIMIT 1",
                [version],
                |_| Ok(()),
            )
            .optional()
            .map_err(to_contract_error)?
            .is_some();
        Ok(applied)
    }

    pub fn pragma_value(&self, name: &str) -> ContractResult<String> {
        let sql = format!("PRAGMA {name}");
        self.connection
            .query_row(&sql, [], |row| row.get::<_, String>(0))
            .map_err(to_contract_error)
    }

    pub fn pragma_i64(&self, name: &str) -> ContractResult<i64> {
        let sql = format!("PRAGMA {name}");
        self.connection
            .query_row(&sql, [], |row| row.get::<_, i64>(0))
            .map_err(to_contract_error)
    }

    pub fn count_rows(&self, table_name: &str) -> ContractResult<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table_name}");
        self.connection
            .query_row(&sql, [], |row| row.get::<_, i64>(0))
            .map_err(to_contract_error)
    }

    pub fn count_edges_by_kind(&self, kind: EdgeKind) -> ContractResult<i64> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM edges WHERE edge_type = ?1",
                [edge_kind(kind)],
                |row| row.get::<_, i64>(0),
            )
            .map_err(to_contract_error)
    }

    fn ensure_phase4_columns(&self) -> ContractResult<()> {
        for (column, definition) in [
            ("start_byte", "INTEGER NOT NULL DEFAULT 0"),
            ("end_byte", "INTEGER NOT NULL DEFAULT 0"),
            ("start_column", "INTEGER NOT NULL DEFAULT 0"),
            ("end_column", "INTEGER NOT NULL DEFAULT 0"),
            ("visibility", "TEXT"),
        ] {
            if !self.column_exists("symbols", column)? {
                self.connection
                    .execute(
                        &format!("ALTER TABLE symbols ADD COLUMN {column} {definition}"),
                        [],
                    )
                    .map_err(to_contract_error)?;
            }
        }

        Ok(())
    }

    fn column_exists(&self, table_name: &str, column_name: &str) -> ContractResult<bool> {
        let sql = format!("PRAGMA table_info({table_name})");
        let mut statement = self.connection.prepare(&sql).map_err(to_contract_error)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(to_contract_error)?;

        for row in rows {
            if row.map_err(to_contract_error)? == column_name {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn configure_connection(&self) -> ContractResult<()> {
        self.connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(to_contract_error)?;
        self.connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(to_contract_error)?;
        self.connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(to_contract_error)?;
        Ok(())
    }
}

impl StorageProvider for SqliteStorage {
    fn name(&self) -> &str {
        "sqlite"
    }

    fn is_local_only(&self) -> bool {
        true
    }
}

impl IndexStore for SqliteStorage {
    fn ensure_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        root_path: &str,
    ) -> ContractResult<()> {
        self.upsert_project(project_id, project_id.as_str(), root_path)?;
        self.upsert_branch(
            branch_id,
            project_id,
            &BranchMetadata::new(branch_id.as_str()),
        )
    }

    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
        FileRepository::get_file(self, file_id)
    }

    fn cleanup_deleted_files(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        live_file_ids: &[FileId],
    ) -> ContractResult<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(to_contract_error)?;
        cleanup_deleted_files_tx(&transaction, project_id, branch_id, live_file_ids)?;
        transaction.commit().map_err(to_contract_error)?;
        Ok(())
    }

    fn upsert_indexed_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file: IndexedFileRecord,
    ) -> ContractResult<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(to_contract_error)?;
        upsert_indexed_file_tx(&transaction, project_id, branch_id, file)?;
        transaction.commit().map_err(to_contract_error)?;
        Ok(())
    }
}

impl IndexStore for &SqliteStorage {
    fn ensure_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        root_path: &str,
    ) -> ContractResult<()> {
        <SqliteStorage as IndexStore>::ensure_project_branch(
            *self, project_id, branch_id, root_path,
        )
    }

    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
        FileRepository::get_file(*self, file_id)
    }

    fn cleanup_deleted_files(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        live_file_ids: &[FileId],
    ) -> ContractResult<()> {
        <SqliteStorage as IndexStore>::cleanup_deleted_files(
            *self,
            project_id,
            branch_id,
            live_file_ids,
        )
    }

    fn upsert_indexed_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file: IndexedFileRecord,
    ) -> ContractResult<()> {
        <SqliteStorage as IndexStore>::upsert_indexed_file(*self, project_id, branch_id, file)
    }
}

impl FileRepository for SqliteStorage {
    fn get_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
        self.connection
            .prepare_cached(
                "SELECT id, project_id, path, content_hash
                 FROM files
                 WHERE id = ?1",
            )
            .map_err(to_contract_error)?
            .query_row([file_id.as_str()], |row| {
                Ok(FileRecord {
                    id: FileId::new(row.get::<_, String>(0)?),
                    project_id: ProjectId::new(row.get::<_, String>(1)?),
                    path: row.get(2)?,
                    content_hash: row.get(3)?,
                })
            })
            .optional()
            .map_err(to_contract_error)
    }
}

impl QueryRepository for SqliteStorage {
    fn find_symbols(&self, scope: &QueryScope, name: &str) -> ContractResult<Vec<QuerySymbol>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, file_id, name, kind, snippet, start_line, end_line, visibility
                 FROM symbols
                 WHERE project_id = ?1 AND branch_id = ?2 AND name = ?3
                 ORDER BY CASE WHEN name = ?3 THEN 0 ELSE 1 END, name, id",
            )
            .map_err(to_contract_error)?;
        let rows = statement
            .query_map(
                params![scope.project_id.as_str(), scope.branch_id.as_str(), name],
                query_symbol_from_row,
            )
            .map_err(to_contract_error)?;
        collect_rows(rows)
    }

    fn get_symbol(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Option<QuerySymbol>> {
        self.connection
            .prepare_cached(
                "SELECT id, file_id, name, kind, snippet, start_line, end_line, visibility
                 FROM symbols
                 WHERE project_id = ?1 AND branch_id = ?2 AND id = ?3",
            )
            .map_err(to_contract_error)?
            .query_row(
                params![
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    symbol_id.as_str()
                ],
                query_symbol_from_row,
            )
            .optional()
            .map_err(to_contract_error)
    }

    fn get_file(&self, scope: &QueryScope, file_id: &FileId) -> ContractResult<Option<QueryFile>> {
        self.connection
            .prepare_cached(
                "SELECT id, path, content_hash, language
                 FROM files
                 WHERE project_id = ?1 AND branch_id = ?2 AND id = ?3",
            )
            .map_err(to_contract_error)?
            .query_row(
                params![
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    file_id.as_str()
                ],
                |row| {
                    Ok(QueryFile {
                        id: FileId::new(row.get::<_, String>(0)?),
                        path: row.get(1)?,
                        content_hash: row.get(2)?,
                        language: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(to_contract_error)
    }

    fn fts_search(
        &self,
        scope: &QueryScope,
        query: &str,
        limit: usize,
    ) -> ContractResult<Vec<FtsSearchHit>> {
        let normalized_query = normalize_fts_query(query);
        if normalized_query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }

        let symbol_limit = limit as i64;
        let file_limit = limit.saturating_sub(limit / 2).max(1) as i64;
        let mut hits = Vec::new();

        let mut symbol_statement = self
            .connection
            .prepare_cached(
                "SELECT s.file_id, s.id, f.path, s.name, symbol_fts.snippet,
                        bm25(symbol_fts) AS score
                 FROM symbol_fts
                 JOIN symbols s ON s.id = symbol_fts.symbol_id
                 JOIN files f ON f.id = s.file_id
                 WHERE symbol_fts MATCH ?1
                   AND s.project_id = ?2
                   AND s.branch_id = ?3
                   AND f.branch_id = ?3
                 ORDER BY score
                 LIMIT ?4",
            )
            .map_err(to_contract_error)?;
        let symbol_rows = symbol_statement
            .query_map(
                params![
                    normalized_query.as_str(),
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    symbol_limit
                ],
                |row| {
                    Ok(FtsSearchHit {
                        file_id: FileId::new(row.get::<_, String>(0)?),
                        symbol_id: Some(SymbolId::new(row.get::<_, String>(1)?)),
                        path: row.get(2)?,
                        name: Some(row.get(3)?),
                        snippet: row.get(4)?,
                        score: row.get::<_, f64>(5)? as f32,
                    })
                },
            )
            .map_err(to_contract_error)?;
        hits.extend(collect_rows(symbol_rows)?);

        let mut file_statement = self
            .connection
            .prepare_cached(
                "SELECT f.id, f.path, substr(file_content_fts.content, 1, 400),
                        bm25(file_content_fts) AS score
                 FROM file_content_fts
                 JOIN files f ON f.id = file_content_fts.file_id
                 WHERE file_content_fts MATCH ?1
                   AND f.project_id = ?2
                   AND f.branch_id = ?3
                 ORDER BY score
                 LIMIT ?4",
            )
            .map_err(to_contract_error)?;
        let file_rows = file_statement
            .query_map(
                params![
                    normalized_query.as_str(),
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    file_limit
                ],
                |row| {
                    Ok(FtsSearchHit {
                        file_id: FileId::new(row.get::<_, String>(0)?),
                        symbol_id: None,
                        path: row.get(1)?,
                        name: None,
                        snippet: row.get(2)?,
                        score: row.get::<_, f64>(3)? as f32,
                    })
                },
            )
            .map_err(to_contract_error)?;
        hits.extend(collect_rows(file_rows)?);

        hits.sort_by(|left, right| left.score.total_cmp(&right.score));
        hits.truncate(limit);
        Ok(hits)
    }

    fn graph_neighbors(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        direction: GraphDirection,
        edge_filter: &[EdgeKind],
        min_confidence: u16,
    ) -> ContractResult<Vec<GraphNeighbor>> {
        let node_id = symbol_node_id(symbol_id);
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT e.id, from_symbol.id, to_symbol.id, e.edge_type, e.confidence_bps,
                        e.provenance
                 FROM edges e
                 JOIN nodes from_node ON from_node.id = e.from_node_id
                 JOIN nodes to_node ON to_node.id = e.to_node_id
                 LEFT JOIN symbols from_symbol ON from_symbol.id = from_node.symbol_id
                 LEFT JOIN symbols to_symbol ON to_symbol.id = to_node.symbol_id
                 WHERE e.project_id = ?1
                   AND e.branch_id = ?2
                   AND e.confidence_bps >= ?3
                   AND ((?4 = 1 AND e.from_node_id = ?6)
                     OR (?5 = 1 AND e.to_node_id = ?6))
                 ORDER BY e.confidence_bps DESC, e.edge_type, e.id",
            )
            .map_err(to_contract_error)?;

        let outbound = matches!(direction, GraphDirection::Outbound | GraphDirection::Both);
        let inbound = matches!(direction, GraphDirection::Inbound | GraphDirection::Both);
        let rows = statement
            .query_map(
                params![
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    i64::from(min_confidence),
                    bool_to_i64(outbound),
                    bool_to_i64(inbound),
                    node_id.as_str()
                ],
                |row| {
                    let confidence_bps = row
                        .get::<_, i64>(4)?
                        .clamp(0, i64::from(EdgeConfidence::MAX_BASIS_POINTS))
                        as u16;
                    Ok(GraphNeighbor {
                        edge_id: EdgeId::new(row.get::<_, String>(0)?),
                        from_symbol: row.get::<_, Option<String>>(1)?.map(SymbolId::new),
                        to_symbol: row.get::<_, Option<String>>(2)?.map(SymbolId::new),
                        edge_kind: parse_edge_kind(&row.get::<_, String>(3)?),
                        confidence: EdgeConfidence::from_basis_points(confidence_bps),
                        provenance: parse_edge_provenance(&row.get::<_, String>(5)?),
                    })
                },
            )
            .map_err(to_contract_error)?;

        let mut neighbors = collect_rows(rows)?;
        if !edge_filter.is_empty() {
            neighbors.retain(|neighbor| edge_filter.contains(&neighbor.edge_kind));
        }
        Ok(neighbors)
    }
}

impl QueryRepository for &SqliteStorage {
    fn find_symbols(&self, scope: &QueryScope, name: &str) -> ContractResult<Vec<QuerySymbol>> {
        <SqliteStorage as QueryRepository>::find_symbols(*self, scope, name)
    }

    fn get_symbol(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Option<QuerySymbol>> {
        <SqliteStorage as QueryRepository>::get_symbol(*self, scope, symbol_id)
    }

    fn get_file(&self, scope: &QueryScope, file_id: &FileId) -> ContractResult<Option<QueryFile>> {
        <SqliteStorage as QueryRepository>::get_file(*self, scope, file_id)
    }

    fn fts_search(
        &self,
        scope: &QueryScope,
        query: &str,
        limit: usize,
    ) -> ContractResult<Vec<FtsSearchHit>> {
        <SqliteStorage as QueryRepository>::fts_search(*self, scope, query, limit)
    }

    fn graph_neighbors(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        direction: GraphDirection,
        edge_filter: &[EdgeKind],
        min_confidence: u16,
    ) -> ContractResult<Vec<GraphNeighbor>> {
        <SqliteStorage as QueryRepository>::graph_neighbors(
            *self,
            scope,
            symbol_id,
            direction,
            edge_filter,
            min_confidence,
        )
    }
}

impl SymbolRepository for SqliteStorage {
    fn find_symbol(&self, project_id: &ProjectId, name: &str) -> ContractResult<Vec<SymbolRecord>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, file_id, name, kind, start_byte, end_byte, start_line,
                        start_column, end_line, end_column, visibility
                 FROM symbols
                 WHERE project_id = ?1 AND name = ?2
                 ORDER BY name, id",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(params![project_id.as_str(), name], |row| {
                Ok(SymbolRecord {
                    id: SymbolId::new(row.get::<_, String>(0)?),
                    file_id: FileId::new(row.get::<_, String>(1)?),
                    name: row.get(2)?,
                    kind: parse_node_kind(&row.get::<_, String>(3)?),
                    start_byte: row.get::<_, i64>(4)? as usize,
                    end_byte: row.get::<_, i64>(5)? as usize,
                    start_line: row.get::<_, i64>(6)? as usize,
                    start_column: row.get::<_, i64>(7)? as usize,
                    end_line: row.get::<_, i64>(8)? as usize,
                    end_column: row.get::<_, i64>(9)? as usize,
                    visibility: row.get(10)?,
                })
            })
            .map_err(to_contract_error)?;

        collect_rows(rows)
    }
}

impl GraphRepository for SqliteStorage {
    fn get_node(&self, node_id: &NodeId) -> ContractResult<Option<GraphNode>> {
        self.connection
            .prepare_cached(
                "SELECT id, project_id, label
                 FROM nodes
                 WHERE id = ?1",
            )
            .map_err(to_contract_error)?
            .query_row([node_id.as_str()], |row| {
                Ok(GraphNode {
                    id: NodeId::new(row.get::<_, String>(0)?),
                    project_id: ProjectId::new(row.get::<_, String>(1)?),
                    label: row.get(2)?,
                })
            })
            .optional()
            .map_err(to_contract_error)
    }

    fn get_edge(&self, edge_id: &EdgeId) -> ContractResult<Option<GraphEdge>> {
        self.connection
            .prepare_cached(
                "SELECT id, from_node_id, to_node_id, confidence_bps, provenance,
                        created_at_unix_ms, updated_at_unix_ms
                 FROM edges
                 WHERE id = ?1",
            )
            .map_err(to_contract_error)?
            .query_row([edge_id.as_str()], |row| {
                let confidence_bps = row
                    .get::<_, i64>(3)?
                    .clamp(0, i64::from(EdgeConfidence::MAX_BASIS_POINTS))
                    as u16;
                let provenance = row.get::<_, String>(4)?;

                Ok(GraphEdge {
                    id: EdgeId::new(row.get::<_, String>(0)?),
                    from: NodeId::new(row.get::<_, String>(1)?),
                    to: NodeId::new(row.get::<_, String>(2)?),
                    metadata: GraphEdgeMetadata {
                        confidence: EdgeConfidence::from_basis_points(confidence_bps),
                        provenance: parse_edge_provenance(&provenance),
                        created_at_unix_ms: row.get::<_, i64>(5)? as u64,
                        updated_at_unix_ms: row.get::<_, i64>(6)? as u64,
                    },
                })
            })
            .optional()
            .map_err(to_contract_error)
    }
}

impl TokenSavingsRepository for SqliteStorage {
    fn record_savings(&self, record: TokenSavingsRecord) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO savings_ledger (
                    tool_call_id, estimated_tokens_saved, returned_tokens,
                    avoided_file_reads, avoided_search_calls
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    record.tool_call_id.as_ref().map(|id| id.as_str()),
                    record.estimated_tokens_saved as i64,
                    record.returned_tokens as i64,
                    record.avoided_file_reads as i64,
                    record.avoided_search_calls as i64
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }
}

impl TokenSavingsRepository for &SqliteStorage {
    fn record_savings(&self, record: TokenSavingsRecord) -> ContractResult<()> {
        <SqliteStorage as TokenSavingsRepository>::record_savings(*self, record)
    }
}

fn upsert_indexed_file_tx(
    transaction: &Transaction<'_>,
    project_id: &ProjectId,
    branch_id: &BranchId,
    indexed: IndexedFileRecord,
) -> ContractResult<()> {
    let file = indexed.file;
    transaction
        .execute(
            "INSERT INTO files (id, project_id, branch_id, path, content_hash, language, size_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                branch_id = excluded.branch_id,
                path = excluded.path,
                content_hash = excluded.content_hash,
                language = excluded.language,
                size_bytes = excluded.size_bytes",
            params![
                file.id.as_str(),
                project_id.as_str(),
                branch_id.as_str(),
                file.path.as_str(),
                file.content_hash.as_str(),
                indexed.language.as_deref(),
                indexed.size_bytes as i64
            ],
        )
        .map_err(to_contract_error)?;

    delete_file_index_rows(transaction, branch_id, &file.id)?;

    transaction
        .execute(
            "DELETE FROM file_content_fts WHERE file_id = ?1",
            [file.id.as_str()],
        )
        .map_err(to_contract_error)?;
    transaction
        .execute(
            "INSERT INTO file_content_fts (file_id, path, content)
             VALUES (?1, ?2, ?3)",
            params![
                file.id.as_str(),
                file.path.as_str(),
                indexed.content.as_str()
            ],
        )
        .map_err(to_contract_error)?;

    let file_node_id = file_node_id(&file.id);
    transaction
        .execute(
            "INSERT INTO nodes (id, project_id, branch_id, kind, label, file_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                branch_id = excluded.branch_id,
                kind = excluded.kind,
                label = excluded.label,
                file_id = excluded.file_id",
            params![
                file_node_id.as_str(),
                project_id.as_str(),
                branch_id.as_str(),
                node_kind(NodeKind::File),
                file.path.as_str(),
                file.id.as_str()
            ],
        )
        .map_err(to_contract_error)?;

    for symbol in &indexed.symbols {
        transaction
            .execute(
                "INSERT INTO symbols (
                    id, project_id, branch_id, file_id, name, kind, snippet, content_hash,
                    start_byte, end_byte, start_line, start_column, end_line, end_column,
                    visibility
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(id) DO UPDATE SET
                    project_id = excluded.project_id,
                    branch_id = excluded.branch_id,
                    file_id = excluded.file_id,
                    name = excluded.name,
                    kind = excluded.kind,
                    snippet = excluded.snippet,
                    content_hash = excluded.content_hash,
                    start_byte = excluded.start_byte,
                    end_byte = excluded.end_byte,
                    start_line = excluded.start_line,
                    start_column = excluded.start_column,
                    end_line = excluded.end_line,
                    end_column = excluded.end_column,
                    visibility = excluded.visibility",
                params![
                    symbol.id.as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    symbol.file_id.as_str(),
                    symbol.name.as_str(),
                    node_kind(symbol.kind),
                    snippet(&indexed.content, symbol.start_byte, symbol.end_byte),
                    file.content_hash.as_str(),
                    symbol.start_byte as i64,
                    symbol.end_byte as i64,
                    symbol.start_line as i64,
                    symbol.start_column as i64,
                    symbol.end_line as i64,
                    symbol.end_column as i64,
                    symbol.visibility.as_deref()
                ],
            )
            .map_err(to_contract_error)?;

        transaction
            .execute(
                "INSERT INTO nodes (id, project_id, branch_id, kind, label, symbol_id, file_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    project_id = excluded.project_id,
                    branch_id = excluded.branch_id,
                    kind = excluded.kind,
                    label = excluded.label,
                    symbol_id = excluded.symbol_id,
                    file_id = excluded.file_id",
                params![
                    symbol_node_id(&symbol.id).as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    node_kind(symbol.kind),
                    symbol.name.as_str(),
                    symbol.id.as_str(),
                    symbol.file_id.as_str()
                ],
            )
            .map_err(to_contract_error)?;

        transaction
            .execute(
                "INSERT INTO symbol_fts (symbol_id, name, documentation, snippet)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    symbol.id.as_str(),
                    symbol.name.as_str(),
                    "",
                    snippet(&indexed.content, symbol.start_byte, symbol.end_byte)
                ],
            )
            .map_err(to_contract_error)?;

        transaction
            .execute(
                "INSERT INTO edges (
                    id, project_id, branch_id, edge_type, from_node_id, to_node_id,
                    confidence_bps, provenance, created_at_unix_ms, updated_at_unix_ms
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    edge_type = excluded.edge_type,
                    from_node_id = excluded.from_node_id,
                    to_node_id = excluded.to_node_id,
                    confidence_bps = excluded.confidence_bps,
                    provenance = excluded.provenance,
                    updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![
                    file_contains_edge_id(&file.id, &symbol.id).as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    edge_kind(EdgeKind::Contains),
                    file_node_id.as_str(),
                    symbol_node_id(&symbol.id).as_str(),
                    i64::from(EdgeConfidence::MAX_BASIS_POINTS),
                    edge_provenance(EdgeProvenance::Ast),
                    0_i64,
                    0_i64
                ],
            )
            .map_err(to_contract_error)?;
    }

    for edge in &indexed.edges {
        transaction
            .execute(
                "INSERT INTO edges (
                    id, project_id, branch_id, edge_type, from_node_id, to_node_id,
                    confidence_bps, provenance, created_at_unix_ms, updated_at_unix_ms
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(id) DO UPDATE SET
                    edge_type = excluded.edge_type,
                    from_node_id = excluded.from_node_id,
                    to_node_id = excluded.to_node_id,
                    confidence_bps = excluded.confidence_bps,
                    provenance = excluded.provenance,
                    updated_at_unix_ms = excluded.updated_at_unix_ms",
                params![
                    edge.id.as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    edge_kind(edge.kind),
                    symbol_node_id(&edge.from_symbol).as_str(),
                    symbol_node_id(&edge.to_symbol).as_str(),
                    i64::from(edge.metadata.confidence.basis_points()),
                    edge_provenance(edge.metadata.provenance),
                    edge.metadata.created_at_unix_ms as i64,
                    edge.metadata.updated_at_unix_ms as i64
                ],
            )
            .map_err(to_contract_error)?;
    }

    Ok(())
}

fn delete_file_index_rows(
    transaction: &Transaction<'_>,
    branch_id: &BranchId,
    file_id: &FileId,
) -> ContractResult<()> {
    transaction
        .execute(
            "DELETE FROM edges
             WHERE branch_id = ?1
               AND (
                    from_node_id IN (SELECT id FROM nodes WHERE file_id = ?2 AND branch_id = ?1)
                 OR to_node_id IN (SELECT id FROM nodes WHERE file_id = ?2 AND branch_id = ?1)
               )",
            params![branch_id.as_str(), file_id.as_str()],
        )
        .map_err(to_contract_error)?;
    transaction
        .execute(
            "DELETE FROM nodes WHERE file_id = ?1 AND branch_id = ?2",
            params![file_id.as_str(), branch_id.as_str()],
        )
        .map_err(to_contract_error)?;
    transaction
        .execute(
            "DELETE FROM symbol_fts
             WHERE symbol_id IN (
                SELECT id FROM symbols WHERE file_id = ?1 AND branch_id = ?2
             )",
            params![file_id.as_str(), branch_id.as_str()],
        )
        .map_err(to_contract_error)?;
    transaction
        .execute(
            "DELETE FROM symbols WHERE file_id = ?1 AND branch_id = ?2",
            params![file_id.as_str(), branch_id.as_str()],
        )
        .map_err(to_contract_error)?;
    Ok(())
}

fn cleanup_deleted_files_tx(
    transaction: &Transaction<'_>,
    project_id: &ProjectId,
    branch_id: &BranchId,
    live_file_ids: &[FileId],
) -> ContractResult<()> {
    let mut statement = transaction
        .prepare(
            "SELECT id FROM files
             WHERE project_id = ?1 AND branch_id = ?2
             ORDER BY id",
        )
        .map_err(to_contract_error)?;
    let rows = statement
        .query_map(params![project_id.as_str(), branch_id.as_str()], |row| {
            row.get::<_, String>(0)
        })
        .map_err(to_contract_error)?;
    let existing_file_ids = collect_rows(rows)?;
    drop(statement);

    for existing_file_id in existing_file_ids {
        let should_keep = live_file_ids
            .iter()
            .any(|live_file_id| live_file_id.as_str() == existing_file_id);

        if should_keep {
            continue;
        }

        let file_id = FileId::new(existing_file_id);
        delete_file_index_rows(transaction, branch_id, &file_id)?;
        transaction
            .execute(
                "DELETE FROM file_content_fts WHERE file_id = ?1",
                [file_id.as_str()],
            )
            .map_err(to_contract_error)?;
        transaction
            .execute(
                "DELETE FROM files
                 WHERE id = ?1 AND project_id = ?2 AND branch_id = ?3",
                params![file_id.as_str(), project_id.as_str(), branch_id.as_str()],
            )
            .map_err(to_contract_error)?;
    }

    Ok(())
}

fn collect_rows<T, F>(rows: rusqlite::MappedRows<'_, F>) -> ContractResult<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(to_contract_error)?);
    }
    Ok(values)
}

fn query_symbol_from_row(row: &Row<'_>) -> rusqlite::Result<QuerySymbol> {
    Ok(QuerySymbol {
        id: SymbolId::new(row.get::<_, String>(0)?),
        file_id: FileId::new(row.get::<_, String>(1)?),
        name: row.get(2)?,
        kind: parse_node_kind(&row.get::<_, String>(3)?),
        snippet: row.get(4)?,
        start_line: row.get::<_, i64>(5)? as usize,
        end_line: row.get::<_, i64>(6)? as usize,
        visibility: row.get(7)?,
    })
}

fn normalize_fts_query(query: &str) -> String {
    query
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn file_node_id(file_id: &FileId) -> NodeId {
    NodeId::new(format!("node-file-{}", file_id.as_str()))
}

fn symbol_node_id(symbol_id: &SymbolId) -> NodeId {
    NodeId::new(format!("node-symbol-{}", symbol_id.as_str()))
}

fn file_contains_edge_id(file_id: &FileId, symbol_id: &SymbolId) -> EdgeId {
    EdgeId::new(format!(
        "edge-file-contains-{}-{}",
        file_id.as_str(),
        symbol_id.as_str()
    ))
}

fn snippet(source: &str, start_byte: usize, end_byte: usize) -> &str {
    source.get(start_byte..end_byte).unwrap_or_default()
}

fn node_kind(kind: NodeKind) -> &'static str {
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
        "variable" => NodeKind::Variable,
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

fn edge_kind(kind: EdgeKind) -> &'static str {
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

fn edge_provenance(provenance: EdgeProvenance) -> &'static str {
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
        "text_heuristic" => EdgeProvenance::TextHeuristic,
        "semantic_similarity" => EdgeProvenance::SemanticSimilarity,
        "user_recorded" => EdgeProvenance::UserRecorded,
        _ => EdgeProvenance::TextHeuristic,
    }
}

fn to_contract_error(error: impl std::fmt::Display) -> ContractError {
    ContractError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{GraphEdge, GraphNode, ToolCallId};
    use tempfile::tempdir;

    #[test]
    fn opens_local_database_with_schema_and_fts() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("b3.db");

        let storage = SqliteStorage::open(path).expect("open sqlite storage");

        assert!(storage.is_local_only());
        assert!(storage.table_exists("projects").expect("projects table"));
        assert!(storage.table_exists("file_content_fts").expect("file fts"));
        assert!(storage.table_exists("symbol_fts").expect("symbol fts"));
        assert!(storage.migration_applied(1).expect("migration"));
        assert!(storage.index_exists("idx_edges_from").expect("edge index"));
        assert!(storage
            .index_exists("idx_files_branch_id")
            .expect("branch index"));
        assert_eq!(storage.pragma_i64("synchronous").expect("pragma"), 1);
    }

    #[test]
    fn repositories_round_trip_records() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        let branch = BranchMetadata::new("main");

        storage
            .upsert_project(&project_id, "Project", ".")
            .expect("project");
        storage
            .upsert_branch(&branch_id, &project_id, &branch)
            .expect("branch");

        let file = FileRecord {
            id: FileId::new("file"),
            project_id: project_id.clone(),
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
        };
        storage.upsert_file(&file, &branch_id).expect("file");

        let symbol = SymbolRecord::new(
            SymbolId::new("symbol"),
            file.id.clone(),
            "run",
            NodeKind::Function,
        );
        storage
            .upsert_symbol(&project_id, &branch_id, &symbol)
            .expect("symbol");

        let from = GraphNode {
            id: NodeId::new("from"),
            project_id: project_id.clone(),
            label: "from".to_string(),
        };
        let to = GraphNode {
            id: NodeId::new("to"),
            project_id: project_id.clone(),
            label: "to".to_string(),
        };
        storage
            .upsert_node(&from, &branch_id, NodeKind::Function)
            .expect("from node");
        storage
            .upsert_node(&to, &branch_id, NodeKind::Function)
            .expect("to node");

        let edge = GraphEdge {
            id: EdgeId::new("edge"),
            from: from.id.clone(),
            to: to.id.clone(),
            metadata: GraphEdgeMetadata {
                confidence: EdgeConfidence::from_basis_points(9_000),
                provenance: EdgeProvenance::Ast,
                created_at_unix_ms: 1,
                updated_at_unix_ms: 2,
            },
        };
        storage
            .upsert_edge(&project_id, &branch_id, &edge, EdgeKind::Calls)
            .expect("edge");

        assert_eq!(
            FileRepository::get_file(&storage, &file.id)
                .expect("get file")
                .expect("file")
                .path,
            "src/lib.rs"
        );
        assert_eq!(
            storage
                .find_symbol(&project_id, "run")
                .expect("find symbol")
                .len(),
            1
        );
        assert_eq!(
            storage
                .get_node(&from.id)
                .expect("get node")
                .expect("node")
                .label,
            "from"
        );
        assert_eq!(
            storage
                .get_edge(&edge.id)
                .expect("get edge")
                .expect("edge")
                .metadata
                .provenance,
            EdgeProvenance::Ast
        );

        storage
            .record_savings(TokenSavingsRecord {
                tool_call_id: Some(ToolCallId::new("tool-call")),
                estimated_tokens_saved: 100,
                returned_tokens: 10,
                avoided_file_reads: 2,
                avoided_search_calls: 1,
            })
            .expect("record savings");
    }
}
