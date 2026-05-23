//! Stable service trait contracts.
//!
//! These traits keep later storage, indexing, query, embedding, MCP, and UI
//! modules from depending on concrete implementations too early.

use crate::{
    AppConfig, BranchId, DomainEvent, EdgeId, EdgeKind, EmbeddingConfig, FileId, FtsSearchHit,
    GraphDirection, GraphEdgeMetadata, GraphNeighbor, NodeId, NodeKind, ProjectId, QueryFile,
    QueryRequest, QueryResult, QueryScope, QuerySymbol, SymbolId, ToolCallId,
};
use crate::{CentralityMetric, CentralitySnapshot};

pub type ContractResult<T> = Result<T, ContractError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractError {
    pub message: String,
}

impl ContractError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub trait StorageProvider {
    fn name(&self) -> &str;
    fn is_local_only(&self) -> bool;
}

pub trait GraphRepository {
    fn get_node(&self, node_id: &NodeId) -> ContractResult<Option<GraphNode>>;
    fn get_edge(&self, edge_id: &EdgeId) -> ContractResult<Option<GraphEdge>>;
}

pub trait SymbolRepository {
    fn find_symbol(&self, project_id: &ProjectId, name: &str) -> ContractResult<Vec<SymbolRecord>>;
}

pub trait FileRepository {
    fn get_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>>;
}

pub trait TokenSavingsRepository {
    fn record_savings(&self, record: TokenSavingsRecord) -> ContractResult<()>;
}

pub trait IndexJobQueue {
    fn enqueue(&self, job: IndexJob) -> ContractResult<IndexJobId>;
}

pub trait Indexer {
    fn index(&self, job: IndexJob) -> ContractResult<IndexSummary>;
}

pub trait IndexStore {
    fn ensure_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        root_path: &str,
    ) -> ContractResult<()>;

    fn existing_file(&self, file_id: &FileId) -> ContractResult<Option<FileRecord>>;

    fn cleanup_deleted_files(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        live_file_ids: &[FileId],
    ) -> ContractResult<()>;

    fn upsert_indexed_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file: IndexedFileRecord,
    ) -> ContractResult<()>;

    fn remove_file(
        &self,
        _project_id: &ProjectId,
        _branch_id: &BranchId,
        _path: &str,
    ) -> ContractResult<()> {
        Ok(())
    }
}

pub trait QueryEngine {
    fn execute(&self, request: QueryRequest) -> ContractResult<QueryResult>;
}

pub trait QueryRepository {
    fn list_symbols(&self, scope: &QueryScope, limit: usize) -> ContractResult<Vec<QuerySymbol>>;
    fn find_symbols(&self, scope: &QueryScope, name: &str) -> ContractResult<Vec<QuerySymbol>>;
    fn get_symbol(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Option<QuerySymbol>>;
    fn get_file(&self, scope: &QueryScope, file_id: &FileId) -> ContractResult<Option<QueryFile>>;
    fn fts_search(
        &self,
        scope: &QueryScope,
        query: &str,
        limit: usize,
    ) -> ContractResult<Vec<FtsSearchHit>>;
    fn graph_neighbors(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
        direction: GraphDirection,
        edge_filter: &[EdgeKind],
        min_confidence: u16,
    ) -> ContractResult<Vec<GraphNeighbor>>;
}

pub trait CentralityRepository {
    fn get_centrality_metric(
        &self,
        scope: &QueryScope,
        symbol_id: &SymbolId,
    ) -> ContractResult<Option<CentralityMetric>>;

    fn upsert_centrality_snapshot(
        &self,
        scope: &QueryScope,
        snapshot: CentralitySnapshot,
    ) -> ContractResult<()>;
}

pub trait EmbeddingProvider {
    fn config(&self) -> &EmbeddingConfig;
    fn embed(&self, input: EmbeddingRequest) -> ContractResult<EmbeddingResult>;
}

pub trait VectorStore {
    fn upsert(&self, record: VectorRecord) -> ContractResult<()>;
    fn search(&self, query: VectorSearchRequest) -> ContractResult<Vec<VectorSearchHit>>;
}

pub trait ConfigProvider {
    fn load(&self) -> ContractResult<AppConfig>;
}

pub trait EventBus {
    fn publish(&self, event: DomainEvent) -> ContractResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    pub id: NodeId,
    pub project_id: ProjectId,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub metadata: GraphEdgeMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecord {
    pub id: SymbolId,
    pub file_id: FileId,
    pub name: String,
    pub kind: NodeKind,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub visibility: Option<String>,
}

impl SymbolRecord {
    pub fn new(id: SymbolId, file_id: FileId, name: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id,
            file_id,
            name: name.into(),
            kind,
            start_byte: 0,
            end_byte: 0,
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
            visibility: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub id: FileId,
    pub project_id: ProjectId,
    pub path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedFileRecord {
    pub file: FileRecord,
    pub language: Option<String>,
    pub size_bytes: u64,
    pub content: String,
    pub symbols: Vec<SymbolRecord>,
    pub edges: Vec<IndexedEdgeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedEdgeRecord {
    pub id: EdgeId,
    pub from_symbol: SymbolId,
    pub to_symbol: SymbolId,
    pub kind: EdgeKind,
    pub metadata: GraphEdgeMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSavingsRecord {
    pub tool_call_id: Option<ToolCallId>,
    pub estimated_tokens_saved: usize,
    pub returned_tokens: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexJobId(String);

impl IndexJobId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexJob {
    pub project_id: ProjectId,
    pub root_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSummary {
    pub files_seen: usize,
    pub files_parsed: usize,
    pub symbols_indexed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingRequest {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingResult {
    pub dimensions: usize,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorRecord {
    pub id: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorSearchRequest {
    pub vector: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorSearchHit {
    pub id: String,
    pub score: f32,
}
