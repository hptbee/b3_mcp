use super::*;

pub(crate) mod architecture;
pub(crate) mod capabilities;
pub(crate) mod config;
pub(crate) mod domain;
pub(crate) mod editing;
pub(crate) mod events;
pub(crate) mod graph;
pub(crate) mod health;
pub(crate) mod indexing;
pub(crate) mod languages;
pub(crate) mod search;
pub(crate) mod status;
pub(crate) mod vector;

pub(crate) fn router() -> Router<ControlState> {
    Router::new()
        .route("/health", get(health::health))
        .route("/api/status", get(health::status))
        .route("/api/projects", get(status::projects))
        .route("/api/project", get(status::project))
        .route("/api/index/preview", post(indexing::index_preview))
        .route("/api/index/run", post(indexing::index_run))
        .route("/api/index/reindex", post(indexing::index_reindex))
        .route("/api/index/status", get(indexing::index_status))
        .route("/api/edit/preview", post(editing::edit_preview))
        .route("/api/edit/apply", post(editing::edit_apply))
        .route(
            "/api/refactor/rename/preview",
            post(editing::rename_preview),
        )
        .route("/api/refactor/rename/apply", post(editing::rename_apply))
        .route("/api/query/:operation", post(search::query_operation))
        .route("/api/search/hybrid", post(search::hybrid_search))
        .route("/api/graph/summary", get(graph::graph_summary))
        .route("/api/graph/neighbors", post(graph::graph_neighbors))
        .route("/api/graph/path", post(graph::graph_path))
        .route("/api/graph/cycles", post(graph::graph_cycles))
        .route("/api/graph/centrality", post(graph::graph_centrality))
        .route("/api/savings/summary", get(graph::savings_summary))
        .route("/api/diagnostics", get(capabilities::diagnostics))
        .route("/api/capabilities", get(capabilities::capabilities))
        .route(
            "/api/architecture/status",
            get(architecture::architecture_status),
        )
        .route(
            "/api/architecture/groups",
            get(architecture::architecture_groups),
        )
        .route(
            "/api/architecture/groups/:group_id/status",
            get(architecture::architecture_group_status),
        )
        .route(
            "/api/architecture/groups/:group_id/summary",
            get(architecture::architecture_group_summary),
        )
        .route(
            "/api/architecture/groups/:group_id/route-matches",
            get(architecture::architecture_group_route_matches),
        )
        .route(
            "/api/architecture/groups/:group_id/message-matches",
            get(architecture::architecture_group_message_matches),
        )
        .route(
            "/api/architecture/groups/:group_id/dependency-matches",
            get(architecture::architecture_group_dependency_matches),
        )
        .route(
            "/api/architecture/groups/:group_id/impact",
            post(architecture::architecture_group_impact),
        )
        .route(
            "/api/architecture/groups/:group_id/graph",
            get(architecture::architecture_group_graph),
        )
        .route(
            "/api/architecture/groups/:group_id/service-map",
            get(architecture::architecture_group_service_map),
        )
        .route("/api/vector/status", get(vector::vector_status))
        .route("/api/vector/providers", get(vector::vector_providers))
        .route("/api/vector/stats", get(vector::vector_stats))
        .route("/api/languages", get(languages::languages))
        .route("/api/lsp/status", get(languages::lsp_status))
        .route("/api/lsp/servers", get(languages::lsp_servers))
        .route("/api/routes", get(domain::routes))
        .route("/api/components", get(domain::components))
        .route("/api/data-access", get(domain::data_access))
        .route("/api/realtime", get(domain::realtime))
        .route("/api/messaging", get(domain::messaging))
        .route("/api/infrastructure", get(domain::infrastructure))
        .route("/api/wpf", get(domain::wpf))
        .route("/api/config", get(config::config))
        .route("/api/config/validate", post(config::validate_config))
        .route("/api/events", get(events::events))
}
