//! Shared domain contracts for the local-first MCP code intelligence platform.
//!
//! This crate owns stable IDs, configuration models, event contracts, graph
//! metadata, query models, and service traits. It must stay implementation-light
//! so MCP, storage, indexing, query, embedding, control, and UI crates can share
//! contracts without inheriting each other's concrete dependencies.

mod architecture;
mod config;
mod contracts;
mod editing;
mod embedding;
mod events;
mod git;
mod ids;
mod language;
mod plugin;
mod query;
mod refactor;
mod scope;
mod vector;

pub use architecture::*;
pub use config::*;
pub use contracts::*;
pub use editing::*;
pub use embedding::*;
pub use events::*;
pub use git::*;
pub use ids::*;
pub use language::*;
pub use plugin::*;
pub use query::*;
pub use refactor::*;
pub use scope::*;
pub use vector::*;

pub const PRODUCT_NAME: &str = "b3_mcp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Project,
    File,
    Module,
    Namespace,
    Class,
    Struct,
    Interface,
    Enum,
    Function,
    Method,
    Variable,
    Route,
    Endpoint,
    ConfigKey,
    Test,
    Package,
    Decision,
    CodeArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Contains,
    Imports,
    Calls,
    References,
    Implements,
    Inherits,
    DependsOn,
    Tests,
    RoutesTo,
    ReadsConfig,
    WritesConfig,
    SimilarTo,
    Touches,
    Decides,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeProvenance {
    Ast,
    ImportAnalysis,
    TextHeuristic,
    SemanticSimilarity,
    UserRecorded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeConfidence(u16);

impl EdgeConfidence {
    pub const MAX_BASIS_POINTS: u16 = 10_000;

    pub fn from_basis_points(value: u16) -> Self {
        Self(value.min(Self::MAX_BASIS_POINTS))
    }

    pub fn basis_points(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdgeMetadata {
    pub confidence: EdgeConfidence,
    pub provenance: EdgeProvenance,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchMetadata {
    pub branch_name: String,
    pub commit_hash: Option<String>,
    pub worktree_dirty: bool,
}

impl BranchMetadata {
    pub fn new(branch_name: impl Into<String>) -> Self {
        Self {
            branch_name: branch_name.into(),
            commit_hash: None,
            worktree_dirty: false,
        }
    }
}
