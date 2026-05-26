use super::*;

pub(crate) async fn config(State(state): State<ControlState>) -> Json<Value> {
    let config = state.app_config.as_ref();
    Json(json!({
        "offline": {
            "local_storage_only": config.offline.local_storage_only,
            "local_embeddings_by_default": config.offline.local_embeddings_by_default,
            "external_providers_enabled_by_default": config.offline.external_providers.enabled_by_default
        },
        "project": {
            "root_path": config.project.root_path,
            "branch_aware": config.project.branch_aware
        },
        "indexing": {
            "enabled": config.indexing.enabled,
            "parser_subprocess_isolation": config.indexing.parser_subprocess_isolation,
            "parser_isolation_mode": parser_isolation_mode(config.indexing.parser_isolation_mode.clone()),
            "parser_timeout_ms": config.indexing.parser_timeout_ms,
            "parser_max_retries": config.indexing.parser_max_retries,
            "parser_worker_path": config.indexing.parser_worker_path,
            "watch_files": config.indexing.watch_files,
            "max_parallel_workers": config.indexing.max_parallel_workers,
            "debounce_ms": config.indexing.debounce_ms,
            "max_watch_batch_size": config.indexing.max_watch_batch_size,
            "ignore_patterns": config.indexing.ignore_patterns
        },
        "retrieval": {
            "max_graph_depth": config.retrieval.max_graph_depth,
            "max_tokens": config.retrieval.max_tokens,
            "bm25_enabled": config.retrieval.bm25_enabled,
            "semantic_enabled": config.retrieval.semantic_enabled,
            "local_qdrant_enabled": config.retrieval.local_qdrant_enabled
        },
        "ui": {
            "control_server_enabled": config.ui.control_server_enabled,
            "websocket_enabled": config.ui.websocket_enabled,
            "bind_address": config.ui.bind_address
        },
        "language_backends": {
            "selection_policy": config.language_backends.selection_policy,
            "enable_lsp": config.language_backends.enable_lsp,
            "enable_experimental_languages": config.language_backends.enable_experimental_languages
        },
        "lsp": {
            "enabled": config.lsp.enabled,
            "startup_timeout_ms": config.lsp.startup_timeout_ms,
            "request_timeout_ms": config.lsp.request_timeout_ms,
            "stderr_capture_bytes": config.lsp.stderr_capture_bytes,
            "servers": config.lsp.servers.iter().map(|server| json!({
                "language_id": server.language_id,
                "command": server.command,
                "args": server.args,
                "enabled": server.enabled
            })).collect::<Vec<_>>()
        }
    }))
}

pub(crate) async fn validate_config(
    payload: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ControlError> {
    let _ = payload.map_err(|rejection| ControlError::bad_request(rejection.body_text()))?;
    Ok(Json(json!({
        "valid": true,
        "mutation_supported": false,
        "message": "config validation skeleton accepted JSON; mutation is deferred"
    })))
}
