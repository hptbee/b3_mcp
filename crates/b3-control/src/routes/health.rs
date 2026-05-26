use super::*;

pub(crate) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        offline_mode: true,
        telemetry_enabled: false,
    })
}

pub(crate) async fn status(
    State(state): State<ControlState>,
) -> Result<Json<StatusResponse>, ControlError> {
    let stats = storage_stats(&state).await?;
    let branch = current_branch(&state).await?;

    Ok(Json(StatusResponse {
        status: "ok",
        project_path: path_string(&state.project_path),
        database_path: path_string(&state.database_path),
        offline_mode: true,
        indexed_file_count: stats.files,
        symbol_count: stats.symbols,
        edge_count: stats.edges,
        current_branch: branch,
        mcp_runtime: RuntimeSummary::default(),
    }))
}
