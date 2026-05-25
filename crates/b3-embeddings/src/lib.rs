//! Local embedding providers.
//!
//! The default provider is `local_hash`: a deterministic lexical hashing
//! provider that works fully offline and does not download models. Cloud
//! providers remain future optional plugins and are not registered here.

use std::collections::BTreeMap;

pub use b3_core::{
    EmbeddingConfig, EmbeddingProvider, EmbeddingProviderCapabilities, EmbeddingProviderKind,
    EmbeddingRequest, EmbeddingResult, EmbeddingVector, LocalEmbeddingProviderKind,
};

use b3_core::{
    normalize_l2, stable_hash, validate_dimension, ContractError, ContractResult, VectorDocument,
};

pub const LOCAL_HASH_PROVIDER_ID: &str = "local_hash";
pub const LOCAL_HASH_MODEL_ID: &str = "local_hash_v1";
pub const DEFAULT_LOCAL_HASH_DIMENSION: usize = 384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalHashEmbeddingConfig {
    pub dimension: usize,
    pub max_input_chars: usize,
    pub normalize_vectors: bool,
}

impl Default for LocalHashEmbeddingConfig {
    fn default() -> Self {
        Self {
            dimension: DEFAULT_LOCAL_HASH_DIMENSION,
            max_input_chars: 2_000,
            normalize_vectors: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalHashEmbeddingProvider {
    config: LocalHashEmbeddingConfig,
}

impl LocalHashEmbeddingProvider {
    pub fn new(config: LocalHashEmbeddingConfig) -> ContractResult<Self> {
        if config.dimension == 0 {
            return Err(ContractError::new(
                "local_hash embedding dimension must be greater than zero",
            ));
        }
        if config.max_input_chars == 0 {
            return Err(ContractError::new(
                "local_hash max_input_chars must be greater than zero",
            ));
        }
        Ok(Self { config })
    }

    pub fn default_provider() -> Self {
        Self {
            config: LocalHashEmbeddingConfig::default(),
        }
    }

    fn truncated_input<'a>(&self, input: &'a str) -> &'a str {
        if input.len() <= self.config.max_input_chars {
            return input;
        }
        let mut end = 0;
        for (index, _) in input.char_indices() {
            if index > self.config.max_input_chars {
                break;
            }
            end = index;
        }
        &input[..end]
    }

    fn hash_token(&self, token: &str, seed: &str) -> (usize, f32) {
        let hash = stable_hash(&[seed, token]);
        let bucket = u64::from_str_radix(&hash[..16], 16).unwrap_or(0);
        let index = (bucket as usize) % self.config.dimension;
        let sign = if (bucket & 1) == 0 { 1.0 } else { -1.0 };
        (index, sign)
    }
}

impl Default for LocalHashEmbeddingProvider {
    fn default() -> Self {
        Self::default_provider()
    }
}

impl EmbeddingProvider for LocalHashEmbeddingProvider {
    fn id(&self) -> &str {
        LOCAL_HASH_PROVIDER_ID
    }

    fn name(&self) -> &str {
        "Local Hash Embeddings"
    }

    fn dimension(&self) -> usize {
        self.config.dimension
    }

    fn max_input_chars(&self) -> usize {
        self.config.max_input_chars
    }

    fn capabilities(&self) -> EmbeddingProviderCapabilities {
        EmbeddingProviderCapabilities::local_deterministic()
    }

    fn config(&self) -> EmbeddingConfig {
        EmbeddingConfig {
            enabled: true,
            provider: LocalEmbeddingProviderKind::LocalHash,
            provider_id: LOCAL_HASH_PROVIDER_ID.to_string(),
            model: LOCAL_HASH_MODEL_ID.to_string(),
            dimension: self.dimension(),
            max_chunk_chars: self.max_input_chars(),
            normalize_vectors: self.config.normalize_vectors,
            ..EmbeddingConfig::default()
        }
    }

    fn embed_text(&self, input: &str) -> ContractResult<Vec<f32>> {
        let input = self.truncated_input(input);
        let tokens = tokenize(input);
        let mut vector = vec![0.0; self.config.dimension];

        for token in tokens.iter() {
            let (index, sign) = self.hash_token(token, "token");
            vector[index] += sign;
        }

        for pair in tokens.windows(2) {
            let bigram = format!("{}::{}", pair[0], pair[1]);
            let (index, sign) = self.hash_token(&bigram, "bigram");
            vector[index] += sign * 0.5;
        }

        if self.config.normalize_vectors {
            normalize_l2(&mut vector);
        }
        validate_dimension(&vector, self.config.dimension)?;
        Ok(vector)
    }

