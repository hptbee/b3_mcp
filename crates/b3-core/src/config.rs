//! Layered configuration contracts.
//!
//! Defaults are offline-first: local storage, local embeddings, no required
//! external APIs, no hosted vector database, no remote telemetry, and no SaaS
//! authentication.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalIntegrationMode {
    DisabledByDefault,
    OptionalPlugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalIntegrationPolicy {
    pub plugin_supported: bool,
    pub enabled_by_default: bool,
}

impl ExternalIntegrationPolicy {
    pub const fn local_only() -> Self {
        Self {
            plugin_supported: false,
            enabled_by_default: false,
        }
    }

    pub const fn optional_plugin_disabled() -> Self {
        Self {
            plugin_supported: true,
            enabled_by_default: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflinePolicy {
    pub external_apis: ExternalIntegrationMode,
    pub hosted_vector_databases: ExternalIntegrationMode,
    pub remote_telemetry: ExternalIntegrationMode,
    pub saas_auth: ExternalIntegrationMode,
}

impl OfflinePolicy {
    pub const fn strict() -> Self {
        Self {
            external_apis: ExternalIntegrationMode::DisabledByDefault,
            hosted_vector_databases: ExternalIntegrationMode::DisabledByDefault,
            remote_telemetry: ExternalIntegrationMode::DisabledByDefault,
            saas_auth: ExternalIntegrationMode::DisabledByDefault,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineConfig {
    pub policy: OfflinePolicy,
    pub local_storage_only: bool,
    pub local_embeddings_by_default: bool,
    pub external_providers: ExternalIntegrationPolicy,
}

impl Default for OfflineConfig {
    fn default() -> Self {
        Self {
            policy: OfflinePolicy::strict(),
            local_storage_only: true,
            local_embeddings_by_default: true,
            external_providers: ExternalIntegrationPolicy::optional_plugin_disabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub root_path: String,
    pub branch_aware: bool,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            root_path: ".".to_string(),
            branch_aware: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingConfig {
    pub enabled: bool,
    pub parser_subprocess_isolation: bool,
    pub watch_files: bool,
    pub max_parallel_workers: usize,
    pub debounce_ms: u64,
    pub max_watch_batch_size: usize,
    pub ignore_patterns: Vec<String>,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            parser_subprocess_isolation: true,
            watch_files: false,
            max_parallel_workers: 1,
            debounce_ms: 500,
            max_watch_batch_size: 100,
            ignore_patterns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalEmbeddingProviderKind {
    Ollama,
    Gguf,
    SentenceTransformers,
    Candle,
    FastEmbed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingConfig {
    pub provider: LocalEmbeddingProviderKind,
    pub model: String,
    pub cloud_plugins: ExternalIntegrationPolicy,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: LocalEmbeddingProviderKind::FastEmbed,
            model: "local-default".to_string(),
            cloud_plugins: ExternalIntegrationPolicy::optional_plugin_disabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalConfig {
    pub max_graph_depth: u8,
    pub max_tokens: usize,
    pub bm25_enabled: bool,
    pub semantic_enabled: bool,
    pub local_qdrant_enabled: bool,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            max_graph_depth: 2,
            max_tokens: 8_000,
            bm25_enabled: true,
            semantic_enabled: true,
            local_qdrant_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphConfig {
    pub max_traversal_depth: u8,
    pub cycle_safe: bool,
    pub edge_confidence_required: bool,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            max_traversal_depth: 3,
            cycle_safe: true,
            edge_confidence_required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiConfig {
    pub control_server_enabled: bool,
    pub websocket_enabled: bool,
    pub bind_address: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            control_server_enabled: false,
            websocket_enabled: true,
            bind_address: "127.0.0.1:0".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppConfig {
    pub offline: OfflineConfig,
    pub project: ProjectConfig,
    pub indexing: IndexingConfig,
    pub retrieval: RetrievalConfig,
    pub embedding: EmbeddingConfig,
    pub graph: GraphConfig,
    pub ui: UiConfig,
}
