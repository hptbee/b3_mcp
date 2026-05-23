//! Query and retrieval domain contracts.

use crate::{
    BranchId, EdgeConfidence, EdgeId, EdgeKind, EdgeProvenance, FileId, NodeKind, ProjectId,
    SymbolId, ToolCallId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub text: String,
    pub token_budget: usize,
}

impl QueryRequest {
    pub fn new(text: impl Into<String>, token_budget: usize) -> Self {
        Self {
            text: text.into(),
            token_budget,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub summary: String,
    pub returned_tokens: usize,
    pub expansion_handles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryScope {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
}

impl QueryScope {
    pub fn new(project_id: ProjectId, branch_id: BranchId) -> Self {
        Self {
            project_id,
            branch_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphDirection {
    Inbound,
    Outbound,
    Both,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryFile {
    pub id: FileId,
    pub path: String,
    pub content_hash: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySymbol {
    pub id: SymbolId,
    pub file_id: FileId,
    pub name: String,
    pub kind: NodeKind,
    pub snippet: String,
    pub start_line: usize,
    pub end_line: usize,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FtsSearchHit {
    pub file_id: FileId,
    pub symbol_id: Option<SymbolId>,
    pub path: String,
    pub name: Option<String>,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNeighbor {
    pub edge_id: EdgeId,
    pub from_symbol: Option<SymbolId>,
    pub to_symbol: Option<SymbolId>,
    pub edge_kind: EdgeKind,
    pub confidence: EdgeConfidence,
    pub provenance: EdgeProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    pub file_id: FileId,
    pub symbol_id: Option<SymbolId>,
    pub title: String,
    pub snippet: String,
    pub why: String,
    pub estimated_tokens: usize,
    pub expansion_handle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPack {
    pub items: Vec<ContextItem>,
    pub returned_tokens: usize,
    pub expansion_handles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySavingsEstimate {
    pub tool_call_id: Option<ToolCallId>,
    pub returned_tokens: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingWeights {
    pub exact_symbol: u16,
    pub lexical_bm25: u16,
    pub semantic_similarity: u16,
    pub graph_proximity: u16,
    pub recency: u16,
    pub centrality: u16,
    pub test_relevance: u16,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            exact_symbol: 100,
            lexical_bm25: 80,
            semantic_similarity: 70,
            graph_proximity: 60,
            recency: 20,
            centrality: 30,
            test_relevance: 30,
        }
    }
}
