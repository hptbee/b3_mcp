//! Storage boundary.
//!
//! This crate will host local SQLite/libSQL persistence, WAL setup, FTS5/BM25
//! indexes, graph tables, sessions, and token savings storage in later phases.
//! It must remain local-first and should implement the repository traits from
//! `b3-core` instead of leaking database details into query, indexing, MCP, or
//! UI crates.

use b3_core::{BranchMetadata, ProjectId};

pub use b3_core::{
    FileRepository, GraphRepository, StorageProvider, SymbolRepository, TokenSavingsRepository,
};

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
