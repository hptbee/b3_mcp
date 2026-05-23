//! Query and retrieval domain contracts.

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
