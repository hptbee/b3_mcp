use super::*;

pub(crate) async fn index_run(
    State(state): State<ControlState>,
    payload: Option<Json<IndexRunRequest>>,
) -> Result<Json<ManualIndexSummary>, ControlError> {
    run_index_for_state(state, false, payload.map(|value| value.0))
        .await
        .map(Json)
}

pub(crate) async fn index_reindex(
    State(state): State<ControlState>,
    payload: Option<Json<IndexRunRequest>>,
) -> Result<Json<ManualIndexSummary>, ControlError> {
    run_index_for_state(state, true, payload.map(|value| value.0))
        .await
        .map(Json)
}

pub(crate) async fn index_preview(
    State(state): State<ControlState>,
    payload: Option<Json<IndexRunRequest>>,
) -> Result<Json<ScopePreview>, ControlError> {
    let request = payload.map(|value| value.0).unwrap_or_default();
    let scope_text = request.scope.unwrap_or_else(|| "project".to_string());
    let mut scope = parse_scope(&scope_text).map_err(ControlError::from_scope)?;
    scope.dry_run = true;
    scope.force = request.force.unwrap_or(false);
    scope.project_id = Some("default".to_string());
    scope.branch_id = Some("main".to_string());
    let storage = state.storage.lock().await;
    let provider = StorageScopeTargetProvider { storage: &storage };
    let plan = plan_scope(
        &state.project_path,
        "default",
        "main",
        scope,
        &IndexerConfig::default().ignore,
        &provider,
    )
    .map_err(ControlError::from_scope)?;
    Ok(Json(plan.preview))
}

pub(crate) async fn index_status(
    State(state): State<ControlState>,
) -> Result<Json<IndexStatusResponse>, ControlError> {
    Ok(Json(state.index_status.lock().await.clone()))
}
