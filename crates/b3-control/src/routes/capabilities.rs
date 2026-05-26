use super::*;

pub(crate) async fn diagnostics(State(state): State<ControlState>) -> Json<Value> {
    let (parse_failure_count, recent_parse_failures) = {
        let storage = state.storage.lock().await;
        (
            storage.parse_failure_count(None, None).unwrap_or(0),
            storage.recent_parse_failures(10).unwrap_or_default(),
        )
    };
    let config = &state.app_config.indexing;
    Json(json!({
        "status": "ok",
        "project_path": path_string(&state.project_path),
        "database_path": path_string(&state.database_path),
        "registry_path": path_string(&state.registry_path),
        "offline_mode": true,
        "telemetry_enabled": false,
        "source_upload_enabled": false,
        "parser": {
            "isolation_mode": parser_isolation_mode(config.parser_isolation_mode.clone()),
            "timeout_ms": config.parser_timeout_ms,
            "max_retries": config.parser_max_retries,
            "worker_path": config.parser_worker_path,
            "parse_failure_count": parse_failure_count,
            "recent_parse_failures": recent_parse_failures.into_iter().map(ParseFailureDto::from).collect::<Vec<_>>()
        },
        "known_limitations": [
            "query ranking integration is deferred",
            "parser subprocess mode is available but in-process mode remains the default",
            "frontend parse-failure dashboard is deferred"
        ]
    }))
}

