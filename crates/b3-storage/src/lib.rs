//! Storage boundary.
//!
//! This crate hosts the offline-first SQLite/libSQL storage foundation: WAL
//! setup, migrations, graph tables, FTS5 tables, token savings storage, and
//! repository implementations. It does not index files, generate embeddings,
//! rank retrieval results, or serve UI/MCP requests.

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use b3_core::ParseFailureRecord;
use b3_core::{
    BranchId, BranchMetadata, CentralityMetric, CentralityRepository, CentralitySnapshot,
    ContractError, ContractResult, EdgeConfidence, EdgeId, EdgeKind, EdgeProvenance,
    EmbeddingVector, FileId, FileRecord, FileRepository, FtsSearchHit, GraphDirection, GraphEdge,
    GraphEdgeMetadata, GraphNeighbor, GraphNode, GraphRepository, IndexStore, IndexedFileRecord,
    NodeId, NodeKind, ProjectId, QueryFile, QueryRepository, QueryScope, QuerySymbol, SourceKind,
    StorageProvider, SymbolId, SymbolRecord, SymbolRepository, TokenSavingsRecord,
    TokenSavingsRepository, VectorDocument, VectorSearchHit, VectorSearchRequest, VectorStore,
    VectorStoreStats,
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

CREATE TABLE IF NOT EXISTS centrality_snapshots (
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    symbol_id TEXT NOT NULL,
    algorithm_version TEXT NOT NULL,
    calculated_at_unix_ms INTEGER NOT NULL,
    in_degree INTEGER NOT NULL,
    out_degree INTEGER NOT NULL,
    fan_in INTEGER NOT NULL,
    fan_out INTEGER NOT NULL,
    degree_centrality REAL NOT NULL,
    pagerank_score REAL NOT NULL,
    component_size INTEGER,
    is_cycle_member INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(project_id, branch_id, symbol_id, algorithm_version),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE,
    FOREIGN KEY(symbol_id) REFERENCES symbols(id) ON DELETE CASCADE
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

const MIGRATION_002: &str = r#"
CREATE TABLE IF NOT EXISTS centrality_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    symbol_id TEXT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    pagerank_score REAL NOT NULL DEFAULT 0,
    in_degree INTEGER NOT NULL DEFAULT 0,
    out_degree INTEGER NOT NULL DEFAULT 0,
    fan_in INTEGER NOT NULL DEFAULT 0,
    fan_out INTEGER NOT NULL DEFAULT 0,
    degree_centrality REAL NOT NULL DEFAULT 0,
    component_size INTEGER NOT NULL DEFAULT 0,
    is_cycle_member INTEGER NOT NULL DEFAULT 0,
    calculated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    algorithm_version TEXT NOT NULL DEFAULT 'manual',
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE,
    FOREIGN KEY(node_id) REFERENCES nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(symbol_id) REFERENCES symbols(id) ON DELETE SET NULL,
    UNIQUE(project_id, branch_id, symbol_id, algorithm_version)
);

CREATE INDEX IF NOT EXISTS idx_centrality_scope_score
ON centrality_snapshots(project_id, branch_id, pagerank_score DESC);
"#;

const MIGRATION_003: &str = r#"
CREATE TABLE IF NOT EXISTS parse_failures (
    failure_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    language TEXT,
    error_kind TEXT NOT NULL,
    error_message TEXT NOT NULL,
    stderr_excerpt TEXT,
    failed_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_parse_failures_scope
ON parse_failures(project_id, branch_id, failed_at_unix_ms DESC);

CREATE INDEX IF NOT EXISTS idx_parse_failures_file
ON parse_failures(file_id);
"#;

const MIGRATION_004: &str = r#"
CREATE TABLE IF NOT EXISTS vector_documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    symbol_id TEXT,
    language TEXT,
    framework TEXT,
    source_kind TEXT NOT NULL,
    path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    chunk_hash TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    start_line INTEGER NOT NULL DEFAULT 0,
    end_line INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE,
    FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE,
    FOREIGN KEY(symbol_id) REFERENCES symbols(id) ON DELETE SET NULL,
    UNIQUE(project_id, branch_id, file_id, source_kind, chunk_hash, chunk_index)
);

CREATE TABLE IF NOT EXISTS embedding_vectors (
    document_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    dimension INTEGER NOT NULL,
    vector BLOB NOT NULL,
    vector_hash TEXT NOT NULL,
    indexed_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(document_id, provider_id),
    FOREIGN KEY(document_id) REFERENCES vector_documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vector_documents_scope
ON vector_documents(project_id, branch_id, source_kind, language, framework);

CREATE INDEX IF NOT EXISTS idx_vector_documents_file
ON vector_documents(project_id, branch_id, file_id);
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

/// Thread-safe SQLite-backed index-store adapter for worker/daemon boundaries.
///
/// `rusqlite::Connection` is not shared directly across threads. This wrapper
/// keeps the locking policy inside the storage crate so adapters such as the
/// control server do not need to know storage internals.
pub struct SharedSqliteIndexStore {
    storage: Mutex<SqliteStorage>,
}

impl SharedSqliteIndexStore {
    pub fn new(storage: SqliteStorage) -> Self {
        Self {
            storage: Mutex::new(storage),
        }
    }

    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&SqliteStorage) -> ContractResult<T>,
    ) -> ContractResult<T> {
        let storage = self
            .storage
            .lock()
            .map_err(|_| ContractError::new("sqlite index store lock poisoned"))?;
        operation(&storage)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageStats {
    pub files: usize,
    pub symbols: usize,
    pub edges: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavingsSummary {
    pub records: usize,
    pub estimated_tokens_saved: usize,
    pub returned_tokens: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCount {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGraphSummary {
    pub project_id: Option<String>,
    pub branch_id: Option<String>,
    pub node_count: usize,
    pub edge_count: usize,
    pub symbol_count: usize,
    pub file_count: usize,
    pub edge_type_counts: Vec<GraphCount>,
    pub node_kind_counts: Vec<GraphCount>,
    pub centrality_snapshot_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGraphNode {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub name: String,
    pub kind: String,
    pub file_path: Option<String>,
    pub symbol_id: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredGraphEdge {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub edge_type: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub confidence: u16,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRoute {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub method: String,
    pub path: String,
    pub framework: String,
    pub route_kind: String,
    pub file_path: String,
    pub symbol_id: String,
    pub handler_name: Option<String>,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredComponent {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub name: String,
    pub framework: String,
    pub file_path: String,
    pub symbol_id: String,
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
pub struct StoredDataAccess {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub technology: String,
    pub kind: String,
    pub operation: Option<String>,
    pub file_path: String,
    pub symbol_id: String,
    pub class_name: Option<String>,
    pub method_name: Option<String>,
    pub entity_name: Option<String>,
    pub context_name: Option<String>,
    pub repository_name: Option<String>,
    pub query_text: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRealtime {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub technology: String,
    pub kind: String,
    pub direction: String,
    pub event_name: Option<String>,
    pub channel_name: Option<String>,
    pub hub_name: Option<String>,
    pub method_name: Option<String>,
    pub endpoint: Option<String>,
    pub file_path: String,
    pub symbol_id: String,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMessaging {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
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
    pub symbol_id: String,
    pub class_name: Option<String>,
    pub function_name: Option<String>,
    pub method_name: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredInfrastructure {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
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
    pub symbol_id: String,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredWpf {
    pub id: String,
    pub project_id: String,
    pub branch_id: String,
    pub technology: String,
    pub kind: String,
    pub name: Option<String>,
    pub x_class: Option<String>,
    pub code_behind: Option<String>,
    pub view_model: Option<String>,
    pub binding_paths: Vec<String>,
    pub command_bindings: Vec<String>,
    pub resource_keys: Vec<String>,
    pub resource_sources: Vec<String>,
    pub data_context: Option<String>,
    pub file_path: String,
    pub symbol_id: String,
    pub line_start: usize,
    pub line_end: usize,
    pub confidence: u16,
    pub source_kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredCentralityRecord {
    pub node_id: String,
    pub symbol_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub pagerank_score: f64,
    pub in_degree: usize,
    pub out_degree: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub degree_centrality: f64,
    pub component_size: usize,
    pub is_cycle_member: bool,
    pub calculated_at_unix_ms: u64,
    pub algorithm_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredParseFailure {
    pub failure_id: String,
    pub project_id: String,
    pub branch_id: String,
    pub file_id: String,
    pub file_path: String,
    pub file_hash: String,
    pub language: Option<String>,
    pub error_kind: String,
    pub error_message: String,
    pub stderr_excerpt: Option<String>,
    pub failed_at_unix_ms: u64,
    pub retry_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewCentralityRecord {
    pub project_id: String,
    pub branch_id: String,
    pub node_id: String,
    pub symbol_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub pagerank_score: f64,
    pub in_degree: usize,
    pub out_degree: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub degree_centrality: f64,
    pub component_size: usize,
    pub is_cycle_member: bool,
    pub calculated_at_unix_ms: u64,
    pub algorithm_version: String,
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

        self.apply_migration(1, "initial_storage_schema", MIGRATION_001)?;
        self.apply_migration(2, "centrality_snapshot_schema", MIGRATION_002)?;
        self.apply_migration(3, "parse_failure_registry", MIGRATION_003)?;
        self.apply_migration(4, "vector_architecture_schema", MIGRATION_004)?;

        Ok(())
    }

    fn apply_migration(&mut self, version: i64, name: &str, sql: &str) -> ContractResult<()> {
        let applied = self
            .connection
            .query_row(
                "SELECT 1 FROM schema_migrations WHERE version = ?1",
                [version],
                |_| Ok(()),
            )
            .optional()
            .map_err(to_contract_error)?
            .is_some();

        if !applied {
            let transaction = self.connection.transaction().map_err(to_contract_error)?;
            transaction.execute_batch(sql).map_err(to_contract_error)?;
            transaction
                .execute(
                    "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                    params![version, name],
                )
                .map_err(to_contract_error)?;
            transaction.commit().map_err(to_contract_error)?;
        }

        self.ensure_phase4_columns()?;
        self.ensure_centrality_table()?;

        Ok(())
    }

    fn ensure_centrality_table(&self) -> ContractResult<()> {
        // Ensure the newer centrality schema (with node_id) exists.
        // MIGRATION_002 defines the richer schema; apply it here if the
        // existing table does not have the expected `node_id` column.
        let has_node_id: bool = self
            .connection
            .prepare("PRAGMA table_info(centrality_snapshots)")
            .map_err(to_contract_error)?
            .query_map([], |row: &Row| {
                Ok(row.get::<_, String>(1).unwrap_or_default())
            })
            .map_err(to_contract_error)?
            .filter_map(Result::ok)
            .any(|col| col == "node_id");

        if !has_node_id {
            // If the existing table lacks `node_id`, recreate it using the
            // migration 2 schema. In tests this runs against an in-memory DB
            // so it's safe to drop and recreate; in real migrations this
            // should be replaced with an ALTER TABLE-based migration.
            self.connection
                .execute_batch("DROP TABLE IF EXISTS centrality_snapshots;")
                .map_err(to_contract_error)?;
            self.connection
                .execute_batch(MIGRATION_002)
                .map_err(to_contract_error)?;
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

    pub fn storage_stats(&self) -> ContractResult<StorageStats> {
        Ok(StorageStats {
            files: self.count_table("files")?,
            symbols: self.count_table("symbols")?,
            edges: self.count_table("edges")?,
        })
    }

    pub fn vector_stats(&self) -> ContractResult<VectorStoreStats> {
        <Self as VectorStore>::stats(self)
    }

    pub fn record_parse_failure(&self, failure: &ParseFailureRecord) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO parse_failures (
                    failure_id, project_id, branch_id, file_id, file_path, file_hash,
                    language, error_kind, error_message, stderr_excerpt,
                    failed_at_unix_ms, retry_count
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(failure_id) DO UPDATE SET
                    error_kind = excluded.error_kind,
                    error_message = excluded.error_message,
                    stderr_excerpt = excluded.stderr_excerpt,
                    failed_at_unix_ms = excluded.failed_at_unix_ms,
                    retry_count = excluded.retry_count",
                params![
                    failure.failure_id.as_str(),
                    failure.project_id.as_str(),
                    failure.branch_id.as_str(),
                    failure.file_id.as_str(),
                    failure.file_path.as_str(),
                    failure.file_hash.as_str(),
                    failure.language.as_deref(),
                    failure.error_kind.as_str(),
                    failure.error_message.as_str(),
                    failure.stderr_excerpt.as_deref(),
                    failure.failed_at_unix_ms as i64,
                    failure.retry_count as i64
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn parse_failure_count(
        &self,
        project_id: Option<&str>,
        branch_id: Option<&str>,
    ) -> ContractResult<usize> {
        match (project_id, branch_id) {
            (Some(project_id), Some(branch_id)) => self
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM parse_failures
                     WHERE project_id = ?1 AND branch_id = ?2",
                    params![project_id, branch_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count as usize)
                .map_err(to_contract_error),
            _ => self
                .connection
                .query_row("SELECT COUNT(*) FROM parse_failures", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map(|count| count as usize)
                .map_err(to_contract_error),
        }
    }

    pub fn recent_parse_failures(&self, limit: usize) -> ContractResult<Vec<StoredParseFailure>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT failure_id, project_id, branch_id, file_id, file_path, file_hash,
                        language, error_kind, error_message, stderr_excerpt,
                        failed_at_unix_ms, retry_count
                 FROM parse_failures
                 ORDER BY failed_at_unix_ms DESC
                 LIMIT ?1",
            )
            .map_err(to_contract_error)?;
        let rows = statement
            .query_map([limit as i64], parse_failure_from_row)
            .map_err(to_contract_error)?;
        collect_rows(rows)
    }

    pub fn current_branch_name(&self) -> ContractResult<Option<String>> {
        self.connection
            .prepare_cached(
                "SELECT name
                 FROM branches
                 ORDER BY updated_at_unix_ms DESC, created_at_unix_ms DESC, name
                 LIMIT 1",
            )
            .map_err(to_contract_error)?
            .query_row([], |row| row.get::<_, String>(0))
            .optional()
            .map_err(to_contract_error)
    }

    pub fn project_roots(&self) -> ContractResult<Vec<String>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT root_path
                 FROM projects
                 ORDER BY name, id",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(to_contract_error)?;

        collect_rows(rows)
    }

    pub fn savings_summary(&self) -> ContractResult<SavingsSummary> {
        self.connection
            .query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(estimated_tokens_saved), 0),
                    COALESCE(SUM(returned_tokens), 0),
                    COALESCE(SUM(avoided_file_reads), 0),
                    COALESCE(SUM(avoided_search_calls), 0)
                 FROM savings_ledger",
                [],
                |row| {
                    Ok(SavingsSummary {
                        records: row.get::<_, i64>(0)? as usize,
                        estimated_tokens_saved: row.get::<_, i64>(1)? as usize,
                        returned_tokens: row.get::<_, i64>(2)? as usize,
                        avoided_file_reads: row.get::<_, i64>(3)? as usize,
                        avoided_search_calls: row.get::<_, i64>(4)? as usize,
                    })
                },
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

    pub fn graph_summary(
        &self,
        project_id: Option<&str>,
        branch_id: Option<&str>,
    ) -> ContractResult<StoredGraphSummary> {
        Ok(StoredGraphSummary {
            project_id: project_id.map(str::to_string).or(self.first_project_id()?),
            branch_id: branch_id.map(str::to_string).or(self.first_branch_id()?),
            node_count: self.count_scoped("nodes", project_id, branch_id)?,
            edge_count: self.count_scoped("edges", project_id, branch_id)?,
            symbol_count: self.count_scoped("symbols", project_id, branch_id)?,
            file_count: self.count_scoped("files", project_id, branch_id)?,
            edge_type_counts: self.group_counts("edges", "edge_type", project_id, branch_id)?,
            node_kind_counts: self.group_counts("nodes", "kind", project_id, branch_id)?,
            centrality_snapshot_count: self.count_scoped(
                "centrality_snapshots",
                project_id,
                branch_id,
            )?,
        })
    }

    pub fn graph_node_by_id(
        &self,
        project_id: &str,
        branch_id: &str,
        node_id: &str,
    ) -> ContractResult<Option<StoredGraphNode>> {
        self.connection
            .prepare_cached(
                "SELECT n.id, n.project_id, n.branch_id, n.label, n.kind,
                        f.path, n.symbol_id, f.language
                 FROM nodes n
                 LEFT JOIN files f ON f.id = n.file_id
                 WHERE n.project_id = ?1 AND n.branch_id = ?2 AND n.id = ?3",
            )
            .map_err(to_contract_error)?
            .query_row(params![project_id, branch_id, node_id], graph_node_from_row)
            .optional()
            .map_err(to_contract_error)
    }

    pub fn graph_node_by_symbol_id(
        &self,
        project_id: &str,
        branch_id: &str,
        symbol_id: &str,
    ) -> ContractResult<Option<StoredGraphNode>> {
        self.connection
            .prepare_cached(
                "SELECT n.id, n.project_id, n.branch_id, n.label, n.kind,
                        f.path, n.symbol_id, f.language
                 FROM nodes n
                 LEFT JOIN files f ON f.id = n.file_id
                 WHERE n.project_id = ?1 AND n.branch_id = ?2 AND n.symbol_id = ?3
                 LIMIT 1",
            )
            .map_err(to_contract_error)?
            .query_row(
                params![project_id, branch_id, symbol_id],
                graph_node_from_row,
            )
            .optional()
            .map_err(to_contract_error)
    }

    pub fn graph_nodes_by_ids(
        &self,
        project_id: &str,
        branch_id: &str,
        node_ids: &[String],
    ) -> ContractResult<Vec<StoredGraphNode>> {
        let mut nodes = Vec::new();
        for node_id in node_ids {
            if let Some(node) = self.graph_node_by_id(project_id, branch_id, node_id)? {
                nodes.push(node);
            }
        }
        Ok(nodes)
    }

    pub fn graph_edges_for_node(
        &self,
        project_id: &str,
        branch_id: &str,
        node_id: &str,
        min_confidence: u16,
        limit: usize,
    ) -> ContractResult<Vec<StoredGraphEdge>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, project_id, branch_id, edge_type, from_node_id, to_node_id,
                        confidence_bps, provenance
                 FROM edges
                 WHERE project_id = ?1
                    AND branch_id = ?2
                    AND confidence_bps >= ?3
                    AND (from_node_id = ?4 OR to_node_id = ?4)
                 ORDER BY id
                 LIMIT ?5",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![
                    project_id,
                    branch_id,
                    i64::from(min_confidence),
                    node_id,
                    limit as i64
                ],
                graph_edge_from_row,
            )
            .map_err(to_contract_error)?;

        collect_rows(rows)
    }

    pub fn graph_edges_scoped(
        &self,
        project_id: &str,
        branch_id: &str,
        min_confidence: u16,
        limit: usize,
    ) -> ContractResult<Vec<StoredGraphEdge>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, project_id, branch_id, edge_type, from_node_id, to_node_id,
                        confidence_bps, provenance
                 FROM edges
                 WHERE project_id = ?1 AND branch_id = ?2 AND confidence_bps >= ?3
                 ORDER BY id
                 LIMIT ?4",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![
                    project_id,
                    branch_id,
                    i64::from(min_confidence),
                    limit as i64
                ],
                graph_edge_from_row,
            )
            .map_err(to_contract_error)?;

        collect_rows(rows)
    }

    pub fn routes(
        &self,
        project_id: &str,
        branch_id: &str,
        framework: Option<&str>,
        method: Option<&str>,
        path: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredRoute>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2 AND s.kind = 'route'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(params![project_id, branch_id, limit as i64], route_from_row)
            .map_err(to_contract_error)?;
        let mut routes = collect_rows(rows)?;
        if let Some(framework) = framework {
            routes.retain(|route| route.framework == framework);
        }
        if let Some(method) = method {
            routes.retain(|route| route.method.eq_ignore_ascii_case(method));
        }
        if let Some(path) = path {
            routes.retain(|route| route.path == path);
        }
        Ok(routes)
    }

    pub fn components(
        &self,
        project_id: &str,
        branch_id: &str,
        framework: Option<&str>,
        name: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredComponent>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2
                   AND s.visibility LIKE '%component.framework=%'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![project_id, branch_id, limit as i64],
                component_from_row,
            )
            .map_err(to_contract_error)?;
        let mut components = collect_rows(rows)?;
        if let Some(framework) = framework {
            components.retain(|component| component.framework == framework);
        }
        if let Some(name) = name {
            components.retain(|component| component.name == name);
        }
        if let Some(file) = file {
            components.retain(|component| component.file_path == file);
        }
        Ok(components)
    }

    pub fn data_access(
        &self,
        project_id: &str,
        branch_id: &str,
        technology: Option<&str>,
        kind: Option<&str>,
        operation: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredDataAccess>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2
                   AND s.visibility LIKE '%data_access.technology=%'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![project_id, branch_id, limit as i64],
                data_access_from_row,
            )
            .map_err(to_contract_error)?;
        let mut records = collect_rows(rows)?;
        if let Some(technology) = technology {
            records.retain(|record| record.technology == technology);
        }
        if let Some(kind) = kind {
            records.retain(|record| record.kind == kind);
        }
        if let Some(operation) = operation {
            records.retain(|record| record.operation.as_deref() == Some(operation));
        }
        if let Some(file) = file {
            records.retain(|record| record.file_path == file);
        }
        Ok(records)
    }

    pub fn realtime(
        &self,
        project_id: &str,
        branch_id: &str,
        technology: Option<&str>,
        kind: Option<&str>,
        event: Option<&str>,
        file: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredRealtime>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2
                   AND s.visibility LIKE '%realtime.technology=%'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![project_id, branch_id, limit as i64],
                realtime_from_row,
            )
            .map_err(to_contract_error)?;
        let mut records = collect_rows(rows)?;
        if let Some(technology) = technology {
            records.retain(|record| record.technology == technology);
        }
        if let Some(kind) = kind {
            records.retain(|record| record.kind.eq_ignore_ascii_case(kind));
        }
        if let Some(event) = event {
            records.retain(|record| record.event_name.as_deref() == Some(event));
        }
        if let Some(file) = file {
            records.retain(|record| record.file_path == file);
        }
        Ok(records)
    }

    pub fn messaging(
        &self,
        project_id: &str,
        branch_id: &str,
        technology: Option<&str>,
        kind: Option<&str>,
        topic: Option<&str>,
        queue: Option<&str>,
        routing_key: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredMessaging>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2
                   AND s.visibility LIKE '%messaging.technology=%'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![project_id, branch_id, limit as i64],
                messaging_from_row,
            )
            .map_err(to_contract_error)?;
        let mut records = collect_rows(rows)?;
        if let Some(technology) = technology {
            records.retain(|record| record.technology == technology);
        }
        if let Some(kind) = kind {
            records.retain(|record| record.kind.eq_ignore_ascii_case(kind));
        }
        if let Some(topic) = topic {
            records.retain(|record| record.topic.as_deref() == Some(topic));
        }
        if let Some(queue) = queue {
            records.retain(|record| record.queue.as_deref() == Some(queue));
        }
        if let Some(routing_key) = routing_key {
            records.retain(|record| record.routing_key.as_deref() == Some(routing_key));
        }
        Ok(records)
    }

    pub fn infrastructure(
        &self,
        project_id: &str,
        branch_id: &str,
        technology: Option<&str>,
        kind: Option<&str>,
        name: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredInfrastructure>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2
                   AND s.visibility LIKE '%infrastructure.technology=%'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![project_id, branch_id, limit as i64],
                infrastructure_from_row,
            )
            .map_err(to_contract_error)?;
        let mut records = collect_rows(rows)?;
        if let Some(technology) = technology {
            records.retain(|record| record.technology == technology);
        }
        if let Some(kind) = kind {
            records.retain(|record| record.kind.eq_ignore_ascii_case(kind));
        }
        if let Some(name) = name {
            records.retain(|record| record.name.as_deref() == Some(name));
        }
        Ok(records)
    }

    pub fn wpf(
        &self,
        project_id: &str,
        branch_id: &str,
        kind: Option<&str>,
        binding: Option<&str>,
        command: Option<&str>,
        limit: usize,
    ) -> ContractResult<Vec<StoredWpf>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT s.id, s.project_id, s.branch_id, s.name, s.file_id, f.path,
                        s.start_line, s.end_line, s.visibility
                 FROM symbols s
                 JOIN files f ON f.id = s.file_id AND f.branch_id = s.branch_id
                 WHERE s.project_id = ?1 AND s.branch_id = ?2
                   AND s.visibility LIKE '%wpf.technology=%'
                 ORDER BY f.path, s.start_line, s.name
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(params![project_id, branch_id, limit as i64], wpf_from_row)
            .map_err(to_contract_error)?;
        let mut records = collect_rows(rows)?;
        if let Some(kind) = kind {
            records.retain(|record| record.kind.eq_ignore_ascii_case(kind));
        }
        if let Some(binding) = binding {
            records.retain(|record| record.binding_paths.iter().any(|value| value == binding));
        }
        if let Some(command) = command {
            records.retain(|record| record.command_bindings.iter().any(|value| value == command));
        }
        Ok(records)
    }

    pub fn centrality_snapshot(
        &self,
        project_id: &str,
        branch_id: &str,
        limit: usize,
    ) -> ContractResult<Vec<StoredCentralityRecord>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT node_id, symbol_id, name, kind, pagerank_score, in_degree,
                        out_degree, fan_in, fan_out, degree_centrality, component_size,
                        is_cycle_member, calculated_at_unix_ms, algorithm_version
                 FROM centrality_snapshots
                 WHERE project_id = ?1 AND branch_id = ?2
                 ORDER BY pagerank_score DESC, node_id
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(params![project_id, branch_id, limit as i64], |row| {
                Ok(StoredCentralityRecord {
                    node_id: row.get(0)?,
                    symbol_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    pagerank_score: row.get(4)?,
                    in_degree: row.get::<_, i64>(5)? as usize,
                    out_degree: row.get::<_, i64>(6)? as usize,
                    fan_in: row.get::<_, i64>(7)? as usize,
                    fan_out: row.get::<_, i64>(8)? as usize,
                    degree_centrality: row.get(9)?,
                    component_size: row.get::<_, i64>(10)? as usize,
                    is_cycle_member: row.get::<_, i64>(11)? != 0,
                    calculated_at_unix_ms: row.get::<_, i64>(12)? as u64,
                    algorithm_version: row.get(13)?,
                })
            })
            .map_err(to_contract_error)?;

        collect_rows(rows)
    }

    pub fn insert_centrality_snapshot(&self, record: &NewCentralityRecord) -> ContractResult<()> {
        self.connection
            .execute(
                "INSERT INTO centrality_snapshots (
                    project_id, branch_id, node_id, symbol_id, name, kind, pagerank_score,
                    in_degree, out_degree, fan_in, fan_out, degree_centrality,
                    component_size, is_cycle_member, calculated_at_unix_ms, algorithm_version
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    record.project_id,
                    record.branch_id,
                    record.node_id,
                    record.symbol_id,
                    record.name,
                    record.kind,
                    record.pagerank_score,
                    record.in_degree as i64,
                    record.out_degree as i64,
                    record.fan_in as i64,
                    record.fan_out as i64,
                    record.degree_centrality,
                    record.component_size as i64,
                    bool_to_i64(record.is_cycle_member),
                    record.calculated_at_unix_ms as i64,
                    record.algorithm_version
                ],
            )
            .map_err(to_contract_error)?;
        Ok(())
    }

    pub fn remove_file_by_path(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        path: &str,
    ) -> ContractResult<()> {
        self.connection
            .execute(
                "DELETE FROM files WHERE project_id = ?1 AND branch_id = ?2 AND path = ?3",
                params![project_id.as_str(), branch_id.as_str(), path],
            )
            .map_err(to_contract_error)?;
        self.connection
            .execute(
                "DELETE FROM file_content_fts WHERE path = ?1",
                params![path],
            )
            .map_err(to_contract_error)?;
        Ok(())
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

    fn count_table(&self, table_name: &'static str) -> ContractResult<usize> {
        let sql = format!("SELECT COUNT(*) FROM {table_name}");
        let count = self
            .connection
            .query_row(&sql, [], |row| row.get::<_, i64>(0))
            .map_err(to_contract_error)?;
        Ok(count as usize)
    }

    fn first_project_id(&self) -> ContractResult<Option<String>> {
        self.connection
            .query_row("SELECT id FROM projects ORDER BY id LIMIT 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(to_contract_error)
    }

    fn first_branch_id(&self) -> ContractResult<Option<String>> {
        self.connection
            .query_row("SELECT id FROM branches ORDER BY id LIMIT 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(to_contract_error)
    }

    fn count_scoped(
        &self,
        table_name: &'static str,
        project_id: Option<&str>,
        branch_id: Option<&str>,
    ) -> ContractResult<usize> {
        let count = match (project_id, branch_id) {
            (Some(project_id), Some(branch_id)) => self.connection.query_row(
                &format!(
                    "SELECT COUNT(*) FROM {table_name} WHERE project_id = ?1 AND branch_id = ?2"
                ),
                params![project_id, branch_id],
                |row| row.get::<_, i64>(0),
            ),
            (Some(project_id), None) => self.connection.query_row(
                &format!("SELECT COUNT(*) FROM {table_name} WHERE project_id = ?1"),
                params![project_id],
                |row| row.get::<_, i64>(0),
            ),
            (None, Some(branch_id)) => self.connection.query_row(
                &format!("SELECT COUNT(*) FROM {table_name} WHERE branch_id = ?1"),
                params![branch_id],
                |row| row.get::<_, i64>(0),
            ),
            (None, None) => self.connection.query_row(
                &format!("SELECT COUNT(*) FROM {table_name}"),
                [],
                |row| row.get::<_, i64>(0),
            ),
        }
        .map_err(to_contract_error)?;
        Ok(count as usize)
    }

    fn group_counts(
        &self,
        table_name: &'static str,
        column_name: &'static str,
        project_id: Option<&str>,
        branch_id: Option<&str>,
    ) -> ContractResult<Vec<GraphCount>> {
        let sql = match (project_id, branch_id) {
            (Some(_), Some(_)) => format!(
                "SELECT {column_name}, COUNT(*) FROM {table_name}
                 WHERE project_id = ?1 AND branch_id = ?2
                 GROUP BY {column_name} ORDER BY {column_name}"
            ),
            (Some(_), None) => format!(
                "SELECT {column_name}, COUNT(*) FROM {table_name}
                 WHERE project_id = ?1 GROUP BY {column_name} ORDER BY {column_name}"
            ),
            (None, Some(_)) => format!(
                "SELECT {column_name}, COUNT(*) FROM {table_name}
                 WHERE branch_id = ?1 GROUP BY {column_name} ORDER BY {column_name}"
            ),
            (None, None) => format!(
                "SELECT {column_name}, COUNT(*) FROM {table_name}
                 GROUP BY {column_name} ORDER BY {column_name}"
            ),
        };

        let mut statement = self.connection.prepare(&sql).map_err(to_contract_error)?;
        let rows = match (project_id, branch_id) {
            (Some(project_id), Some(branch_id)) => statement
                .query_map(params![project_id, branch_id], graph_count_from_row)
                .map_err(to_contract_error)?,
            (Some(project_id), None) => statement
                .query_map(params![project_id], graph_count_from_row)
                .map_err(to_contract_error)?,
            (None, Some(branch_id)) => statement
                .query_map(params![branch_id], graph_count_from_row)
                .map_err(to_contract_error)?,
            (None, None) => statement
                .query_map([], graph_count_from_row)
                .map_err(to_contract_error)?,
        };

        collect_rows(rows)
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

    fn record_parse_failure(&self, failure: ParseFailureRecord) -> ContractResult<()> {
        SqliteStorage::record_parse_failure(self, &failure)
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

    fn record_parse_failure(&self, failure: ParseFailureRecord) -> ContractResult<()> {
        SqliteStorage::record_parse_failure(self, &failure)
    }
}

impl IndexStore for SharedSqliteIndexStore {
    fn ensure_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        root_path: &str,
    ) -> ContractResult<()> {
        self.with_storage(|storage| storage.ensure_project_branch(project_id, branch_id, root_path))
    }

    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>> {
        self.with_storage(|storage| FileRepository::get_file(storage, file_id))
    }

    fn cleanup_deleted_files(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        live_file_ids: &[FileId],
    ) -> ContractResult<()> {
        self.with_storage(|storage| {
            storage.cleanup_deleted_files(project_id, branch_id, live_file_ids)
        })
    }

    fn upsert_indexed_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file: IndexedFileRecord,
    ) -> ContractResult<()> {
        self.with_storage(|storage| storage.upsert_indexed_file(project_id, branch_id, file))
    }

    fn remove_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        path: &str,
    ) -> ContractResult<()> {
        self.with_storage(|storage| storage.remove_file_by_path(project_id, branch_id, path))
    }

    fn record_parse_failure(&self, failure: ParseFailureRecord) -> ContractResult<()> {
        self.with_storage(|storage| SqliteStorage::record_parse_failure(storage, &failure))
    }
}

impl VectorStore for SqliteStorage {
    fn upsert_documents(&self, documents: &[VectorDocument]) -> ContractResult<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(to_contract_error)?;
        for document in documents {
            let metadata_json =
                serde_json::to_string(&document.metadata).map_err(to_contract_error)?;
            transaction
                .execute(
                    "INSERT INTO vector_documents (
                        id, project_id, branch_id, file_id, symbol_id, language, framework,
                        source_kind, path, content_hash, chunk_hash, chunk_index, text,
                        start_line, end_line, metadata_json
                     )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                     ON CONFLICT(id) DO UPDATE SET
                        project_id = excluded.project_id,
                        branch_id = excluded.branch_id,
                        file_id = excluded.file_id,
                        symbol_id = excluded.symbol_id,
                        language = excluded.language,
                        framework = excluded.framework,
                        source_kind = excluded.source_kind,
                        path = excluded.path,
                        content_hash = excluded.content_hash,
                        chunk_hash = excluded.chunk_hash,
                        chunk_index = excluded.chunk_index,
                        text = excluded.text,
                        start_line = excluded.start_line,
                        end_line = excluded.end_line,
                        metadata_json = excluded.metadata_json",
                    params![
                        document.id.as_str(),
                        document.project_id.as_str(),
                        document.branch_id.as_str(),
                        document.file_id.as_str(),
                        document.symbol_id.as_ref().map(|id| id.as_str()),
                        document.language.as_deref(),
                        document.framework.as_deref(),
                        document.source_kind.as_str(),
                        document.path.as_str(),
                        document.content_hash.as_str(),
                        document.chunk_hash.as_str(),
                        document.chunk_index as i64,
                        document.text.as_str(),
                        document.start_line as i64,
                        document.end_line as i64,
                        metadata_json,
                    ],
                )
                .map_err(to_contract_error)?;
        }
        transaction.commit().map_err(to_contract_error)?;
        Ok(())
    }

    fn upsert_vectors(&self, vectors: &[EmbeddingVector]) -> ContractResult<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(to_contract_error)?;
        for vector in vectors {
            transaction
                .execute(
                    "INSERT INTO embedding_vectors (
                        document_id, provider_id, dimension, vector, vector_hash, indexed_at_unix_ms
                     )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(document_id, provider_id) DO UPDATE SET
                        dimension = excluded.dimension,
                        vector = excluded.vector,
                        vector_hash = excluded.vector_hash,
                        indexed_at_unix_ms = excluded.indexed_at_unix_ms",
                    params![
                        vector.document_id.as_str(),
                        vector.provider_id.as_str(),
                        vector.dimension as i64,
                        encode_vector(&vector.vector),
                        vector.vector_hash.as_str(),
                        vector.indexed_at_unix_ms as i64,
                    ],
                )
                .map_err(to_contract_error)?;
        }
        transaction.commit().map_err(to_contract_error)?;
        Ok(())
    }

    fn delete_by_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file_id: &FileId,
    ) -> ContractResult<usize> {
        self.connection
            .execute(
                "DELETE FROM vector_documents
                 WHERE project_id = ?1 AND branch_id = ?2 AND file_id = ?3",
                params![project_id.as_str(), branch_id.as_str(), file_id.as_str()],
            )
            .map_err(to_contract_error)
    }

    fn delete_by_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> ContractResult<usize> {
        self.connection
            .execute(
                "DELETE FROM vector_documents WHERE project_id = ?1 AND branch_id = ?2",
                params![project_id.as_str(), branch_id.as_str()],
            )
            .map_err(to_contract_error)
    }

    fn search(&self, request: VectorSearchRequest) -> ContractResult<Vec<VectorSearchHit>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT d.id, d.project_id, d.branch_id, d.file_id, d.symbol_id,
                        d.language, d.framework, d.source_kind, d.path, d.content_hash,
                        d.chunk_hash, d.chunk_index, d.text, d.start_line, d.end_line,
                        d.metadata_json, v.provider_id, v.dimension, v.vector
                 FROM vector_documents d
                 JOIN embedding_vectors v ON v.document_id = d.id
                 WHERE d.project_id = ?1 AND d.branch_id = ?2
                 ORDER BY d.path, d.chunk_index",
            )
            .map_err(to_contract_error)?;

        let rows = statement
            .query_map(
                params![request.project_id.as_str(), request.branch_id.as_str()],
                row_to_vector_document_with_vector,
            )
            .map_err(to_contract_error)?;

        let mut hits = Vec::new();
        for row in rows {
            let (document, provider_id, vector) = row.map_err(to_contract_error)?;
            if let Some(language) = &request.language {
                if document.language.as_ref() != Some(language) {
                    continue;
                }
            }
            if let Some(framework) = &request.framework {
                if document.framework.as_ref() != Some(framework) {
                    continue;
                }
            }
            if let Some(source_kind) = request.source_kind {
                if document.source_kind != source_kind {
                    continue;
                }
            }
            let score = cosine_similarity(&request.query_vector, &vector);
            if request.min_score.is_some_and(|min_score| score < min_score) {
                continue;
            }
            hits.push(VectorSearchHit {
                document,
                score,
                distance: 1.0 - score,
                provider_id,
            });
        }
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(request.limit);
        Ok(hits)
    }

    fn get_document(&self, document_id: &str) -> ContractResult<Option<VectorDocument>> {
        self.connection
            .prepare_cached(
                "SELECT id, project_id, branch_id, file_id, symbol_id, language, framework,
                        source_kind, path, content_hash, chunk_hash, chunk_index, text,
                        start_line, end_line, metadata_json
                 FROM vector_documents
                 WHERE id = ?1",
            )
            .map_err(to_contract_error)?
            .query_row([document_id], row_to_vector_document)
            .optional()
            .map_err(to_contract_error)
    }

    fn stats(&self) -> ContractResult<VectorStoreStats> {
        Ok(VectorStoreStats {
            documents: self.count_table("vector_documents")?,
            vectors: self.count_table("embedding_vectors")?,
        })
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
    fn list_symbols(&self, scope: &QueryScope, limit: usize) -> ContractResult<Vec<QuerySymbol>> {
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT id, file_id, name, kind, snippet, start_line, end_line, visibility
                 FROM symbols
                 WHERE project_id = ?1 AND branch_id = ?2
                 ORDER BY name, id
                 LIMIT ?3",
            )
            .map_err(to_contract_error)?;
        let rows = statement
            .query_map(
                params![
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    limit as i64
                ],
                query_symbol_from_row,
            )
            .map_err(to_contract_error)?;
        collect_rows(rows)
    }

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
    fn list_symbols(&self, scope: &QueryScope, limit: usize) -> ContractResult<Vec<QuerySymbol>> {
        <SqliteStorage as QueryRepository>::list_symbols(*self, scope, limit)
    }

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

