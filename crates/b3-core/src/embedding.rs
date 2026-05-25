//! Embedding provider contracts.
//!
//! Phase 10.0 defines the local/offline architecture only. Real local model
//! loading is deferred; external providers remain optional plugins and disabled
//! by default.

use serde::{Deserialize, Serialize};

use crate::{ContractError, ContractResult, EmbeddingConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingProviderKind {
    Local,
    Test,
    ExternalPlugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingProviderCapabilities {
    pub kind: EmbeddingProviderKind,
    pub local_only: bool,
    pub deterministic: bool,
    pub batch: bool,
    pub requires_network: bool,
    pub requires_api_key: bool,
    pub downloads_models: bool,
    pub telemetry: bool,
}

impl EmbeddingProviderCapabilities {
    pub const fn local_test() -> Self {
        Self {
            kind: EmbeddingProviderKind::Test,
            local_only: true,
            deterministic: true,
            batch: true,
            requires_network: false,
            requires_api_key: false,
            downloads_models: false,
            telemetry: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub provider_id: String,
    pub dimensions: usize,
    pub vector: Vec<f32>,
    pub normalized: bool,
}

impl EmbeddingResult {
    pub fn validate_dimension(&self, expected: usize) -> ContractResult<()> {
        if self.dimensions != expected || self.vector.len() != expected {
            return Err(ContractError::new(format!(
                "embedding dimension mismatch: expected {expected}, got {} values with declared dimension {}",
                self.vector.len(),
                self.dimensions
            )));
        }
        Ok(())
    }

    pub fn magnitude(&self) -> f32 {
        self.vector
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt()
    }

    pub fn is_unit_normalized(&self, epsilon: f32) -> bool {
        (self.magnitude() - 1.0).abs() <= epsilon
    }
}

pub trait EmbeddingProvider {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn dimension(&self) -> usize;
    fn max_input_chars(&self) -> usize;
    fn capabilities(&self) -> EmbeddingProviderCapabilities;

    fn config(&self) -> EmbeddingConfig {
        EmbeddingConfig {
            enabled: true,
            provider_id: self.id().to_string(),
            dimension: self.dimension(),
            max_chunk_chars: self.max_input_chars(),
            ..EmbeddingConfig::default()
        }
    }

    fn embed_text(&self, input: &str) -> ContractResult<Vec<f32>>;

    fn embed_batch(&self, inputs: &[String]) -> ContractResult<Vec<Vec<f32>>> {
        inputs.iter().map(|input| self.embed_text(input)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TinyProvider;

    impl EmbeddingProvider for TinyProvider {
        fn id(&self) -> &str {
            "tiny-test"
        }

        fn name(&self) -> &str {
            "Tiny Test"
        }

        fn dimension(&self) -> usize {
            2
        }

        fn max_input_chars(&self) -> usize {
            64
        }

        fn capabilities(&self) -> EmbeddingProviderCapabilities {
            EmbeddingProviderCapabilities::local_test()
        }

        fn embed_text(&self, input: &str) -> ContractResult<Vec<f32>> {
            Ok(vec![input.len() as f32, 1.0])
        }
    }

    #[test]
    fn provider_metadata_is_explicitly_local_and_offline() {
        let provider = TinyProvider;
        let capabilities = provider.capabilities();

        assert_eq!(provider.id(), "tiny-test");
        assert_eq!(provider.dimension(), 2);
        assert!(capabilities.local_only);
        assert!(!capabilities.requires_network);
        assert!(!capabilities.requires_api_key);
        assert!(!capabilities.telemetry);
    }

    #[test]
    fn validates_embedding_dimensions_and_normalization() {
        let result = EmbeddingResult {
            provider_id: "test".to_string(),
            dimensions: 2,
            vector: vec![0.6, 0.8],
            normalized: true,
        };

        assert!(result.validate_dimension(2).is_ok());
        assert!(result.is_unit_normalized(0.0001));
        assert!(result.validate_dimension(3).is_err());
    }
}