pub(crate) async fn capabilities(State(state): State<ControlState>) -> Json<Value> {
    let language_registry = default_language_backend_registry();
    let lsp = LspBackend::from(&state.app_config.lsp);
    let architecture = ArchitectureCapabilityStatus::default();
    Json(json!({
        "product": PRODUCT_NAME,
        "offline_first": true,
        "free_by_default": true,
        "external_api_required": false,
        "telemetry_enabled": false,
        "vector_search": {
            "architecture_available": true,
            "semantic_search_available": true,
            "semantic_search_ready": true,
            "vector_search_ready": true,
            "hybrid_ranking_available": true,
            "local_hybrid_search_api_available": true,
            "mcp_semantic_search_tool_available": true,
            "quality_benchmark_ready": false,
            "cross_project_semantic_search": false,
            "storage_available": true,
            "provider": state.app_config.embedding.provider_id.as_str(),
            "enabled": state.app_config.embedding.enabled,
            "dimension": state.app_config.embedding.dimension,
            "local_only": true,
            "external_plugins_enabled": state.app_config.embedding.external_plugins_enabled,
            "hosted_vector_database_required": false,
            "openai_api_required": false,
            "cloud_embedding_api_required": false,
            "real_local_provider_phase": "10.1",
            "sqlite_vector_storage_phase": "10.2",
            "hybrid_ranking_phase": "10.3",
            "control_mcp_integration_phase": "10.4"
        },
        "architecture": architecture,
        "editing": {
            "phase": "13",
            "symbolic_editing_mvp": true,
            "control_preview_endpoint": "/api/edit/preview",
            "control_apply_endpoint": "/api/edit/apply",
            "rename_refactor_mvp": true,
            "rename_preview_endpoint": "/api/refactor/rename/preview",
            "rename_apply_endpoint": "/api/refactor/rename/apply",
            "dry_run_default": true,
            "explicit_apply_required": true,
            "backup_default": true,
            "single_file_default": true,
            "bounded_multi_file_rename": true,
            "mcp_tool_available": false,
            "rename_refactor_available": true,
            "ide_grade_semantic_rename": false,
            "local_only": true,
            "external_api_required": false,
            "telemetry_enabled": false
        },
        "quality_audit": {
            "phase": "17",
            "completed": true,
            "support_matrix_audited": true,
            "capability_reporting_audited": true,
            "fixture_coverage_audited": true,
            "metadata_consistency_audited": true,
            "secret_redaction_audited": true,
            "false_positive_guardrails_audited": true,
            "benchmark_audited": true,
            "support_levels": {
                "good": ["rust"],
                "basic_static": [
                    "javascript",
                    "typescript",
                    "jsx",
                    "tsx",
                    "csharp",
                    "go",
                    "python",
                    "java",
                    "kotlin",
                    "php",
                    "ruby",
                    "c",
                    "cpp",
                    "swift",
                    "objective_c",
                    "dart",
                    "yaml",
                    "json",
                    "toml",
                    "xml",
                    "html",
                    "css",
                    "scss",
                    "xaml",
                    "sql",
                    "ksql",
                    "env"
                ],
                "basic_hints": ["threejs_webgl"],
                "unsupported": ["architecture_graph_ui", "full_git_intelligence", "broad_refactor_engine"]
            },
            "runtime_validation_claimed": false,
            "compiler_grade_claimed": false,
            "ide_grade_refactor_claimed": false,
            "architecture_graph_ui_ready": false,
            "full_git_intelligence_ready": false,
            "local_only": true,
            "external_api_required": false,
            "telemetry_enabled": false,
            "mandatory_lsp_required": false
        },
        "mcp_runtime": RuntimeSummary::default(),
        "language_backend": {
            "tree_sitter": {
                "enabled": true,
                "best_supported_language": "rust",
                "parsed_languages": ["rust", "javascript", "typescript", "jsx", "tsx"],
                "static_parsed_languages": ["csharp", "go"],
                "backend_languages": {
                    "python": "basic_static",
                    "java": "basic_static",
                    "kotlin": "basic_static",
                    "php": "basic_static",
                    "ruby": "basic_static"
                },
                "detect_only_languages": []
            },
            "additional_backend_languages": {
                "available": true,
                "phase": "14",
                "support": "basic_static",
                "languages": {
                    "python": "basic",
                    "java": "basic",
                    "kotlin": "basic",
                    "php": "basic",
                    "ruby": "basic"
                },
                "features": {
                    "file_detection": true,
                    "project_detection": true,
                    "symbol_extraction": true,
                    "import_extraction": true,
                    "route_hints": true,
                    "data_access_hints": true,
                    "messaging_hints": true
                },
                "package_manager_execution_required": false,
                "compiler_execution_required": false,
                "runtime_execution_required": false,
                "language_server_required": false,
                "external_api_required": false,
                "telemetry_enabled": false
            },
            "go": {
                "available": true,
                "support": "basic_static",
                "runtime_execution_required": false,
                "go_toolchain_required": false,
                "module_download_required": false,
                "features": {
                    "go_file_detection": true,
                    "go_mod_detection": true,
                    "packages": true,
                    "imports": true,
                    "functions": true,
                    "methods": true,
                    "structs": true,
                    "interfaces": true,
                    "type_declarations": true,
                    "const_var_declarations": true,
                    "local_call_edges": true,
                    "http_route_hints": true
                },
                "deferred": ["type_checking", "interface_implementation_graph", "deep_framework_intelligence", "grpc_intelligence"]
            },
            "node_rest": {
                "available": true,
                "support": "basic",
                "frameworks": {
                    "express": "basic",
                    "nestjs": "basic",
                    "fastify": "basic"
                },
                "runtime_execution_required": false
            },
            "react_components": {
                "available": true,
                "support": "basic",
                "framework": "react",
                "runtime_execution_required": false,
                "features": {
                    "function_components": true,
                    "arrow_components": true,
                    "class_components": true,
                    "props_type_links": true,
                    "jsx_usages": true,
                    "hooks": true
                }
            },
            "nextjs": {
                "available": true,
                "support": "basic",
                "framework": "nextjs",
                "runtime_execution_required": false,
                "features": {
                    "package_detection": true,
                    "config_detection": true,
                    "app_router_routes": true,
                    "pages_router_routes": true,
                    "route_handlers": true,
                    "http_method_exports": true,
                    "use_client_boundaries": true
                }
            },
            "realtime": {
                "available": true,
                "support": "basic_static",
                "technologies": {
                    "websocket": "basic",
                    "socketio": "basic",
                    "signalr": "basic",
                    "rsocket": "basic"
                },
                "runtime_execution_required": false,
                "network_connection_required": false,
                "payload_schema_inference": false,
                "cross_project_event_matching": false
            },
            "messaging": {
                "available": true,
                "support": "basic_static",
                "technologies": {
                    "amqp": "basic",
                    "rabbitmq": "basic",
                    "kafka": "basic",
                    "google_pubsub": "basic",
                    "nestjs_messaging": "basic"
                },
                "broker_connection_required": false,
                "cloud_api_required": false,
                "runtime_discovery": false,
                "payload_schema_inference": false,
                "cross_project_matching": true
            },
            "infrastructure": {
                "available": true,
                "support": "basic_static",
                "technologies": {
                    "docker": "basic",
                    "docker_compose": "basic",
                    "kubernetes": "basic",
                    "terraform": "basic",
                    "gcp": "basic",
                    "gke": "basic"
                },
                "docker_execution_required": false,
                "kubectl_execution_required": false,
                "terraform_execution_required": false,
                "gcloud_execution_required": false,
                "cloud_api_required": false,
                "runtime_discovery": false,
                "cross_project_matching": true
            },
            "dotnet_desktop": {
                "available": true,
                "support": "basic_static",
                "technologies": {
                    "wpf": "basic",
                    "xaml": "basic"
                },
                "visual_studio_required": false,
                "msbuild_required": false,
                "dotnet_execution_required": false,
                "xaml_compiler_required": false,
                "runtime_execution_required": false,
                "binding_type_checking": false,
                "deep_mvvm_analysis": false
            },
            "systems_mobile_config_web_phase15": {
                "available": true,
                "phase": "15",
                "support": "basic_static",
                "systems_languages": {
                    "c": "basic",
                    "cpp": "basic",
                    "swift": "basic",
                    "objective_c": "basic",
                    "dart": "basic"
                },
                "config_files": {
                    "yaml": "basic",
                    "json": "basic",
                    "toml": "basic",
                    "xml": "basic"
                },
                "web_files": {
                    "html": "basic",
                    "css": "basic",
                    "scss": "basic",
                    "threejs_webgl": "basic_hints"
                },
                "data_files": {
                    "ksqldb": "basic"
                },
                "features": {
                    "file_detection": true,
                    "project_config_detection": true,
                    "symbol_extraction": true,
                    "import_include_reference_extraction": true,
                    "safe_config_key_paths": true,
                    "html_route_hints": true,
                    "css_asset_references": true,
                    "xaml_hardening": true,
                    "ksqldb_messaging_hints": true
                },
                "local_only": true,
                "package_manager_execution_required": false,
                "compiler_execution_required": false,
                "formatter_execution_required": false,
                "runtime_execution_required": false,
                "browser_execution_required": false,
                "webgl_execution_required": false,
                "broker_connection_required": false,
                "ksqldb_connection_required": false,
                "database_connection_required": false,
                "cloud_api_required": false,
                "external_api_required": false,
                "telemetry_enabled": false,
                "mandatory_lsp_required": false
            },
            "config_data_web_hardening_phase16": {
                "available": true,
                "phase": "16",
                "support": "basic_static_hardened",
                "technologies": {
                    "yaml": "basic_hardened",
                    "json": "basic_hardened",
                    "toml": "basic_hardened",
                    "xml": "basic_hardened",
                    "env": "basic_static_safe",
                    "html": "basic_hardened",
                    "css": "basic_hardened",
                    "scss": "basic_hardened",
                    "xaml": "basic_hardened",
                    "ksqldb": "basic_static",
                    "sql": "basic_static",
                    "threejs_webgl": "basic_static_hints"
                },
                "features": {
                    "secret_redaction": true,
                    "safe_env_example_parsing": true,
                    "real_env_value_redaction": true,
                    "config_reference_hints": true,
                    "html_template_route_hints": true,
                    "css_asset_and_media_hints": true,
                    "sql_table_reference_hints": true,
                    "ksqldb_topic_dependency_hints": true,
                    "threejs_shader_asset_hints": true
                },
                "local_only": true,
                "package_manager_execution_required": false,
                "compiler_execution_required": false,
                "formatter_execution_required": false,
                "runtime_execution_required": false,
                "browser_execution_required": false,
                "webgl_execution_required": false,
                "broker_connection_required": false,
                "database_connection_required": false,
                "ksqldb_connection_required": false,
                "cloud_api_required": false,
                "external_api_required": false,
                "telemetry_enabled": false,
                "mandatory_lsp_required": false
            },
            "lsp": {
                "available": true,
                "enabled": lsp.enabled,
                "status": lsp.status(),
                "local_only": true,
                "auto_start": false,
                "missing_servers_fatal": false
            }
        },
        "control_api": {
            "projects": true,
            "query": true,
            "graph": true,
            "config_read": true,
            "config_mutation": false,
            "languages": true,
            "lsp_status": true,
            "routes": true,
            "data_access": true,
            "realtime": true,
            "messaging": true,
            "infrastructure": true,
            "wpf": true,
            "vector_status": true,
            "vector_providers": true,
            "vector_stats": true,
            "hybrid_search": true,
            "architecture_status": true,
            "architecture_groups": true,
            "architecture_group_summary": true,
            "architecture_group_route_matches": true,
            "architecture_group_message_matches": true,
            "architecture_group_dependency_matches": true,
            "architecture_group_impact": true,
            "architecture_group_graph": true,
            "architecture_group_service_map": true,
            "events": "sse"
        },
        "language_backends": language_registry
    }))
}
