//! Local embedding provider boundary.
//!
//! Core semantic search must work offline. Cloud embedding providers may be
//! added later as optional plugins, but they are not part of the default path.
//! This crate will host concrete local providers in later phases; for Phase 1.5
//! it re-exports the stable core contracts.

pub use b3_core::{
    EmbeddingConfig, EmbeddingProvider, EmbeddingRequest, EmbeddingResult,
    LocalEmbeddingProviderKind,
};
