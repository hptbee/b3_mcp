//! Vector document, chunk, and store contracts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{BranchId, ContractError, ContractResult, FileId, ProjectId, SymbolId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    FileChunk,
    SymbolChunk,
    RouteChunk,
    ComponentChunk,
    DataAccessChunk,
    RealtimeChunk,
    MessagingChunk,
    InfrastructureChunk,
    WpfChunk,
    GoChunk,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileChunk => "FileChunk",
            Self::SymbolChunk => "SymbolChunk",
            Self::RouteChunk => "RouteChunk",
            Self::ComponentChunk => "ComponentChunk",
            Self::DataAccessChunk => "DataAccessChunk",
            Self::RealtimeChunk => "RealtimeChunk",
            Self::MessagingChunk => "MessagingChunk",
            Self::InfrastructureChunk => "InfrastructureChunk",
            Self::WpfChunk => "WpfChunk",
            Self::GoChunk => "GoChunk",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorDocument {
    pub id: String,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub symbol_id: Option<SymbolId>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub source_kind: SourceKind,
    pub path: String,
    pub content_hash: String,
    pub chunk_hash: String,
    pub chunk_index: usize,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub metadata: BTreeMap<String, String>,
}

impl VectorDocument {
    pub fn deterministic_id(
        project_id: &ProjectId,
        branch_id: &BranchId,
        file_id: &FileId,
        source_kind: SourceKind,
        chunk_hash: &str,
        chunk_index: usize,
    ) -> String {
        stable_hash(&[
            project_id.as_str(),
            branch_id.as_str(),
            file_id.as_str(),
            source_kind.as_str(),
            chunk_hash,
            &chunk_index.to_string(),
        ])
    }