impl CentralityRepository for SqliteStorage {
    fn get_centrality_metric(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Option<CentralityMetric>> {
        self.connection
            .prepare_cached(
                "SELECT symbol_id, in_degree, out_degree, fan_in, fan_out,
                        degree_centrality, pagerank_score, component_size,
                        is_cycle_member, algorithm_version, calculated_at_unix_ms
                 FROM centrality_snapshots
                 WHERE project_id = ?1 AND branch_id = ?2 AND symbol_id = ?3
                 ORDER BY calculated_at_unix_ms DESC, algorithm_version DESC
                 LIMIT 1",
            )
            .map_err(to_contract_error)?
            .query_row(
                params![
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    symbol_id.as_str()
                ],
                centrality_metric_from_row,
            )
            .optional()
            .map_err(to_contract_error)
    }

    fn upsert_centrality_snapshot(
        &self,
        scope: &QueryScope,
        snapshot: CentralitySnapshot,
    ) -> ContractResult<()> {
        let mut statement = self
            .connection
            .prepare_cached(
                "INSERT INTO centrality_snapshots (
                    project_id, branch_id, node_id, symbol_id, name, kind, algorithm_version, calculated_at_unix_ms,
                    in_degree, out_degree, fan_in, fan_out, degree_centrality, pagerank_score,
                    component_size, is_cycle_member
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                 ON CONFLICT(project_id, branch_id, symbol_id, algorithm_version) DO UPDATE SET
                    node_id = excluded.node_id,
                    name = excluded.name,
                    kind = excluded.kind,
                    calculated_at_unix_ms = excluded.calculated_at_unix_ms,
                    in_degree = excluded.in_degree,
                    out_degree = excluded.out_degree,
                    fan_in = excluded.fan_in,
                    fan_out = excluded.fan_out,
                    degree_centrality = excluded.degree_centrality,
                    pagerank_score = excluded.pagerank_score,
                    component_size = excluded.component_size,
                    is_cycle_member = excluded.is_cycle_member",
            )
            .map_err(to_contract_error)?;

        for metric in snapshot.metrics {
            let symbol_id = SymbolId::new(metric.symbol_id.clone());
            let (name, kind) = match self.get_symbol(scope, &symbol_id) {
                Ok(Some(symbol)) => (symbol.name, node_kind(symbol.kind).to_string()),
                _ => (metric.symbol_id.clone(), "unknown".to_string()),
            };

            statement
                .execute(params![
                    scope.project_id.as_str(),
                    scope.branch_id.as_str(),
                    symbol_node_id(&symbol_id).as_str(),
                    metric.symbol_id.as_str(),
                    name.as_str(),
                    kind.as_str(),
                    snapshot.algorithm_version.as_str(),
                    snapshot.calculated_at_unix_ms as i64,
                    metric.in_degree as i64,
                    metric.out_degree as i64,
                    metric.fan_in as i64,
                    metric.fan_out as i64,
                    metric.degree_centrality,
                    metric.pagerank_score,
                    metric.component_size.map(|value| value as i64),
                    bool_to_i64(metric.is_cycle_member)
                ])
                .map_err(to_contract_error)?;
        }
        Ok(())
    }
}

impl CentralityRepository for &SqliteStorage {
    fn get_centrality_metric(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Option<CentralityMetric>> {
        <SqliteStorage as CentralityRepository>::get_centrality_metric(*self, scope, symbol_id)
    }

    fn upsert_centrality_snapshot(
        &self,
        scope: &QueryScope,
        snapshot: CentralitySnapshot,
    ) -> ContractResult<()> {
        <SqliteStorage as CentralityRepository>::upsert_centrality_snapshot(*self, scope, snapshot)
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

fn row_to_vector_document(row: &Row<'_>) -> rusqlite::Result<VectorDocument> {
    let metadata_json: String = row.get(15)?;
    let metadata = serde_json::from_str::<BTreeMap<String, String>>(&metadata_json)
        .unwrap_or_else(|_| BTreeMap::new());
    Ok(VectorDocument {
        id: row.get(0)?,
        project_id: ProjectId::new(row.get::<_, String>(1)?),
        branch_id: BranchId::new(row.get::<_, String>(2)?),
        file_id: FileId::new(row.get::<_, String>(3)?),
        symbol_id: row.get::<_, Option<String>>(4)?.map(SymbolId::new),
        language: row.get(5)?,
        framework: row.get(6)?,
        source_kind: parse_source_kind(&row.get::<_, String>(7)?),
        path: row.get(8)?,
        content_hash: row.get(9)?,
        chunk_hash: row.get(10)?,
        chunk_index: row.get::<_, i64>(11)? as usize,
        text: row.get(12)?,
        start_line: row.get::<_, i64>(13)? as usize,
        end_line: row.get::<_, i64>(14)? as usize,
        metadata,
    })
}

fn row_to_vector_document_with_vector(
    row: &Row<'_>,
) -> rusqlite::Result<(VectorDocument, String, Vec<f32>)> {
    let document = row_to_vector_document(row)?;
    let provider_id = row.get(16)?;
    let vector = decode_vector(&row.get::<_, Vec<u8>>(18)?);
    Ok((document, provider_id, vector))
}

fn parse_source_kind(value: &str) -> SourceKind {
    match value {
        "SymbolChunk" => SourceKind::SymbolChunk,
        "RouteChunk" => SourceKind::RouteChunk,
        "ComponentChunk" => SourceKind::ComponentChunk,
        "DataAccessChunk" => SourceKind::DataAccessChunk,
        "RealtimeChunk" => SourceKind::RealtimeChunk,
        "MessagingChunk" => SourceKind::MessagingChunk,
        "InfrastructureChunk" => SourceKind::InfrastructureChunk,
        "WpfChunk" => SourceKind::WpfChunk,
        "GoChunk" => SourceKind::GoChunk,
        _ => SourceKind::FileChunk,
    }
}

fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_mag = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_mag = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_mag == 0.0 || right_mag == 0.0 {
        return 0.0;
    }
    dot / (left_mag * right_mag)
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

fn graph_count_from_row(row: &Row<'_>) -> rusqlite::Result<GraphCount> {
    Ok(GraphCount {
        name: row.get(0)?,
        count: row.get::<_, i64>(1)? as usize,
    })
}

fn graph_node_from_row(row: &Row<'_>) -> rusqlite::Result<StoredGraphNode> {
    Ok(StoredGraphNode {
        id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        name: row.get(3)?,
        kind: row.get(4)?,
        file_path: row.get(5)?,
        symbol_id: row.get(6)?,
        language: row.get(7)?,
        visibility: None,
        provenance: None,
    })
}

fn graph_edge_from_row(row: &Row<'_>) -> rusqlite::Result<StoredGraphEdge> {
    Ok(StoredGraphEdge {
        id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        edge_type: row.get(3)?,
        from_node_id: row.get(4)?,
        to_node_id: row.get(5)?,
        confidence: row
            .get::<_, i64>(6)?
            .clamp(0, i64::from(EdgeConfidence::MAX_BASIS_POINTS)) as u16,
        provenance: row.get(7)?,
    })
}

fn route_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRoute> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let name: String = row.get(3)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    let (fallback_method, fallback_path) = name
        .split_once(' ')
        .map(|(method, path)| (method.to_string(), path.to_string()))
        .unwrap_or_else(|| ("UNKNOWN".to_string(), name.clone()));
    Ok(StoredRoute {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        method: route_metadata_value(&metadata, "method").unwrap_or(fallback_method),
        path: route_metadata_value(&metadata, "path").unwrap_or(fallback_path),
        framework: route_metadata_value(&metadata, "framework")
            .unwrap_or_else(|| "unknown".to_string()),
        route_kind: route_metadata_value(&metadata, "kind").unwrap_or_else(|| "api".to_string()),
        file_path: route_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        handler_name: route_metadata_value(&metadata, "handler"),
        class_name: route_metadata_value(&metadata, "class"),
        function_name: route_metadata_value(&metadata, "function"),
        line_start: route_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: route_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: route_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: route_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn component_from_row(row: &Row<'_>) -> rusqlite::Result<StoredComponent> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let name: String = row.get(3)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredComponent {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        name,
        framework: component_metadata_value(&metadata, "framework")
            .unwrap_or_else(|| "react".to_string()),
        file_path,
        symbol_id,
        export_kind: component_metadata_value(&metadata, "export"),
        component_kind: component_metadata_value(&metadata, "kind")
            .unwrap_or_else(|| "unknown".to_string()),
        props_type_name: component_metadata_value(&metadata, "props"),
        hooks: component_metadata_value(&metadata, "hooks")
            .map(|hooks| split_metadata_list(&hooks))
            .unwrap_or_default(),
        usages: component_metadata_value(&metadata, "usages")
            .map(|usages| split_metadata_list(&usages))
            .unwrap_or_default(),
        line_start: component_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: component_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: component_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: component_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn data_access_from_row(row: &Row<'_>) -> rusqlite::Result<StoredDataAccess> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredDataAccess {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: data_access_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: data_access_metadata_value(&metadata, "kind")
            .unwrap_or_else(|| "Unknown".to_string()),
        operation: data_access_metadata_value(&metadata, "operation"),
        file_path: data_access_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        class_name: data_access_metadata_value(&metadata, "class"),
        method_name: data_access_metadata_value(&metadata, "method"),
        entity_name: data_access_metadata_value(&metadata, "entity"),
        context_name: data_access_metadata_value(&metadata, "context"),
        repository_name: data_access_metadata_value(&metadata, "repository"),
        query_text: data_access_metadata_value(&metadata, "query"),
        line_start: data_access_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: data_access_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: data_access_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: data_access_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn realtime_from_row(row: &Row<'_>) -> rusqlite::Result<StoredRealtime> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredRealtime {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: realtime_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: realtime_metadata_value(&metadata, "kind").unwrap_or_else(|| "Unknown".to_string()),
        direction: realtime_metadata_value(&metadata, "direction")
            .unwrap_or_else(|| "unknown".to_string()),
        event_name: realtime_metadata_value(&metadata, "event"),
        channel_name: realtime_metadata_value(&metadata, "channel"),
        hub_name: realtime_metadata_value(&metadata, "hub"),
        method_name: realtime_metadata_value(&metadata, "method"),
        endpoint: realtime_metadata_value(&metadata, "endpoint"),
        file_path: realtime_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        class_name: realtime_metadata_value(&metadata, "class"),
        function_name: realtime_metadata_value(&metadata, "function"),
        line_start: realtime_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: realtime_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: realtime_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: realtime_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn messaging_from_row(row: &Row<'_>) -> rusqlite::Result<StoredMessaging> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredMessaging {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: messaging_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: messaging_metadata_value(&metadata, "kind").unwrap_or_else(|| "Unknown".to_string()),
        direction: messaging_metadata_value(&metadata, "direction")
            .unwrap_or_else(|| "unknown".to_string()),
        topic: messaging_metadata_value(&metadata, "topic"),
        queue: messaging_metadata_value(&metadata, "queue"),
        exchange: messaging_metadata_value(&metadata, "exchange"),
        routing_key: messaging_metadata_value(&metadata, "routing_key"),
        pattern: messaging_metadata_value(&metadata, "pattern"),
        consumer_group: messaging_metadata_value(&metadata, "consumer_group"),
        file_path: messaging_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        class_name: messaging_metadata_value(&metadata, "class"),
        function_name: messaging_metadata_value(&metadata, "function"),
        method_name: messaging_metadata_value(&metadata, "method"),
        line_start: messaging_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: messaging_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: messaging_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: messaging_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn infrastructure_from_row(row: &Row<'_>) -> rusqlite::Result<StoredInfrastructure> {
    let symbol_id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let file_path: String = row.get(5)?;
    let line_start = row.get::<_, i64>(6)? as usize;
    let line_end = row.get::<_, i64>(7)? as usize;
    let metadata = row.get::<_, Option<String>>(8)?.unwrap_or_default();
    Ok(StoredInfrastructure {
        id: symbol_node_id(&SymbolId::new(symbol_id.clone()))
            .as_str()
            .to_string(),
        project_id,
        branch_id,
        technology: infrastructure_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "unknown".to_string()),
        kind: infrastructure_metadata_value(&metadata, "kind")
            .unwrap_or_else(|| "Unknown".to_string()),
        name: infrastructure_metadata_value(&metadata, "name"),
        resource_type: infrastructure_metadata_value(&metadata, "resource_type"),
        provider: infrastructure_metadata_value(&metadata, "provider"),
        image: infrastructure_metadata_value(&metadata, "image"),
        service_name: infrastructure_metadata_value(&metadata, "service_name"),
        container_name: infrastructure_metadata_value(&metadata, "container_name"),
        namespace: infrastructure_metadata_value(&metadata, "namespace"),
        ports: metadata_list(&metadata, "ports"),
        env_keys: metadata_list(&metadata, "env_keys"),
        labels: metadata_list(&metadata, "labels"),
        selectors: metadata_list(&metadata, "selectors"),
        file_path: infrastructure_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        line_start: infrastructure_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_start),
        line_end: infrastructure_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(line_end),
        confidence: infrastructure_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0),
        source_kind: infrastructure_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn wpf_from_row(row: &Row<'_>) -> rusqlite::Result<StoredWpf> {
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let branch_id: String = row.get(2)?;
    let fallback_name: String = row.get(3)?;
    let symbol_id: String = row.get(4)?;
    let file_path: String = row.get(5)?;
    let fallback_line_start: usize = row.get(6)?;
    let fallback_line_end: usize = row.get(7)?;
    let metadata: String = row.get(8)?;

    Ok(StoredWpf {
        id,
        project_id,
        branch_id,
        technology: wpf_metadata_value(&metadata, "technology")
            .unwrap_or_else(|| "wpf".to_string()),
        kind: wpf_metadata_value(&metadata, "kind").unwrap_or_else(|| "Unknown".to_string()),
        name: wpf_metadata_value(&metadata, "name").or(Some(fallback_name)),
        x_class: wpf_metadata_value(&metadata, "x_class"),
        code_behind: wpf_metadata_value(&metadata, "code_behind"),
        view_model: wpf_metadata_value(&metadata, "view_model"),
        binding_paths: wpf_metadata_value(&metadata, "binding_paths")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        command_bindings: wpf_metadata_value(&metadata, "command_bindings")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        resource_keys: wpf_metadata_value(&metadata, "resource_keys")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        resource_sources: wpf_metadata_value(&metadata, "resource_sources")
            .map(|value| split_metadata_list(&value))
            .unwrap_or_default(),
        data_context: wpf_metadata_value(&metadata, "data_context"),
        file_path: wpf_metadata_value(&metadata, "file").unwrap_or(file_path),
        symbol_id,
        line_start: wpf_metadata_value(&metadata, "line_start")
            .and_then(|value| value.parse().ok())
            .unwrap_or(fallback_line_start),
        line_end: wpf_metadata_value(&metadata, "line_end")
            .and_then(|value| value.parse().ok())
            .unwrap_or(fallback_line_end),
        confidence: wpf_metadata_value(&metadata, "confidence")
            .and_then(|value| value.parse().ok())
            .unwrap_or(7000),
        source_kind: wpf_metadata_value(&metadata, "source")
            .unwrap_or_else(|| "WpfMetadata".to_string()),
    })
}

fn route_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("route.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

fn infrastructure_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("infrastructure.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

fn wpf_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("wpf.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.trim().to_string())
    })
}

fn metadata_list(metadata: &str, key: &str) -> Vec<String> {
    infrastructure_metadata_value(metadata, key)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn component_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("component.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";"))
    })
}

fn data_access_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("data_access.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

fn realtime_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("realtime.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

fn messaging_metadata_value(metadata: &str, key: &str) -> Option<String> {
    let full_key = format!("messaging.{key}=");
    metadata.split(';').find_map(|part| {
        part.strip_prefix(&full_key)
            .map(|value| value.replace("%3B", ";").replace("\\n", "\n"))
    })
}

fn split_metadata_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
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

fn centrality_metric_from_row(row: &Row<'_>) -> rusqlite::Result<CentralityMetric> {
    Ok(CentralityMetric {
        symbol_id: row.get(0)?,
        in_degree: row.get::<_, i64>(1)? as usize,
        out_degree: row.get::<_, i64>(2)? as usize,
        fan_in: row.get::<_, i64>(3)? as usize,
        fan_out: row.get::<_, i64>(4)? as usize,
        degree_centrality: row.get(5)?,
        pagerank_score: row.get(6)?,
        component_size: row.get::<_, Option<i64>>(7)?.map(|value| value as usize),
        is_cycle_member: row.get::<_, i64>(8)? != 0,
        algorithm_version: row.get(9)?,
        calculated_at_unix_ms: row.get::<_, i64>(10)? as u64,
    })
}

fn parse_failure_from_row(row: &Row<'_>) -> rusqlite::Result<StoredParseFailure> {
    Ok(StoredParseFailure {
        failure_id: row.get(0)?,
        project_id: row.get(1)?,
        branch_id: row.get(2)?,
        file_id: row.get(3)?,
        file_path: row.get(4)?,
        file_hash: row.get(5)?,
        language: row.get(6)?,
        error_kind: row.get(7)?,
        error_message: row.get(8)?,
        stderr_excerpt: row.get(9)?,
        failed_at_unix_ms: row.get::<_, i64>(10)? as u64,
        retry_count: row.get::<_, i64>(11)? as usize,
    })
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
        assert!(storage
            .table_exists("parse_failures")
            .expect("parse failures"));
        assert!(storage.migration_applied(1).expect("migration"));
        assert!(storage.migration_applied(3).expect("parse migration"));
        assert!(storage.index_exists("idx_edges_from").expect("edge index"));
        assert!(storage
            .index_exists("idx_parse_failures_scope")
            .expect("parse failure scope index"));
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

    #[test]
    fn routes_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
            file: FileRecord {
                id: FileId::new("file"),
                project_id: project_id.clone(),
                path: "src/server.ts".to_string(),
                content_hash: "hash".to_string(),
            },
            language: Some("typescript".to_string()),
            size_bytes: 32,
            content: "app.get('/users', listUsers);".to_string(),
            symbols: vec![SymbolRecord {
                id: SymbolId::new("route-symbol"),
                file_id: FileId::new("file"),
                name: "GET /users".to_string(),
                kind: NodeKind::Route,
                start_byte: 0,
                end_byte: 28,
                start_line: 1,
                start_column: 0,
                end_line: 1,
                end_column: 28,
                visibility: Some("route.framework=express;route.method=GET;route.path=/users;route.file=src/server.ts;route.handler=listUsers;route.function=listUsers;route.source=ExpressCall;route.line_start=1;route.line_end=1;route.confidence=9500".to_string()),
            }],
            edges: Vec::new(),
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first route upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second route upsert");
        let routes = storage
            .routes(
                "project",
                "main",
                Some("express"),
                Some("GET"),
                Some("/users"),
                10,
            )
            .expect("routes");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].handler_name.as_deref(), Some("listUsers"));

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .routes("project", "main", None, None, None, 10)
            .expect("routes after cleanup")
            .is_empty());
    }

    #[test]
    fn components_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
            file: FileRecord {
                id: FileId::new("component-file"),
                project_id: project_id.clone(),
                path: "src/ProductCard.tsx".to_string(),
                content_hash: "hash".to_string(),
            },
            language: Some("tsx".to_string()),
            size_bytes: 32,
            content: "export function ProductCard() { return <div />; }".to_string(),
            symbols: vec![SymbolRecord {
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
                visibility: Some("export;component.framework=react;component.export=named;component.kind=function;component.props=ProductCardProps;component.source=FunctionDeclaration;component.hooks=useState,useEffect;component.usages=Badge;component.line_start=1;component.line_end=1;component.confidence=9500".to_string()),
            }],
            edges: Vec::new(),
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first component upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second component upsert");
        let components = storage
            .components(
                "project",
                "main",
                Some("react"),
                Some("ProductCard"),
                Some("src/ProductCard.tsx"),
                10,
            )
            .expect("components");
        assert_eq!(components.len(), 1);
        assert_eq!(
            components[0].props_type_name.as_deref(),
            Some("ProductCardProps")
        );
        assert_eq!(components[0].hooks, vec!["useState", "useEffect"]);
        assert_eq!(components[0].usages, vec!["Badge"]);

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .components("project", "main", None, None, None, 10)
            .expect("components after cleanup")
            .is_empty());
    }

    #[test]
    fn data_access_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
            file: FileRecord {
                id: FileId::new("data-file"),
                project_id: project_id.clone(),
                path: "src/data.ts".to_string(),
                content_hash: "hash".to_string(),
            },
            language: Some("typescript".to_string()),
            size_bytes: 32,
            content: "prisma.user.findMany();".to_string(),
            symbols: vec![SymbolRecord {
                id: SymbolId::new("data-access-symbol"),
                file_id: FileId::new("data-file"),
                name: "Prisma read user".to_string(),
                kind: NodeKind::Endpoint,
                start_byte: 0,
                end_byte: 23,
                start_line: 1,
                start_column: 0,
                end_line: 1,
                end_column: 23,
                visibility: Some("data_access.technology=prisma;data_access.kind=QueryCall;data_access.operation=read;data_access.file=src/data.ts;data_access.entity=user;data_access.method=listUsers;data_access.source=PrismaClientCall;data_access.line_start=1;data_access.line_end=1;data_access.confidence=8500".to_string()),
            }],
            edges: Vec::new(),
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first data access upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second data access upsert");
        let records = storage
            .data_access(
                "project",
                "main",
                Some("prisma"),
                Some("QueryCall"),
                Some("read"),
                Some("src/data.ts"),
                10,
            )
            .expect("data access");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].entity_name.as_deref(), Some("user"));
        assert_eq!(records[0].method_name.as_deref(), Some("listUsers"));

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .data_access("project", "main", None, None, None, None, 10)
            .expect("data access after cleanup")
            .is_empty());
    }

