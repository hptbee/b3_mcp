use super::*;

pub(crate) async fn edit_preview(
    State(state): State<ControlState>,
    payload: Result<Json<EditRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<b3_core::EditPlan>, ControlError> {
    let request = edit_request_with_defaults(state.clone(), payload)?;
    let storage = state.storage.lock().await;
    SymbolicEditEngine::new(&*storage)
        .preview_edit(request)
        .map(Json)
        .map_err(|error| ControlError::bad_request(error.to_string()))
}

pub(crate) async fn edit_apply(
    State(state): State<ControlState>,
    payload: Result<Json<EditRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<b3_core::EditApplyResult>, ControlError> {
    let request = edit_request_with_defaults(state.clone(), payload)?;
    let storage = state.storage.lock().await;
    SymbolicEditEngine::new(&*storage)
        .apply_edit(request)
        .map(Json)
        .map_err(|error| ControlError::bad_request(error.to_string()))
}

pub(crate) async fn rename_preview(
    State(state): State<ControlState>,
    payload: Result<Json<RenameRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<b3_core::RenamePlan>, ControlError> {
    let request = rename_request_with_defaults(state.clone(), payload)?;
    let storage = state.storage.lock().await;
    RenameRefactorEngine::new(&*storage)
        .preview_rename(request)
        .map(Json)
        .map_err(|error| ControlError::bad_request(error.to_string()))
}

pub(crate) async fn rename_apply(
    State(state): State<ControlState>,
    payload: Result<Json<RenameRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<b3_core::RenameApplyResult>, ControlError> {
    let request = rename_request_with_defaults(state.clone(), payload)?;
    let storage = state.storage.lock().await;
    RenameRefactorEngine::new(&*storage)
        .apply_rename(request)
        .map(Json)
        .map_err(|error| ControlError::bad_request(error.to_string()))
}

fn edit_request_with_defaults(
    state: ControlState,
    payload: Result<Json<EditRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<EditRequest, ControlError> {
    let mut request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    if request.project_path.is_none() {
        request.project_path = Some(path_string(&state.project_path));
    }
    if request.database_path.is_none() {
        request.database_path = Some(path_string(&state.database_path));
    }
    Ok(request)
}

fn rename_request_with_defaults(
    state: ControlState,
    payload: Result<Json<RenameRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<RenameRequest, ControlError> {
    let mut request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    if request.project_path.is_none() {
        request.project_path = Some(path_string(&state.project_path));
    }
    if request.database_path.is_none() {
        request.database_path = Some(path_string(&state.database_path));
    }
    Ok(request)
}