    pub fn new(input: VectorDocumentInput) -> Self {
        let chunk_hash = stable_hash(&[
            input.content_hash.as_str(),
            input.source_kind.as_str(),
            &input.chunk_index.to_string(),
            input.text.as_str(),
        ]);
        let id = Self::deterministic_id(
            &input.project_id,
            &input.branch_id,
            &input.file_id,
            input.source_kind,
            &chunk_hash,
            input.chunk_index,
        );

        Self {
            id,
            project_id: input.project_id,
            branch_id: input.branch_id,
            file_id: input.file_id,
            symbol_id: input.symbol_id,
            language: input.language,
            framework: input.framework,
            source_kind: input.source_kind,
            path: input.path,
            content_hash: input.content_hash,
            chunk_hash,
            chunk_index: input.chunk_index,
            text: input.text,
            start_line: input.start_line,
            end_line: input.end_line,
            metadata: input.metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorDocumentInput {
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub file_id: FileId,
    pub symbol_id: Option<SymbolId>,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub source_kind: SourceKind,
    pub path: String,
    pub content_hash: String,
    pub chunk_index: usize,
    pub text: String,
    pub start_line: usize,
    pub end_line: usize,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingVector {
    pub document_id: String,
    pub provider_id: String,
    pub dimension: usize,
    pub vector: Vec<f32>,
    pub vector_hash: String,
    pub indexed_at_unix_ms: u64,
}

impl EmbeddingVector {
    pub fn new(
        document_id: impl Into<String>,
        provider_id: impl Into<String>,
        dimension: usize,
        vector: Vec<f32>,
        indexed_at_unix_ms: u64,
    ) -> Self {
        let vector_hash = hash_f32_vector(&vector);
        Self {
            document_id: document_id.into(),
            provider_id: provider_id.into(),
            dimension,
            vector,
            vector_hash,
            indexed_at_unix_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorSearchRequest {
    pub query_vector: Vec<f32>,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub source_kind: Option<SourceKind>,
    pub limit: usize,
    pub min_score: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorSearchHit {
    pub document: VectorDocument,
    pub score: f32,
    pub distance: f32,
    pub provider_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VectorStoreStats {
    pub documents: usize,
    pub vectors: usize,
}

pub trait VectorStore {
    fn upsert_documents(&self, documents: &[VectorDocument]) -> ContractResult<()>;
    fn upsert_vectors(&self, vectors: &[EmbeddingVector]) -> ContractResult<()>;
    fn delete_by_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        file_id: &FileId,
    ) -> ContractResult<usize>;
    fn delete_by_project_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> ContractResult<usize>;
    fn search(&self, request: VectorSearchRequest) -> ContractResult<Vec<VectorSearchHit>>;
    fn get_document(&self, document_id: &str) -> ContractResult<Option<VectorDocument>>;
    fn stats(&self) -> ContractResult<VectorStoreStats>;
}

pub fn hash_f32_vector(vector: &[f32]) -> String {
    let mut parts = Vec::with_capacity(vector.len());
    for value in vector {
        parts.push(format!("{:08x}", value.to_bits()));
    }
    stable_hash(&parts.iter().map(String::as_str).collect::<Vec<_>>())
}

pub fn dot_product(left: &[f32], right: &[f32]) -> ContractResult<f32> {
    validate_dimension(left, right.len())?;
    Ok(left
        .iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum())
}

pub fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|value| value * value).sum::<f32>().sqrt()
}

pub fn normalize_l2(vector: &mut [f32]) -> f32 {
    let norm = l2_norm(vector);
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
    norm
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> ContractResult<f32> {
    validate_dimension(left, right.len())?;
    let left_norm = l2_norm(left);
    let right_norm = l2_norm(right);
    if left_norm == 0.0 || right_norm == 0.0 {
        return Ok(0.0);
    }
    Ok(dot_product(left, right)? / (left_norm * right_norm))
}

pub fn validate_dimension(vector: &[f32], expected: usize) -> ContractResult<()> {
    if vector.len() != expected {
        return Err(ContractError::new(format!(
            "vector dimension mismatch: expected {expected}, got {}",
            vector.len()
        )));
    }
    Ok(())
}

pub fn stable_hash(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for part in parts {
        for byte in part.as_bytes().iter().chain([0xff].iter()) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> VectorDocumentInput {
        VectorDocumentInput {
            project_id: ProjectId::new("project"),
            branch_id: BranchId::new("main"),
            file_id: FileId::new("file"),
            symbol_id: Some(SymbolId::new("symbol")),
            language: Some("rust".to_string()),
            framework: None,
            source_kind: SourceKind::SymbolChunk,
            path: "src/lib.rs".to_string(),
            content_hash: "content".to_string(),
            chunk_index: 0,
            text: text.to_string(),
            start_line: 1,
            end_line: 3,
            metadata: BTreeMap::from([("name".to_string(), "run".to_string())]),
        }
    }

    #[test]
    fn document_ids_and_hashes_are_deterministic() {
        let first = VectorDocument::new(input("pub fn run() {}"));
        let second = VectorDocument::new(input("pub fn run() {}"));

        assert_eq!(first.id, second.id);
        assert_eq!(first.chunk_hash, second.chunk_hash);
        assert_eq!(first.source_kind, SourceKind::SymbolChunk);
        assert_eq!(first.metadata.get("name").expect("name"), "run");
    }

    #[test]
    fn vector_hashes_are_deterministic_and_dimension_is_recorded() {
        let first = EmbeddingVector::new("doc", "test", 3, vec![0.1, 0.2, 0.3], 0);
        let second = EmbeddingVector::new("doc", "test", 3, vec![0.1, 0.2, 0.3], 0);

        assert_eq!(first.vector_hash, second.vector_hash);
        assert_eq!(first.dimension, 3);
    }

    #[test]
    fn vector_math_handles_normalization_and_cosine() {
        let mut vector = vec![3.0, 4.0];
        let norm = normalize_l2(&mut vector);

        assert_eq!(norm, 5.0);
        assert!((l2_norm(&vector) - 1.0).abs() < 0.0001);
        assert!((cosine_similarity(&vector, &vector).expect("cosine") - 1.0).abs() < 0.0001);
        assert_eq!(
            cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]).expect("zero cosine"),
            0.0
        );
        assert!(cosine_similarity(&[1.0], &[1.0, 0.0]).is_err());
    }
}
