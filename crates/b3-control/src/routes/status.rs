use super::*;

pub(crate) async fn projects(
    State(state): State<ControlState>,
) -> Result<Json<ProjectsResponse>, ControlError> {
    let roots = state
        .storage
        .lock()
        .await
        .project_roots()
        .map_err(ControlError::internal)?;
    let projects = if roots.is_empty() {
        vec![ProjectResponse {
            path: path_string(&state.project_path),
            active: true,
        }]
    } else {
        roots
            .into_iter()
            .map(|path| ProjectResponse {
                path,
                active: false,
            })
            .collect()
    };

    Ok(Json(ProjectsResponse { projects }))
}

pub(crate) async fn project(
    State(state): State<ControlState>,
) -> Result<Json<ProjectDetail>, ControlError> {
    let stats = storage_stats(&state).await?;
    Ok(Json(ProjectDetail {
        path: path_string(&state.project_path),
        database_path: path_string(&state.database_path),
        indexed_file_count: stats.files,
        symbol_count: stats.symbols,
        edge_count: stats.edges,
        offline_mode: true,
    }))
}