    #[test]
    fn realtime_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
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
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first realtime upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second realtime upsert");
        let records = storage
            .realtime(
                "project",
                "main",
                Some("socketio"),
                Some("listener"),
                Some("message"),
                Some("src/socket.ts"),
                10,
            )
            .expect("realtime");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].event_name.as_deref(), Some("message"));
        assert_eq!(records[0].function_name.as_deref(), Some("connect"));

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .realtime("project", "main", None, None, None, None, 10)
            .expect("realtime after cleanup")
            .is_empty());
    }

    #[test]
    fn messaging_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
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
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first messaging upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second messaging upsert");
        let records = storage
            .messaging(
                "project",
                "main",
                Some("kafka"),
                Some("producer"),
                Some("orders"),
                Some("orders.queue"),
                Some("order.created"),
                10,
            )
            .expect("messaging");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].topic.as_deref(), Some("orders"));
        assert_eq!(records[0].consumer_group.as_deref(), Some("orders-workers"));

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .messaging("project", "main", None, None, None, None, None, 10)
            .expect("messaging after cleanup")
            .is_empty());
    }

    #[test]
    fn infrastructure_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
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
                end_line: 12,
                end_column: 0,
                visibility: Some("infrastructure.technology=kubernetes;infrastructure.kind=Deployment;infrastructure.name=api;infrastructure.resource_type=Deployment;infrastructure.provider=google;infrastructure.image=my-api:latest;infrastructure.service_name=api;infrastructure.container_name=api;infrastructure.namespace=default;infrastructure.ports=8080;infrastructure.env_keys=NODE_ENV;infrastructure.labels=app=api;infrastructure.selectors=app=api;infrastructure.file=deploy/k8s.yaml;infrastructure.source=KubernetesDeployment;infrastructure.line_start=1;infrastructure.line_end=12;infrastructure.confidence=9000".to_string()),
            }],
            edges: Vec::new(),
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first infrastructure upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second infrastructure upsert");
        let records = storage
            .infrastructure(
                "project",
                "main",
                Some("kubernetes"),
                Some("deployment"),
                Some("api"),
                10,
            )
            .expect("infrastructure");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].image.as_deref(), Some("my-api:latest"));
        assert_eq!(records[0].ports, vec!["8080"]);

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .infrastructure("project", "main", None, None, None, 10)
            .expect("infrastructure after cleanup")
            .is_empty());
    }

    #[test]
    fn wpf_round_trip_without_duplicates_and_cleanup_with_files() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("project branch");
        let indexed = IndexedFileRecord {
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
        };

        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed.clone())
            .expect("first wpf upsert");
        storage
            .upsert_indexed_file(&project_id, &branch_id, indexed)
            .expect("second wpf upsert");
        let records = storage
            .wpf(
                "project",
                "main",
                Some("window"),
                Some("UserName"),
                Some("SaveCommand"),
                10,
            )
            .expect("wpf");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].x_class.as_deref(), Some("App.Views.MainWindow"));
        assert_eq!(
            records[0].code_behind.as_deref(),
            Some("Views/MainWindow.xaml.cs")
        );
        assert_eq!(records[0].binding_paths, vec!["UserName", "SelectedUser"]);
        assert_eq!(records[0].command_bindings, vec!["SaveCommand"]);
        assert_eq!(records[0].resource_keys, vec!["PrimaryBrush"]);
        assert_eq!(records[0].resource_sources, vec!["Themes/Colors.xaml"]);

        storage
            .cleanup_deleted_files(&project_id, &branch_id, &[])
            .expect("cleanup deleted file");
        assert!(storage
            .wpf("project", "main", None, None, None, 10)
            .expect("wpf after cleanup")
            .is_empty());
    }

    #[test]
    fn parse_failures_round_trip_records() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
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
                project_id: project_id.clone(),
                branch_id: branch_id.clone(),
                file_id: FileId::new("file"),
                file_path: "src/lib.rs".to_string(),
                file_hash: "hash".to_string(),
                language: Some("rs".to_string()),
                error_kind: "timeout".to_string(),
                error_message: "parser timed out".to_string(),
                stderr_excerpt: Some("stderr".to_string()),
                failed_at_unix_ms: 7,
                retry_count: 1,
            })
            .expect("record failure");

        assert_eq!(
            storage
                .parse_failure_count(Some("project"), Some("main"))
                .expect("count"),
            1
        );
        let failures = storage.recent_parse_failures(5).expect("recent");
        assert_eq!(failures[0].file_path, "src/lib.rs");
        assert_eq!(failures[0].retry_count, 1);
    }

    #[test]
    fn shared_index_store_wraps_sqlite_index_contract() {
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        let file_id = FileId::new("file");
        let store = SharedSqliteIndexStore::new(
            SqliteStorage::open_in_memory().expect("open sqlite storage"),
        );

        store
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("ensure branch");
        store
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: file_id.clone(),
                        project_id: project_id.clone(),
                        path: "src/lib.rs".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("rust".to_string()),
                    size_bytes: 16,
                    content: "pub fn run() {}\n".to_string(),
                    symbols: Vec::new(),
                    edges: Vec::new(),
                },
            )
            .expect("upsert indexed file");

        assert_eq!(
            store
                .existing_file(&file_id)
                .expect("existing file")
                .expect("file")
                .path,
            "src/lib.rs"
        );
        store
            .remove_file(&project_id, &branch_id, "src/lib.rs")
            .expect("remove file");
        assert!(store
            .existing_file(&file_id)
            .expect("existing file after delete")
            .is_none());
    }

    #[test]
    fn vector_documents_and_vectors_upsert_without_duplicates() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        let file_id = FileId::new("file");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: file_id.clone(),
                        project_id: project_id.clone(),
                        path: "src/lib.rs".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("rust".to_string()),
                    size_bytes: 16,
                    content: "pub fn run() {}\n".to_string(),
                    symbols: Vec::new(),
                    edges: Vec::new(),
                },
            )
            .expect("file");
        let document = VectorDocument::new(b3_core::VectorDocumentInput {
            project_id: project_id.clone(),
            branch_id: branch_id.clone(),
            file_id: file_id.clone(),
            symbol_id: None,
            language: Some("rust".to_string()),
            framework: None,
            source_kind: SourceKind::FileChunk,
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
            chunk_index: 0,
            text: "pub fn run() {}".to_string(),
            start_line: 1,
            end_line: 1,
            metadata: BTreeMap::from([("kind".to_string(), "file".to_string())]),
        });
        let vector = EmbeddingVector::new(
            document.id.clone(),
            "deterministic-test",
            3,
            vec![1.0, 0.0, 0.0],
            10,
        );

        storage
            .upsert_documents(&[document.clone()])
            .expect("documents");
        storage
            .upsert_documents(&[document.clone()])
            .expect("documents second");
        storage.upsert_vectors(&[vector]).expect("vectors");

        let stats = storage.vector_stats().expect("stats");
        assert_eq!(stats.documents, 1);
        assert_eq!(stats.vectors, 1);
        assert_eq!(
            storage
                .get_document(&document.id)
                .expect("get")
                .expect("document")
                .metadata["kind"],
            "file"
        );
    }

    #[test]
    fn vector_search_and_cleanup_are_local_sqlite_only() {
        let storage = SqliteStorage::open_in_memory().expect("open sqlite storage");
        let project_id = ProjectId::new("project");
        let branch_id = BranchId::new("main");
        let file_id = FileId::new("file");
        storage
            .ensure_project_branch(&project_id, &branch_id, ".")
            .expect("branch");
        storage
            .upsert_indexed_file(
                &project_id,
                &branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: file_id.clone(),
                        project_id: project_id.clone(),
                        path: "src/lib.rs".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("rust".to_string()),
                    size_bytes: 16,
                    content: "pub fn run() {}\n".to_string(),
                    symbols: Vec::new(),
                    edges: Vec::new(),
                },
            )
            .expect("file");
        let first = VectorDocument::new(b3_core::VectorDocumentInput {
            project_id: project_id.clone(),
            branch_id: branch_id.clone(),
            file_id: file_id.clone(),
            symbol_id: None,
            language: Some("rust".to_string()),
            framework: None,
            source_kind: SourceKind::FileChunk,
            path: "src/lib.rs".to_string(),
            content_hash: "hash".to_string(),
            chunk_index: 0,
            text: "alpha".to_string(),
            start_line: 1,
            end_line: 1,
            metadata: BTreeMap::new(),
        });
        let second = VectorDocument::new(b3_core::VectorDocumentInput {
            chunk_index: 1,
            text: "beta".to_string(),
            ..b3_core::VectorDocumentInput {
                project_id: project_id.clone(),
                branch_id: branch_id.clone(),
                file_id: file_id.clone(),
                symbol_id: None,
                language: Some("rust".to_string()),
                framework: None,
                source_kind: SourceKind::FileChunk,
                path: "src/lib.rs".to_string(),
                content_hash: "hash".to_string(),
                chunk_index: 0,
                text: String::new(),
                start_line: 2,
                end_line: 2,
                metadata: BTreeMap::new(),
            }
        });
        storage
            .upsert_documents(&[first.clone(), second.clone()])
            .expect("documents");
        storage
            .upsert_vectors(&[
                EmbeddingVector::new(first.id.clone(), "test", 3, vec![1.0, 0.0, 0.0], 0),
                EmbeddingVector::new(second.id.clone(), "test", 3, vec![0.0, 1.0, 0.0], 0),
            ])
            .expect("vectors");

        let hits = storage
            .search(VectorSearchRequest {
                query_vector: vec![1.0, 0.0, 0.0],
                project_id: project_id.clone(),
                branch_id: branch_id.clone(),
                language: Some("rust".to_string()),
                framework: None,
                source_kind: Some(SourceKind::FileChunk),
                limit: 1,
                min_score: Some(0.5),
            })
            .expect("search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document.id, first.id);
        assert_eq!(
            storage
                .delete_by_file(&project_id, &branch_id, &file_id)
                .expect("delete"),
            2
        );
        assert_eq!(storage.vector_stats().expect("stats").documents, 0);
    }
}
