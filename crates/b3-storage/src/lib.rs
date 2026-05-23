//! Storage boundary.
//!
//! This crate hosts the offline-first SQLite/libSQL storage foundation: WAL
//! setup, migrations, graph tables, FTS5 tables, token savings storage, and
//! repository implementations. It does not index files, generate embeddings,
//! rank retrieval results, or serve UI/MCP requests.

use std::path::Path;

use b3_core::{
    BranchId, BranchMetadata, ContractError, ContractResult, EdgeConfidence, EdgeId, EdgeKind,
    EdgeProvenance, FileId, FileRecord, FileRepository, GraphEdge, GraphEdgeMetadata, GraphNode,
    GraphRepository, NodeId, NodeKind, ProjectId, StorageProvider, SymbolId, SymbolRecord,
    SymbolRepository, TokenSavingsRecord, TokenSavingsRepository,
};
use rusqlite::{params, Connection, OptionalExtension, Row};

pub use b3_core::{
    FileRepository as FileRepositoryContract, GraphRepository as GraphRepositoryContract,
    StorageProvider as StorageProviderContract, SymbolRepository as SymbolRepositoryContract,
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
    start_line INTEGER NOT NULL DEFAULT 0,
    end_line INTEGER NOT NULL DEFAULT 0,
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
                "INSERT INTO symbols (id, project_id, branch_id, file_id, name)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    branch_id = excluded.branch_id,
                    file_id = excluded.file_id,
                    name = excluded.name",
                params![
                    record.id.as_str(),
                    project_id.as_str(),
                    branch_id.as_str(),
                    record.file_id.as_str(),
                    record.name.as_str()
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

impl SymbolRepository for SqliteStorage {
    fn find_symbol(&self, project_id: &ProjectId, name: &str) -> ContractResult<Vec<SymbolRecord>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, file_id, name
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

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
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

        let symbol = SymbolRecord {
            id: SymbolId::new("symbol"),
            file_id: file.id.clone(),
            name: "run".to_string(),
        };
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
            storage
                .get_file(&file.id)
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
