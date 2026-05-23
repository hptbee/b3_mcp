//! Stable service trait contracts.
//!
//! These traits keep later storage, indexing, query, embedding, MCP, and UI
//! modules from depending on concrete implementations too early.

use crate::{
    AppConfig, DomainEvent, EdgeId, EmbeddingConfig, FileId, GraphEdgeMetadata, NodeId, ProjectId,
    QueryRequest, QueryResult, SymbolId, ToolCallId,
};

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

pub trait QueryEngine {
    fn execute(&self, request: QueryRequest) -> ContractResult<QueryResult>;
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub id: FileId,
    pub project_id: ProjectId,
    pub path: String,
    pub content_hash: String,
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
