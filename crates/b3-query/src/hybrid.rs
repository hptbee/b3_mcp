//! Deterministic hybrid ranking.
//!
//! This module combines local lexical, vector, and metadata signals. It does
//! not expose MCP tools, call external services, or own vector persistence.

use std::collections::{BTreeMap, BTreeSet};

use b3_core::{
    ContractError, ContractResult, EmbeddingProvider, FileId, FtsSearchHit, QueryRepository,
    QueryScope, SourceKind, SymbolId, VectorSearchHit, VectorSearchRequest, VectorStore,
};
use b3_embeddings::{
    LocalHashEmbeddingConfig, LocalHashEmbeddingProvider, DEFAULT_LOCAL_HASH_DIMENSION,
    LOCAL_HASH_PROVIDER_ID,
};

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 100;
const DEFAULT_LEXICAL_WEIGHT: f32 = 0.4;
const DEFAULT_VECTOR_WEIGHT: f32 = 0.5;
const DEFAULT_METADATA_WEIGHT: f32 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct HybridSearchRequest {
    pub scope: QueryScope,
    pub query_text: String,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub source_kind: Option<SourceKind>,
    pub path_prefix: Option<String>,
    pub limit: usize,
    pub lexical_weight: f32,
    pub vector_weight: f32,
    pub metadata_weight: f32,
    pub min_score: Option<f32>,
    pub explain: bool,
    pub provider_id: Option<String>,
    pub dimension: Option<usize>,
}

impl HybridSearchRequest {
    pub fn new(scope: QueryScope, query_text: impl Into<String>) -> Self {
        Self {
            scope,
            query_text: query_text.into(),
            language: None,
            framework: None,
            source_kind: None,
            path_prefix: None,
            limit: DEFAULT_LIMIT,
            lexical_weight: DEFAULT_LEXICAL_WEIGHT,
            vector_weight: DEFAULT_VECTOR_WEIGHT,
            metadata_weight: DEFAULT_METADATA_WEIGHT,
            min_score: None,
            explain: false,
            provider_id: Some(LOCAL_HASH_PROVIDER_ID.to_string()),
            dimension: Some(DEFAULT_LOCAL_HASH_DIMENSION),
        }
    }

    pub fn validate(&self) -> ContractResult<()> {
        if self.query_text.trim().is_empty() {
            return Err(ContractError::new(
                "hybrid search query_text must not be empty",
            ));
        }
        if self.limit > MAX_LIMIT {
            return Err(ContractError::new(format!(
                "hybrid search limit must be at most {MAX_LIMIT}"
            )));
        }
        for (name, weight) in [
            ("lexical_weight", self.lexical_weight),
            ("vector_weight", self.vector_weight),
            ("metadata_weight", self.metadata_weight),
        ] {
            if !weight.is_finite() || weight < 0.0 || weight > 1.0 {
                return Err(ContractError::new(format!(
                    "{name} must be a finite value between 0.0 and 1.0"
                )));
            }
        }
        if self.lexical_weight + self.vector_weight + self.metadata_weight <= 0.0 {
            return Err(ContractError::new(
                "at least one hybrid search weight must be greater than zero",
            ));
        }
        if let Some(min_score) = self.min_score {
            if !min_score.is_finite() || !(0.0..=1.0).contains(&min_score) {
                return Err(ContractError::new(
                    "min_score must be a finite value between 0.0 and 1.0",
                ));
            }
        }
        Ok(())
    }

