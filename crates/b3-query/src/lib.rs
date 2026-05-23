//! Hybrid query and graph retrieval boundary.
//!
//! Query code should orchestrate exact lookup, FTS/BM25, vector search, graph
//! expansion, ranking, and context packing through core traits. It should not
//! own storage clients, embedding workers, or MCP request handling.

pub use b3_core::{QueryEngine, QueryRequest, QueryResult, RankingWeights, RetrievalConfig};
