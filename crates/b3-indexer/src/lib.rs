//! Indexing boundary.
//!
//! This crate will own repository discovery, ignore filtering, language
//! detection, file hashing, tree-sitter parsing, relationship extraction, graph
//! updates, FTS updates, and embedding queue handoff. Indexing must run outside
//! the MCP hot path and use subprocess parser isolation for risky parsing.

use b3_core::{BranchMetadata, ProjectId};

pub use b3_core::{IndexJob, IndexJobQueue, IndexSummary, Indexer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRequest {
    pub project_id: ProjectId,
    pub root_path: String,
    pub branch: BranchMetadata,
    pub parser_isolation: ParserIsolation,
}

impl IndexRequest {
    pub fn new(project_id: ProjectId, root_path: impl Into<String>, branch: BranchMetadata) -> Self {
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
