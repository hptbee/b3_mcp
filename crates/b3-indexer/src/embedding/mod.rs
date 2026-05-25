//! Embedding planning for the indexing pipeline.
//!
//! This module prepares deterministic vector documents. It does not load
//! models, call APIs, or perform semantic ranking.

pub mod chunking;
pub mod planner;

pub use chunking::{ChunkCandidate, ChunkPlanner, ChunkPlannerConfig, ChunkSource};
pub use planner::{EmbeddingPlan, EmbeddingPlanner};
