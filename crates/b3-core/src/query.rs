//! Query and retrieval domain contracts.

use crate::{
    BranchId, EdgeConfidence, EdgeId, EdgeKind, EdgeProvenance, FileId, NodeKind, ProjectId,
    SymbolId, ToolCallId,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryIntent {
    SymbolLookup,
    CodeSearch,
    CallerLookup,
    CalleeLookup,
    DependencyTrace,
    ImpactAnalysis,
    ContextPack,
    TestSearch,
    Explanation,
}

impl QueryIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SymbolLookup => "symbol_lookup",
            Self::CodeSearch => "code_search",
            Self::CallerLookup => "caller_lookup",
            Self::CalleeLookup => "callee_lookup",
            Self::DependencyTrace => "dependency_trace",
            Self::ImpactAnalysis => "impact_analysis",
            Self::ContextPack => "context_pack",
            Self::TestSearch => "test_search",
            Self::Explanation => "explanation",
        }
    }
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
    pub source_provenance: String,
    pub estimated_tokens: usize,
    pub expansion_handle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPack {
    pub items: Vec<ContextItem>,
    pub returned_tokens: usize,
    pub token_budget: usize,
    pub skipped_items: Vec<String>,
    pub truncation_reason: Option<String>,
    pub expansion_handles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySavingsEstimate {
    pub tool_call_id: Option<ToolCallId>,
    pub returned_tokens: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CentralityMetric {
    pub symbol_id: String,
    pub in_degree: usize,
    pub out_degree: usize,
    pub fan_in: usize,
    pub fan_out: usize,
    pub degree_centrality: f64,
    pub pagerank_score: f64,
    pub component_size: Option<usize>,
    pub is_cycle_member: bool,
    pub algorithm_version: String,
    pub calculated_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CentralitySnapshot {
    pub project_id: String,
    pub branch_id: String,
    pub algorithm_version: String,
    pub calculated_at_unix_ms: u64,
    pub metrics: Vec<CentralityMetric>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryTraceDto {
    pub trace_id: String,
    pub query_input: String,
    pub query_intent: String,
    pub project_id: String,
    pub branch_id: String,
    pub exact_symbol_hits: Vec<String>,
    pub fts_hits: Vec<String>,
    pub graph_traversal_steps: Vec<String>,
    pub ranking_decisions: Vec<String>,
    pub context_items_selected: Vec<String>,
    pub context_items_skipped: Vec<String>,
    pub truncation_reason: Option<String>,
    pub token_budget_used: usize,
    pub token_budget: usize,
    pub token_savings_estimate: Option<QuerySavingsEstimateDto>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySavingsEstimateDto {
    pub returned_tokens: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
    pub estimated_tokens_saved: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolDto {
    pub symbol_id: String,
    pub file_id: String,
    pub name: String,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub visibility: Option<String>,
    pub score: i64,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItemDto {
    pub file_id: String,
    pub symbol_id: Option<String>,
    pub title: String,
    pub snippet: String,
    pub why: String,
    pub source_provenance: String,
    pub estimated_tokens: usize,
    pub expansion_handle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPackResponse {
    pub items: Vec<ContextItemDto>,
    pub returned_tokens: usize,
    pub token_budget: usize,
    pub skipped_items: Vec<String>,
    pub truncation_reason: Option<String>,
    pub expansion_handles: Vec<String>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindSymbolResponse {
    pub symbols: Vec<SymbolDto>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchCodeResponse {
    pub symbols: Vec<SymbolDto>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversalStepDto {
    pub symbol: SymbolDto,
    pub edge_id: String,
    pub edge_kind: String,
    pub direction: String,
    pub distance: usize,
    pub confidence_bps: u16,
    pub provenance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindCallersResponse {
    pub callers: Vec<TraversalStepDto>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindCalleesResponse {
    pub callees: Vec<TraversalStepDto>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedSymbolsResponse {
    pub related: Vec<TraversalStepDto>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactRiskSignalDto {
    pub name: String,
    pub value: i64,
    pub weight: i64,
    pub contribution: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedTestDto {
    pub symbol: SymbolDto,
    pub confidence_bps: u16,
    pub relation: String,
    pub direct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAnalysisResponse {
    pub impacted: Vec<TraversalStepDto>,
    pub risk_score: u16,
    pub risk_level: ImpactRiskLevel,
    pub risk_reasons: Vec<String>,
    pub risk_signals: Vec<ImpactRiskSignalDto>,
    pub impacted_symbols: Vec<SymbolDto>,
    pub impacted_files: Vec<String>,
    pub related_tests: Vec<RelatedTestDto>,
    pub missing_tests: bool,
    pub dependency_paths: Vec<Vec<String>>,
    pub cycles_involved: Vec<Vec<String>>,
    pub trace_id: String,
    pub trace: Option<QueryTraceDto>,
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
