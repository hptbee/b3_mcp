use super::*;

pub(crate) async fn vector_status(
    State(state): State<ControlState>,
) -> Result<Json<Value>, ControlError> {
    let stats = state
        .storage
        .lock()
        .await
        .vector_stats()
        .map_err(ControlError::internal)?;
    let registry = b3_embeddings::EmbeddingProviderRegistry::offline_default();
    let local_hash_available = registry.get(b3_embeddings::LOCAL_HASH_PROVIDER_ID).is_ok();
    Ok(Json(json!({
        "status": "ok",
        "architecture_available": true,
        "enabled": state.app_config.embedding.enabled,
        "provider": state.app_config.embedding.provider_id.as_str(),
        "dimension": state.app_config.embedding.dimension,
        "documents": stats.documents,
        "vectors": stats.vectors,
        "local_only": true,
        "local_hash_provider_available": local_hash_available,
        "storage_available": true,
        "external_plugins_enabled": state.app_config.embedding.external_plugins_enabled,
        "semantic_search_available": true,
        "semantic_search_ready": true,
        "vector_search_ready": true,
        "hybrid_ranking_available": true,
        "local_hybrid_search_api_available": true,
        "mcp_semantic_search_tool_available": true,
        "quality_benchmark_ready": false,
        "cross_project_semantic_search": false,
        "hosted_vector_database_required": false,
        "openai_api_required": false,
        "cloud_embedding_api_required": false,
        "model_download_required": false,
        "telemetry_enabled": false
    })))
}

pub(crate) async fn vector_providers() -> Json<Value> {
    let registry = b3_embeddings::EmbeddingProviderRegistry::offline_default();
    let providers = registry
        .available_providers()
        .into_iter()
        .map(|provider| {
            json!({
                "id": provider.id,
                "name": provider.name,
                "kind": format!("{:?}", provider.kind),
                "dimension": provider.dimension,
                "local_only": provider.local_only,
                "deterministic": provider.deterministic,
                "batch": provider.batch,
                "requires_network": false,
                "requires_api_key": false,
                "downloads_models": false,
                "telemetry": false
            })
        })
        .collect::<Vec<_>>();
    Json(json!({
        "status": "ok",
        "providers": providers,
        "external_plugins_enabled": false
    }))
}

pub(crate) async fn vector_stats(
    State(state): State<ControlState>,
) -> Result<Json<Value>, ControlError> {
    let stats = state
        .storage
        .lock()
        .await
        .vector_stats()
        .map_err(ControlError::internal)?;
    Ok(Json(json!({
        "status": "ok",
        "documents": stats.documents,
        "vectors": stats.vectors,
        "providers": stats.providers,
        "dimensions": stats.dimensions,
        "source_kind_counts": stats.source_kind_counts,
        "language_counts": stats.language_counts,
        "framework_counts": stats.framework_counts,
        "local_only": true,
        "hosted_vector_database_required": false
    })))
}
