//! Local embedding provider boundary.
//!
//! Core semantic search must work offline. Cloud embedding providers may be
//! added later as optional plugins, but they are not part of the default path.
//! This crate will host concrete local providers in later phases; Phase 10.0
//! includes only a deterministic provider suitable for tests and architecture
//! validation.

pub use b3_core::{
    EmbeddingConfig, EmbeddingProvider, EmbeddingProviderCapabilities, EmbeddingProviderKind,
    EmbeddingRequest, EmbeddingResult, LocalEmbeddingProviderKind,
};

use b3_core::{stable_hash, ContractResult};

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
        let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for value in &mut vector {
                *value /= magnitude;
            }
        }
        Ok(vector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_provider_is_stable_normalized_and_local_only() {
        let provider = DeterministicTestEmbeddingProvider::new(8);
        let first = provider.embed_text("pub fn run() {}").expect("first");
        let second = provider.embed_text("pub fn run() {}").expect("second");
        let magnitude = first.iter().map(|value| value * value).sum::<f32>().sqrt();

        assert_eq!(first, second);
        assert_eq!(first.len(), 8);
        assert!((magnitude - 1.0).abs() < 0.0001);
        assert!(provider.capabilities().local_only);
        assert!(!provider.capabilities().requires_network);
    }
}
