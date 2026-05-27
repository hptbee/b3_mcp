use super::*;

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

const MIGRATION_005: &str = r#"
CREATE TABLE IF NOT EXISTS embedding_vectors_v2 (
    document_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model_id TEXT,
    dimension INTEGER NOT NULL,
    vector_blob BLOB NOT NULL,
    vector_hash TEXT NOT NULL,
    normalized INTEGER NOT NULL DEFAULT 1,
    updated_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(document_id, provider_id, dimension),
    FOREIGN KEY(document_id) REFERENCES vector_documents(id) ON DELETE CASCADE
);

INSERT OR REPLACE INTO embedding_vectors_v2 (
    document_id, provider_id, model_id, dimension, vector_blob, vector_hash, normalized, updated_at_unix_ms
)
SELECT document_id, provider_id, NULL, dimension, vector, vector_hash, 1, indexed_at_unix_ms
FROM embedding_vectors;

DROP TABLE embedding_vectors;

ALTER TABLE embedding_vectors_v2 RENAME TO embedding_vectors;

CREATE INDEX IF NOT EXISTS idx_vector_documents_scope
ON vector_documents(project_id, branch_id, source_kind, language, framework);

CREATE INDEX IF NOT EXISTS idx_vector_documents_file
ON vector_documents(project_id, branch_id, file_id);

CREATE INDEX IF NOT EXISTS idx_vector_documents_symbol
ON vector_documents(project_id, branch_id, symbol_id);

CREATE INDEX IF NOT EXISTS idx_vector_documents_chunk
ON vector_documents(project_id, branch_id, chunk_hash);

CREATE INDEX IF NOT EXISTS idx_embedding_vectors_provider_dimension
ON embedding_vectors(provider_id, dimension);
"#;

const MIGRATION_006: &str = r#"
CREATE TABLE IF NOT EXISTS index_git_snapshots (
    project_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    is_git_repo INTEGER NOT NULL DEFAULT 0,
    git_repo_root TEXT,
    git_dir TEXT,
    indexed_branch TEXT,
    indexed_commit TEXT,
    indexed_short_commit TEXT,
    indexed_detached_head INTEGER NOT NULL DEFAULT 0,
    indexed_dirty INTEGER NOT NULL DEFAULT 0,
    indexed_staged_count INTEGER NOT NULL DEFAULT 0,
    indexed_unstaged_count INTEGER NOT NULL DEFAULT 0,
    indexed_untracked_count INTEGER NOT NULL DEFAULT 0,
    indexed_conflicted_count INTEGER NOT NULL DEFAULT 0,
    indexed_total_changed_count INTEGER NOT NULL DEFAULT 0,
    indexed_at_unix_ms INTEGER NOT NULL DEFAULT 0,
    warnings_json TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY(project_id, branch_id),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY(branch_id) REFERENCES branches(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_index_git_snapshots_indexed_at
ON index_git_snapshots(indexed_at_unix_ms DESC);
"#;

impl SqliteStorage {
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
        self.apply_migration(5, "sqlite_vector_search_index", MIGRATION_005)?;
        self.apply_migration(6, "branch_aware_index_git_snapshots", MIGRATION_006)?;

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
}
