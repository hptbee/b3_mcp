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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCapabilityDescriptor {
    pub plugin_id: PluginId,
    pub capability: PluginCapability,
    pub local_only: bool,
}

pub trait PluginRegistry {
    /// Discover installed plugin metadata without loading or activating plugin code.
    fn discover(&self) -> ContractResult<Vec<PluginMetadata>>;

    /// Return capability descriptors suitable for offline-first feature negotiation.
    fn capabilities(&self) -> ContractResult<Vec<PluginCapabilityDescriptor>>;

    /// Find plugins advertising a specific capability.
    fn by_capability(&self, capability: PluginCapability) -> ContractResult<Vec<PluginMetadata>>;

    fn lifecycle_state(&self, plugin_id: &PluginId) -> ContractResult<PluginLifecycleState>;
}

pub trait PluginLifecycle {
    /// Load a plugin boundary without making it active in request paths.
    fn load(&self, plugin_id: &PluginId) -> ContractResult<()>;

    /// Activate a loaded plugin after policy checks, timeout setup, and cancellation wiring.
    fn activate(&self, plugin_id: &PluginId) -> ContractResult<()>;

    /// Pause an active plugin without unloading its local metadata.
    fn pause(&self, plugin_id: &PluginId) -> ContractResult<()>;

    /// Unload plugin code and release local resources.
    fn unload(&self, plugin_id: &PluginId) -> ContractResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryPluginRegistry {
        plugins: Vec<PluginMetadata>,
    }

    impl PluginRegistry for MemoryPluginRegistry {
        fn discover(&self) -> ContractResult<Vec<PluginMetadata>> {
            Ok(self.plugins.clone())
        }

        fn capabilities(&self) -> ContractResult<Vec<PluginCapabilityDescriptor>> {
            Ok(self
                .plugins
                .iter()
                .flat_map(|plugin| {
                    plugin
                        .capabilities
                        .iter()
                        .map(|capability| PluginCapabilityDescriptor {
                            plugin_id: plugin.id.clone(),
                            capability: *capability,
                            local_only: !plugin.external_integration.plugin_supported,
                        })
                })
                .collect())
        }

        fn by_capability(
            &self,
            capability: PluginCapability,
        ) -> ContractResult<Vec<PluginMetadata>> {
            Ok(self
                .plugins
                .iter()
                .filter(|plugin| plugin.capabilities.contains(&capability))
                .cloned()
                .collect())
        }

        fn lifecycle_state(&self, _plugin_id: &PluginId) -> ContractResult<PluginLifecycleState> {
            Ok(PluginLifecycleState::Discovered)
        }
    }

    #[test]
    fn registry_discovers_capabilities_without_activation() {
        let registry = MemoryPluginRegistry {
            plugins: vec![PluginMetadata {
                id: PluginId::new("local-rust"),
                name: "Local Rust".to_string(),
                version: "0.1.0".to_string(),
                capabilities: vec![PluginCapability::LanguageExtraction],
                external_integration: ExternalIntegrationPolicy::local_only(),
                execution: PluginExecutionPolicy::default(),
            }],
        };

        let capabilities = registry.capabilities().expect("capabilities");
        let language_plugins = registry
            .by_capability(PluginCapability::LanguageExtraction)
            .expect("by capability");

        assert_eq!(capabilities.len(), 1);
        assert!(capabilities[0].local_only);
        assert_eq!(language_plugins.len(), 1);
        assert_eq!(
            registry
                .lifecycle_state(&PluginId::new("local-rust"))
                .expect("state"),
            PluginLifecycleState::Discovered
        );
    }
}