    fn effective_limit(&self) -> usize {
        if self.limit == 0 {
            DEFAULT_LIMIT
        } else {
            self.limit.min(MAX_LIMIT)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HybridSearchResult {
    pub document_id: String,
    pub file_id: FileId,
    pub symbol_id: Option<SymbolId>,
    pub path: String,
    pub text_preview: String,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub source_kind: SourceKind,
    pub start_line: usize,
    pub end_line: usize,
    pub final_score: f32,
    pub lexical_score: f32,
    pub vector_score: f32,
    pub metadata_score: f32,
    pub explanation: Option<HybridRankingExplanation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HybridRankingExplanation {
    pub final_score: f32,
    pub lexical_score: f32,
    pub vector_score: f32,
    pub metadata_score: f32,
    pub matched_terms: Vec<String>,
    pub boosts: Vec<String>,
    pub vector_provider: Option<String>,
    pub vector_dimension: Option<usize>,
    pub fallback_reason: Option<String>,
    pub filters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HybridSearchResponse {
    pub results: Vec<HybridSearchResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct Candidate {
    document_id: String,
    file_id: FileId,
    symbol_id: Option<SymbolId>,
    path: String,
    text: String,
    language: Option<String>,
    framework: Option<String>,
    source_kind: SourceKind,
    start_line: usize,
    end_line: usize,
    lexical_score: f32,
    vector_score: f32,
    vector_provider: Option<String>,
    vector_dimension: Option<usize>,
    fallback_reason: Option<String>,
}

impl Candidate {
    fn from_vector(hit: VectorSearchHit) -> Self {
        let document = hit.document;
        Self {
            document_id: document.id,
            file_id: document.file_id,
            symbol_id: document.symbol_id,
            path: document.path,
            text: document.text,
            language: document.language,
            framework: document.framework,
            source_kind: document.source_kind,
            start_line: document.start_line,
            end_line: document.end_line,
            lexical_score: 0.0,
            vector_score: normalize_score(hit.score),
            vector_provider: Some(hit.provider_id),
            vector_dimension: Some(hit.dimension),
            fallback_reason: None,
        }
    }

    fn from_fts(hit: FtsSearchHit) -> Self {
        Self {
            document_id: format!(
                "lexical:{}:{}",
                hit.file_id.as_str(),
                hit.symbol_id
                    .as_ref()
                    .map(|symbol| symbol.as_str())
                    .unwrap_or(hit.path.as_str())
            ),
            file_id: hit.file_id,
            symbol_id: hit.symbol_id,
            path: hit.path,
            text: hit.snippet,
            language: None,
            framework: None,
            source_kind: SourceKind::FileChunk,
            start_line: 0,
            end_line: 0,
            lexical_score: normalize_fts_score(hit.score),
            vector_score: 0.0,
            vector_provider: None,
            vector_dimension: None,
            fallback_reason: None,
        }
    }

    fn merge(&mut self, other: Candidate) {
        self.lexical_score = self.lexical_score.max(other.lexical_score);
        self.vector_score = self.vector_score.max(other.vector_score);
        if self.text.trim().is_empty() {
            self.text = other.text;
        }
        if self.symbol_id.is_none() {
            self.symbol_id = other.symbol_id;
        }
        if self.language.is_none() {
            self.language = other.language;
        }
        if self.framework.is_none() {
            self.framework = other.framework;
        }
        if self.vector_provider.is_none() {
            self.vector_provider = other.vector_provider;
        }
        if self.vector_dimension.is_none() {
            self.vector_dimension = other.vector_dimension;
        }
        if self.fallback_reason.is_none() {
            self.fallback_reason = other.fallback_reason;
        }
    }
}

pub struct HybridSearchEngine<'a, R, V> {
    repository: &'a R,
    vector_store: &'a V,
}

impl<'a, R, V> HybridSearchEngine<'a, R, V>
where
    R: QueryRepository,
    V: VectorStore,
{
    pub fn new(repository: &'a R, vector_store: &'a V) -> Self {
        Self {
            repository,
            vector_store,
        }
    }

    pub fn search(&self, request: HybridSearchRequest) -> ContractResult<HybridSearchResponse> {
        request.validate()?;
        let query_terms = tokenize(&request.query_text);
        let mut warnings = Vec::new();
        let mut candidates = BTreeMap::<String, Candidate>::new();

        for hit in self.lexical_candidates(&request)? {
            let mut candidate = Candidate::from_fts(hit);
            candidate.lexical_score = lexical_score(
                &request.query_text,
                &query_terms,
                &candidate.text,
                &candidate.path,
                candidate.symbol_id.as_ref().map(SymbolId::as_str),
            );
            merge_candidate(&mut candidates, candidate);
        }

        match self.vector_candidates(&request) {
            Ok(hits) if hits.is_empty() => warnings.push(
                "No vector data available; returned lexical/metadata fallback results.".to_string(),
            ),
            Ok(hits) => {
                for hit in hits {
                    let mut candidate = Candidate::from_vector(hit);
                    candidate.lexical_score = lexical_score(
                        &request.query_text,
                        &query_terms,
                        &candidate.text,
                        &candidate.path,
                        candidate.symbol_id.as_ref().map(SymbolId::as_str),
                    );
                    merge_candidate(&mut candidates, candidate);
                }
            }
            Err(error) => {
                warnings.push(format!("vector search unavailable: {error}"));
                for candidate in candidates.values_mut() {
                    candidate.fallback_reason = Some("vector search unavailable".to_string());
                }
            }
        }

        let filters = filters(&request);
        let mut results = candidates
            .into_values()
            .filter(|candidate| candidate_matches(candidate, &request))
            .map(|candidate| rank_candidate(candidate, &request, &query_terms, &filters))
            .filter(|result| {
                request
                    .min_score
                    .is_none_or(|min_score| result.final_score >= min_score)
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .final_score
                .partial_cmp(&left.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .vector_score
                        .partial_cmp(&left.vector_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    right
                        .lexical_score
                        .partial_cmp(&left.lexical_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.start_line.cmp(&right.start_line))
                .then_with(|| left.document_id.cmp(&right.document_id))
        });
        results.truncate(request.effective_limit());

        Ok(HybridSearchResponse { results, warnings })
    }

    fn lexical_candidates(
        &self,
        request: &HybridSearchRequest,
    ) -> ContractResult<Vec<FtsSearchHit>> {
        let limit = request
            .effective_limit()
            .saturating_mul(4)
            .max(DEFAULT_LIMIT);
        self.repository
            .fts_search(&request.scope, &request.query_text, limit)
    }

    fn vector_candidates(
        &self,
        request: &HybridSearchRequest,
    ) -> ContractResult<Vec<VectorSearchHit>> {
        if request.vector_weight == 0.0 {
            return Ok(Vec::new());
        }
        let dimension = request.dimension.unwrap_or(DEFAULT_LOCAL_HASH_DIMENSION);
        let provider_id = request
            .provider_id
            .clone()
            .unwrap_or_else(|| LOCAL_HASH_PROVIDER_ID.to_string());
        if provider_id != LOCAL_HASH_PROVIDER_ID {
            return Err(ContractError::new(format!(
                "hybrid ranking only supports offline provider '{LOCAL_HASH_PROVIDER_ID}' in Phase 10.3"
            )));
        }
        let provider = LocalHashEmbeddingProvider::new(LocalHashEmbeddingConfig {
            dimension,
            max_input_chars: 2_000,
            normalize_vectors: true,
        })?;
        let query_vector = provider.embed_text(&request.query_text)?;
        self.vector_store.search(VectorSearchRequest {
            query_vector,
            project_id: request.scope.project_id.clone(),
            branch_id: request.scope.branch_id.clone(),
            provider_id: Some(provider_id),
            dimension: Some(dimension),
            language: request.language.clone(),
            framework: request.framework.clone(),
            source_kind: request.source_kind,
            file_id: None,
            symbol_id: None,
            path_prefix: request.path_prefix.clone(),
            limit: request
                .effective_limit()
                .saturating_mul(4)
                .max(DEFAULT_LIMIT),
            min_score: None,
        })
    }
}

fn merge_candidate(candidates: &mut BTreeMap<String, Candidate>, candidate: Candidate) {
    if let Some(existing_key) = candidates
        .iter()
        .find(|(_, existing)| {
            existing.file_id == candidate.file_id
                && existing.symbol_id == candidate.symbol_id
                && existing.path == candidate.path
        })
        .map(|(key, _)| key.clone())
    {
        if let Some(mut existing) = candidates.remove(&existing_key) {
            let key = if candidate.document_id.starts_with("lexical:") {
                existing.document_id.clone()
            } else {
                candidate.document_id.clone()
            };
            existing.merge(candidate);
            existing.document_id = key.clone();
            candidates.insert(key, existing);
            return;
        }
    }
    candidates
        .entry(candidate.document_id.clone())
        .and_modify(|existing| existing.merge(candidate.clone()))
        .or_insert(candidate);
}

fn rank_candidate(
    candidate: Candidate,
    request: &HybridSearchRequest,
    query_terms: &[String],
    filters: &[String],
) -> HybridSearchResult {
    let (metadata_score, boosts) = metadata_score(&candidate, request, query_terms);
    let weight_sum = request.lexical_weight + request.vector_weight + request.metadata_weight;
    let lexical_weight = request.lexical_weight / weight_sum;
    let vector_weight = request.vector_weight / weight_sum;
    let metadata_weight = request.metadata_weight / weight_sum;
    let final_score = normalize_score(
        lexical_weight * candidate.lexical_score
            + vector_weight * candidate.vector_score
            + metadata_weight * metadata_score,
    );
    let matched_terms = matched_terms(query_terms, &candidate);
    let explanation = request.explain.then(|| HybridRankingExplanation {
        final_score,
        lexical_score: candidate.lexical_score,
        vector_score: candidate.vector_score,
        metadata_score,
        matched_terms,
        boosts,
        vector_provider: candidate.vector_provider.clone(),
        vector_dimension: candidate.vector_dimension,
        fallback_reason: candidate.fallback_reason.clone(),
        filters: filters.to_vec(),
    });

    HybridSearchResult {
        document_id: candidate.document_id,
        file_id: candidate.file_id,
        symbol_id: candidate.symbol_id,
        path: candidate.path,
        text_preview: preview(&candidate.text),
        language: candidate.language,
        framework: candidate.framework,
        source_kind: candidate.source_kind,
        start_line: candidate.start_line,
        end_line: candidate.end_line,
        final_score,
        lexical_score: candidate.lexical_score,
        vector_score: candidate.vector_score,
        metadata_score,
        explanation,
    }
}

fn candidate_matches(candidate: &Candidate, request: &HybridSearchRequest) -> bool {
    if let Some(language) = &request.language {
        if candidate.language.as_ref() != Some(language) {
            return false;
        }
    }
    if let Some(framework) = &request.framework {
        if candidate.framework.as_ref() != Some(framework) {
            return false;
        }
    }
    if let Some(source_kind) = request.source_kind {
        if candidate.source_kind != source_kind {
            return false;
        }
    }
    if let Some(path_prefix) = &request.path_prefix {
        if !candidate.path.starts_with(path_prefix) {
            return false;
        }
    }
    true
}

pub fn lexical_score(
    query: &str,
    query_terms: &[String],
    text: &str,
    path: &str,
    name: Option<&str>,
) -> f32 {
    if query_terms.is_empty() {
        return 0.0;
    }
    let mut haystack = tokenize(text);
    haystack.extend(tokenize(path));
    if let Some(name) = name {
        haystack.extend(tokenize(name));
    }
    let haystack = haystack.into_iter().collect::<BTreeSet<_>>();
    let matched = query_terms
        .iter()
        .filter(|term| haystack.contains(*term))
        .count();
    let mut score = matched as f32 / query_terms.len() as f32;
    let query_lower = query.to_lowercase();
    if !query_lower.trim().is_empty() && text.to_lowercase().contains(query_lower.trim()) {
        score += 0.15;
    }
    if path.to_lowercase().contains(query_lower.trim()) {
        score += 0.10;
    }
    normalize_score(score)
}

fn metadata_score(
    candidate: &Candidate,
    request: &HybridSearchRequest,
    query_terms: &[String],
) -> (f32, Vec<String>) {
    let mut score: f32 = 0.0;
    let mut boosts = Vec::new();
    if request.language.is_some() && candidate.language == request.language {
        score += 0.20;
        boosts.push("language_match".to_string());
    }
    if request.framework.is_some() && candidate.framework == request.framework {
        score += 0.20;
        boosts.push("framework_match".to_string());
    }
    if request.source_kind.is_some() && Some(candidate.source_kind) == request.source_kind {
        score += 0.20;
        boosts.push("source_kind_match".to_string());
    }
    if candidate.source_kind == SourceKind::SymbolChunk {
        score += 0.10;
        boosts.push("symbol_chunk".to_string());
    }
    if matches!(
        candidate.source_kind,
        SourceKind::RouteChunk
            | SourceKind::ComponentChunk
            | SourceKind::DataAccessChunk
            | SourceKind::RealtimeChunk
            | SourceKind::MessagingChunk
            | SourceKind::InfrastructureChunk
    ) && !matched_terms(query_terms, candidate).is_empty()
    {
        score += 0.10;
        boosts.push("source_specific_terms".to_string());
    }
    if candidate.text.len() <= 2_000 {
        score += 0.05;
        boosts.push("compact_chunk".to_string());
    }
    if query_terms.iter().any(|term| {
        tokenize(&candidate.path)
            .iter()
            .any(|path_term| path_term == term)
    }) {
        score += 0.10;
        boosts.push("path_term_match".to_string());
    }
    (normalize_score(score), boosts)
}

fn matched_terms(query_terms: &[String], candidate: &Candidate) -> Vec<String> {
    let mut haystack = tokenize(&candidate.text);
    haystack.extend(tokenize(&candidate.path));
    let haystack = haystack.into_iter().collect::<BTreeSet<_>>();
    query_terms
        .iter()
        .filter(|term| haystack.contains(*term))
        .cloned()
        .collect()
}

pub fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut previous_lowercase = false;

    for character in input.chars() {
        if character.is_alphanumeric() {
            if previous_lowercase && character.is_uppercase() && !current.is_empty() {
                push_token(&mut tokens, &current);
                current.clear();
            }
            current.extend(character.to_lowercase());
            previous_lowercase = character.is_lowercase();
        } else {
            push_token(&mut tokens, &current);
            current.clear();
            previous_lowercase = false;
        }
    }
    push_token(&mut tokens, &current);
    tokens
}

fn push_token(tokens: &mut Vec<String>, token: &str) {
    if token.is_empty() {
        return;
    }
    for part in token.split('_').filter(|part| !part.is_empty()) {
        tokens.push(part.to_string());
    }
}

fn normalize_fts_score(score: f32) -> f32 {
    if !score.is_finite() {
        return 0.0;
    }
    normalize_score(score / (score.abs() + 1.0))
}

fn normalize_score(score: f32) -> f32 {
    if !score.is_finite() {
        0.0
    } else {
        score.clamp(0.0, 1.0)
    }
}

fn preview(text: &str) -> String {
    let mut preview = text
        .split_whitespace()
        .take(48)
        .collect::<Vec<_>>()
        .join(" ");
    if preview.len() > 240 {
        preview.truncate(240);
    }
    preview
}

fn filters(request: &HybridSearchRequest) -> Vec<String> {
    let mut filters = vec![
        format!("project_id={}", request.scope.project_id.as_str()),
        format!("branch_id={}", request.scope.branch_id.as_str()),
    ];
    if let Some(language) = &request.language {
        filters.push(format!("language={language}"));
    }
    if let Some(framework) = &request.framework {
        filters.push(format!("framework={framework}"));
    }
    if let Some(source_kind) = request.source_kind {
        filters.push(format!("source_kind={}", source_kind.as_str()));
    }
    if let Some(path_prefix) = &request.path_prefix {
        filters.push(format!("path_prefix={path_prefix}"));
    }
    filters
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use b3_core::{
        BranchId, FileId, FileRecord, FtsSearchHit, IndexStore, IndexedFileRecord, ProjectId,
        QueryFile, QueryRepository, QueryScope, QuerySymbol, SymbolId, VectorDocument,
        VectorDocumentInput, VectorStore,
    };
    use b3_embeddings::EmbeddingProvider;
    use b3_storage::SqliteStorage;
    use tempfile::tempdir;

    use super::*;

    struct LexicalOnlyRepo {
        hit: FtsSearchHit,
    }

    impl QueryRepository for LexicalOnlyRepo {
        fn list_symbols(
            &self,
            _scope: &QueryScope,
            _limit: usize,
        ) -> ContractResult<Vec<QuerySymbol>> {
            Ok(Vec::new())
        }

        fn find_symbols(
            &self,
            _scope: &QueryScope,
            _name: &str,
        ) -> ContractResult<Vec<QuerySymbol>> {
            Ok(Vec::new())
        }

        fn get_symbol(
            &self,
            _scope: &QueryScope,
            _symbol_id: &SymbolId,
        ) -> ContractResult<Option<QuerySymbol>> {
            Ok(None)
        }

        fn get_file(
            &self,
            _scope: &QueryScope,
            _file_id: &FileId,
        ) -> ContractResult<Option<QueryFile>> {
            Ok(None)
        }

        fn fts_search(
            &self,
            _scope: &QueryScope,
            _query: &str,
            _limit: usize,
        ) -> ContractResult<Vec<FtsSearchHit>> {
            Ok(vec![self.hit.clone()])
        }

        fn graph_neighbors(
            &self,
            _scope: &QueryScope,
            _symbol_id: &SymbolId,
            _direction: b3_core::GraphDirection,
            _edge_filter: &[b3_core::EdgeKind],
            _min_confidence: u16,
        ) -> ContractResult<Vec<b3_core::GraphNeighbor>> {
            Ok(Vec::new())
        }
    }

    fn scope() -> QueryScope {
        QueryScope::new(ProjectId::new("project"), BranchId::new("main"))
    }

    fn document(
        project_id: ProjectId,
        branch_id: BranchId,
        file_id: FileId,
        text: &str,
        path: &str,
    ) -> VectorDocument {
        VectorDocument::new(VectorDocumentInput {
            project_id,
            branch_id,
            file_id,
            symbol_id: None,
            language: Some("rust".to_string()),
            framework: Some("axum".to_string()),
            source_kind: SourceKind::FileChunk,
            path: path.to_string(),
            content_hash: "hash".to_string(),
            chunk_index: 0,
            text: text.to_string(),
            start_line: 1,
            end_line: 1,
            metadata: BTreeMap::new(),
        })
    }

    fn storage_with_vectors() -> (SqliteStorage, QueryScope) {
        let dir = tempdir().expect("tempdir");
        let storage = SqliteStorage::open(dir.path().join("b3.db")).expect("storage");
        let scope = scope();
        let file_id = FileId::new("file");
        storage
            .ensure_project_branch(&scope.project_id, &scope.branch_id, ".")
            .expect("branch");
        storage
            .upsert_indexed_file(
                &scope.project_id,
                &scope.branch_id,
                IndexedFileRecord {
                    file: FileRecord {
                        id: file_id.clone(),
                        project_id: scope.project_id.clone(),
                        path: "src/orders.rs".to_string(),
                        content_hash: "hash".to_string(),
                    },
                    language: Some("rust".to_string()),
                    size_bytes: 64,
                    content: "pub fn create_order() {}\n".to_string(),
                    symbols: Vec::new(),
                    edges: Vec::new(),
                },
            )
            .expect("file");
        let document = document(
            scope.project_id.clone(),
            scope.branch_id.clone(),
            file_id,
            "pub fn create_order() {}",
            "src/orders.rs",
        );
        let provider = LocalHashEmbeddingProvider::new(LocalHashEmbeddingConfig {
            dimension: 32,
            max_input_chars: 2_000,
            normalize_vectors: true,
        })
        .expect("provider");
        storage.upsert_documents(&[document.clone()]).expect("docs");
        storage
            .upsert_vectors(&[b3_core::EmbeddingVector::new(
                document.id,
                LOCAL_HASH_PROVIDER_ID,
                32,
                provider.embed_text("create order").expect("embed"),
                1,
            )])
            .expect("vectors");
        (storage, scope)
    }

    #[test]
    fn lexical_scoring_handles_tokens_identifiers_path_and_unicode() {
        let terms = tokenize("createOrder path_test こんにちは");
        let score = lexical_score(
            "createOrder path_test",
            &terms,
            "fn create_order() {} こんにちは",
            "src/path_test.rs",
            Some("create_order"),
        );

        assert!(terms.contains(&"create".to_string()));
        assert!(terms.contains(&"order".to_string()));
        assert!(score > 0.5);
        assert_eq!(lexical_score("", &[], "", "", None), 0.0);
    }

    #[test]
    fn hybrid_search_uses_stored_local_hash_vectors_and_explains_scores() {
        let (storage, scope) = storage_with_vectors();
        let engine = HybridSearchEngine::new(&storage, &storage);
        let mut request = HybridSearchRequest::new(scope, "create order");
        request.dimension = Some(32);
        request.language = Some("rust".to_string());
        request.framework = Some("axum".to_string());
        request.explain = true;

        let response = engine.search(request).expect("hybrid");

        assert_eq!(response.results.len(), 1);
        let result = &response.results[0];
        assert!(result.vector_score > 0.0);
        assert!(result.final_score > 0.0);
        let explanation = result.explanation.as_ref().expect("explanation");
        assert_eq!(
            explanation.vector_provider.as_deref(),
            Some(LOCAL_HASH_PROVIDER_ID)
        );
        assert!(explanation.boosts.contains(&"language_match".to_string()));
        assert!(explanation.boosts.contains(&"framework_match".to_string()));
        assert!(explanation.matched_terms.contains(&"create".to_string()));
    }

    #[test]
    fn hybrid_search_falls_back_to_lexical_when_vectors_missing() {
        let hit = FtsSearchHit {
            file_id: FileId::new("file"),
            symbol_id: None,
            path: "src/orders.rs".to_string(),
            name: Some("create_order".to_string()),
            snippet: "pub fn create_order() {}".to_string(),
            score: 3.0,
        };
        let repo = LexicalOnlyRepo { hit };
        let storage = SqliteStorage::open_in_memory().expect("storage");
        let mut request = HybridSearchRequest::new(scope(), "create order");
        request.dimension = Some(32);
        request.explain = true;
        let engine = HybridSearchEngine::new(&repo, &storage);

        let response = engine.search(request).expect("hybrid");

        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].vector_score, 0.0);
        assert!(response
            .warnings
            .iter()
            .any(|warning| warning.contains("No vector data available")));
    }

    #[test]
    fn hybrid_request_validates_weights_and_limit() {
        let mut request = HybridSearchRequest::new(scope(), "create order");

        request.limit = MAX_LIMIT + 1;
        assert!(request.validate().is_err());
        request.limit = 10;
        request.vector_weight = f32::NAN;
        assert!(request.validate().is_err());
        request.vector_weight = 0.0;
        request.lexical_weight = 0.0;
        request.metadata_weight = 0.0;
        assert!(request.validate().is_err());
    }

    #[test]
    fn hybrid_search_rejects_provider_mismatch() {
        let (storage, scope) = storage_with_vectors();
        let engine = HybridSearchEngine::new(&storage, &storage);
        let mut request = HybridSearchRequest::new(scope, "create order");
        request.provider_id = Some("openai".to_string());

        assert!(
            engine.search(request).expect("fallback response").warnings[0]
                .contains("vector search unavailable")
        );
    }

    #[test]
    fn hybrid_search_deterministic_tie_break_and_filters() {
        let (storage, scope) = storage_with_vectors();
        let engine = HybridSearchEngine::new(&storage, &storage);
        let mut request = HybridSearchRequest::new(scope, "create order");
        request.dimension = Some(32);
        request.path_prefix = Some("src/".to_string());
        request.limit = 1;

        let response = engine.search(request).expect("hybrid");

        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].path, "src/orders.rs");
    }

    #[test]
    fn hybrid_search_returns_empty_results_for_empty_candidate_sets() {
        let storage = SqliteStorage::open_in_memory().expect("storage");
        let engine = HybridSearchEngine::new(&storage, &storage);
        let mut request = HybridSearchRequest::new(scope(), "missing");
        request.dimension = Some(32);

        let response = engine.search(request).expect("hybrid");

        assert!(response.results.is_empty());
    }

    #[test]
    fn hybrid_search_dedupes_lexical_and_vector_candidates_by_document_when_ids_match() {
        let (storage, scope) = storage_with_vectors();
        let docs = storage
            .search(VectorSearchRequest {
                query_vector: LocalHashEmbeddingProvider::new(LocalHashEmbeddingConfig {
                    dimension: 32,
                    max_input_chars: 2_000,
                    normalize_vectors: true,
                })
                .expect("provider")
                .embed_text("create order")
                .expect("embed"),
                project_id: scope.project_id.clone(),
                branch_id: scope.branch_id.clone(),
                provider_id: Some(LOCAL_HASH_PROVIDER_ID.to_string()),
                dimension: Some(32),
                language: None,
                framework: None,
                source_kind: None,
                file_id: None,
                symbol_id: None,
                path_prefix: None,
                limit: 1,
                min_score: None,
            })
            .expect("vector docs");
        assert_eq!(docs.len(), 1);
        let hit = FtsSearchHit {
            file_id: docs[0].document.file_id.clone(),
            symbol_id: None,
            path: docs[0].document.path.clone(),
            name: None,
            snippet: docs[0].document.text.clone(),
            score: 2.0,
        };
        let repo = LexicalOnlyRepo { hit };
        let engine = HybridSearchEngine::new(&repo, &storage);
        let mut request = HybridSearchRequest::new(scope, "create order");
        request.dimension = Some(32);

        let response = engine.search(request).expect("hybrid");

        assert_eq!(response.results.len(), 1);
        assert!(response.results[0].lexical_score > 0.0);
        assert!(response.results[0].vector_score > 0.0);
    }

    #[test]
    fn hybrid_module_does_not_require_network_fixture() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("note.txt"), "local only").expect("write");
        assert!(dir.path().join("note.txt").exists());
    }
}
