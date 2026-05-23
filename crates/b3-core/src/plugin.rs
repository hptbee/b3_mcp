//! Plugin extension contracts.
//!
//! Plugins are optional extension points for local language packs, graph
//! enrichers, ranking providers, embedding providers, and MCP tool extensions.
//! They must be discoverable by capability, cancellable, timeout-bound, and
//! disabled by default when they integrate with external services.

use crate::{ContractResult, ExternalIntegrationPolicy, PluginId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCapability {
    LanguageExtraction,
    GraphEnrichment,
    RankingProvider,
    EmbeddingProvider,
    McpToolExtension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginLifecycleState {
    Discovered,
    Loaded,
    Active,
    Paused,
    Unloaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginExecutionPolicy {
    pub timeout_ms: u64,
    pub cancellation_required: bool,
}

impl Default for PluginExecutionPolicy {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            cancellation_required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<PluginCapability>,
    pub external_integration: ExternalIntegrationPolicy,
    pub execution: PluginExecutionPolicy,
}

pub trait PluginRegistry {
    fn discover(&self) -> ContractResult<Vec<PluginMetadata>>;
    fn by_capability(
        &self,
        capability: PluginCapability,
    ) -> ContractResult<Vec<PluginMetadata>>;
    fn lifecycle_state(&self, plugin_id: &PluginId) -> ContractResult<PluginLifecycleState>;
}