    fn embed_batch(&self, inputs: &[String]) -> ContractResult<Vec<Vec<f32>>> {
        inputs.iter().map(|input| self.embed_text(input)).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingProviderInfo {
    pub id: String,
    pub name: String,
    pub kind: EmbeddingProviderKind,
    pub dimension: usize,
    pub local_only: bool,
    pub deterministic: bool,
    pub batch: bool,
}

#[derive(Default)]
pub struct EmbeddingProviderRegistry {
    providers: BTreeMap<String, Box<dyn EmbeddingProvider + Send + Sync>>,
}

impl EmbeddingProviderRegistry {
    pub fn offline_default() -> Self {
        let mut registry = Self::default();
        registry.register(Box::new(LocalHashEmbeddingProvider::default_provider()));
        registry
    }

    pub fn register(&mut self, provider: Box<dyn EmbeddingProvider + Send + Sync>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    pub fn get(&self, provider_id: &str) -> ContractResult<&(dyn EmbeddingProvider + Send + Sync)> {
        self.providers
            .get(provider_id)
            .map(|provider| provider.as_ref())
            .ok_or_else(|| {
                ContractError::new(format!(
                    "embedding provider '{provider_id}' is not registered"
                ))
            })
    }

    pub fn available_providers(&self) -> Vec<EmbeddingProviderInfo> {
        self.providers
            .values()
            .map(|provider| {
                let capabilities = provider.capabilities();
                EmbeddingProviderInfo {
                    id: provider.id().to_string(),
                    name: provider.name().to_string(),
                    kind: capabilities.kind,
                    dimension: provider.dimension(),
                    local_only: capabilities.local_only,
                    deterministic: capabilities.deterministic,
                    batch: capabilities.batch,
                }
            })
            .collect()
    }
}

pub fn provider_from_config(
    config: &EmbeddingConfig,
) -> ContractResult<Box<dyn EmbeddingProvider + Send + Sync>> {
    config.validate_offline_defaults()?;
    match config.provider_id.as_str() {
        LOCAL_HASH_PROVIDER_ID => Ok(Box::new(LocalHashEmbeddingProvider::new(
            LocalHashEmbeddingConfig {
                dimension: config.dimension,
                max_input_chars: config.max_chunk_chars,
                normalize_vectors: config.normalize_vectors,
            },
        )?)),
        "deterministic-test" => Ok(Box::new(DeterministicTestEmbeddingProvider::new(
            config.dimension.max(1),
        ))),
        "none" => Err(ContractError::new("embedding provider is disabled")),
        other => Err(ContractError::new(format!(
            "embedding provider '{other}' is not available offline"
        ))),
    }
}

pub fn embed_documents(
    provider: &(dyn EmbeddingProvider + Send + Sync),
    documents: &[VectorDocument],
    indexed_at_unix_ms: u64,
) -> ContractResult<Vec<EmbeddingVector>> {
    let inputs = documents
        .iter()
        .map(|document| document.text.clone())
        .collect::<Vec<_>>();
    let vectors = provider.embed_batch(&inputs)?;
    if vectors.len() != documents.len() {
        return Err(ContractError::new(format!(
            "embedding batch output count mismatch: expected {}, got {}",
            documents.len(),
            vectors.len()
        )));
    }

    documents
        .iter()
        .zip(vectors)
        .map(|(document, vector)| {
            validate_dimension(&vector, provider.dimension())?;
            Ok(EmbeddingVector::new(
                document.id.clone(),
                provider.id(),
                provider.dimension(),
                vector,
                indexed_at_unix_ms,
            ))
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct DeterministicTestEmbeddingProvider {
    dimension: usize,
    max_input_chars: usize,
}

impl DeterministicTestEmbeddingProvider {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            max_input_chars: 2_000,
        }
    }

    fn value_for(input: &str, index: usize) -> f32 {
        let hash = stable_hash(&[input, &index.to_string()]);
        let bucket = u32::from_str_radix(&hash[..8], 16).unwrap_or(0);
        (bucket as f32 / u32::MAX as f32) - 0.5
    }
}

impl EmbeddingProvider for DeterministicTestEmbeddingProvider {
    fn id(&self) -> &str {
        "deterministic-test"
    }

    fn name(&self) -> &str {
        "Deterministic Test Embeddings"
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn max_input_chars(&self) -> usize {
        self.max_input_chars
    }

    fn capabilities(&self) -> EmbeddingProviderCapabilities {
        EmbeddingProviderCapabilities::local_test()
    }

    fn embed_text(&self, input: &str) -> ContractResult<Vec<f32>> {
        let mut vector = (0..self.dimension)
            .map(|index| Self::value_for(input, index))
            .collect::<Vec<_>>();
        normalize_l2(&mut vector);
        Ok(vector)
    }
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut previous_lowercase = false;

    for character in input.chars() {
        if character.is_alphanumeric() {
            if previous_lowercase && character.is_uppercase() && !current.is_empty() {
                push_split_token(&mut tokens, &current);
                current.clear();
            }
            current.extend(character.to_lowercase());
            previous_lowercase = character.is_lowercase();
        } else {
            push_split_token(&mut tokens, &current);
            current.clear();
            previous_lowercase = false;
        }
    }
    push_split_token(&mut tokens, &current);
    tokens
}

fn push_split_token(tokens: &mut Vec<String>, token: &str) {
    if token.is_empty() {
        return;
    }
    for part in token.split('_').filter(|part| !part.is_empty()) {
        tokens.push(part.to_string());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use b3_core::{BranchId, FileId, ProjectId, SourceKind, VectorDocument, VectorDocumentInput};

    use super::*;

    fn provider() -> LocalHashEmbeddingProvider {
        LocalHashEmbeddingProvider::new(LocalHashEmbeddingConfig {
            dimension: 32,
            max_input_chars: 64,
            normalize_vectors: true,
        })
        .expect("provider")
    }

    fn document(text: &str) -> VectorDocument {
        VectorDocument::new(VectorDocumentInput {
            project_id: ProjectId::new("project"),
            branch_id: BranchId::new("main"),
            file_id: FileId::new("file"),
            symbol_id: None,
            language: Some("rust".to_string()),
            framework: None,
            source_kind: SourceKind::FileChunk,
            path: "src/lib.rs".to_string(),
            content_hash: "content".to_string(),
            chunk_index: 0,
            text: text.to_string(),
            start_line: 1,
            end_line: 1,
            metadata: BTreeMap::from([("source".to_string(), "test".to_string())]),
        })
    }

    #[test]
    fn local_hash_provider_metadata_is_available_and_local_only() {
        let provider = provider();
        let capabilities = provider.capabilities();

        assert_eq!(provider.id(), LOCAL_HASH_PROVIDER_ID);
        assert_eq!(provider.dimension(), 32);
        assert_eq!(capabilities.kind, EmbeddingProviderKind::Local);
        assert!(capabilities.local_only);
        assert!(capabilities.deterministic);
        assert!(capabilities.batch);
        assert!(!capabilities.requires_network);
        assert!(!capabilities.requires_api_key);
        assert!(!capabilities.downloads_models);
        assert!(!capabilities.telemetry);
    }

    #[test]
    fn local_hash_is_deterministic_normalized_and_distinguishes_text() {
        let provider = provider();
        let first = provider
            .embed_text("pub fn createOrder() {}")
            .expect("first");
        let second = provider
            .embed_text("pub fn createOrder() {}")
            .expect("second");
        let different = provider
            .embed_text("delete invoice route")
            .expect("different");

        assert_eq!(first, second);
        assert_ne!(first, different);
        assert_eq!(first.len(), 32);
        assert!((b3_core::l2_norm(&first) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn batch_unicode_code_like_large_and_empty_inputs_are_safe() {
        let provider = provider();
        let inputs = vec![
            "こんにちは мир".to_string(),
            "GET /api/orders/:id snake_case camelCase".to_string(),
            "x".repeat(1_000),
            String::new(),
        ];
        let vectors = provider.embed_batch(&inputs).expect("batch");

        assert_eq!(vectors.len(), inputs.len());
        assert!(vectors.iter().all(|vector| vector.len() == 32));
        assert!(vectors[3].iter().all(|value| *value == 0.0));
    }

    #[test]
    fn registry_finds_local_hash_and_rejects_unknown_provider() {
        let registry = EmbeddingProviderRegistry::offline_default();
        let provider = registry.get(LOCAL_HASH_PROVIDER_ID).expect("local_hash");
        let providers = registry.available_providers();

        assert_eq!(provider.id(), LOCAL_HASH_PROVIDER_ID);
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].kind, EmbeddingProviderKind::Local);
        assert!(registry.get("openai").is_err());
    }

    #[test]
    fn config_builds_offline_local_hash_provider() {
        let config = EmbeddingConfig {
            enabled: true,
            dimension: 32,
            max_chunk_chars: 64,
            ..EmbeddingConfig::default()
        };
        let provider = provider_from_config(&config).expect("provider");

        assert_eq!(provider.id(), LOCAL_HASH_PROVIDER_ID);
        assert_eq!(provider.dimension(), 32);
    }

    #[test]
    fn documents_embed_with_provider_metadata_and_preserve_chunk_metadata() {
        let provider = provider();
        let documents = vec![document("pub fn run() {}")];
        let vectors = embed_documents(&provider, &documents, 42).expect("vectors");

        assert_eq!(vectors.len(), 1);
        assert_eq!(vectors[0].provider_id, LOCAL_HASH_PROVIDER_ID);
        assert_eq!(vectors[0].dimension, 32);
        assert_eq!(vectors[0].document_id, documents[0].id);
        assert_eq!(documents[0].metadata["source"], "test");
        assert_eq!(
            documents[0].chunk_hash,
            document("pub fn run() {}").chunk_hash
        );
    }

    #[test]
    fn deterministic_test_provider_remains_test_kind() {
        let provider = DeterministicTestEmbeddingProvider::new(8);
        let vector = provider.embed_text("pub fn run() {}").expect("vector");

        assert_eq!(provider.capabilities().kind, EmbeddingProviderKind::Test);
        assert_eq!(vector.len(), 8);
        assert!((b3_core::l2_norm(&vector) - 1.0).abs() < 0.0001);
    }
}
