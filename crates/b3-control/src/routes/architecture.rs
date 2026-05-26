use super::*;

pub(crate) async fn architecture_status() -> Json<ArchitectureCapabilityStatus> {
    Json(ArchitectureCapabilityStatus::default())
}

pub(crate) async fn architecture_groups(
    State(state): State<ControlState>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    Ok(Json(json!({
        "status": "ok",
        "groups": federation.groups(),
        "registry_path": path_string(&state.registry_path),
        "local_only": true,
        "federation_ready": true,
        "matching_ready": true,
        "route_matching_ready": true,
        "messaging_matching_ready": true,
        "package_contract_infra_matching_ready": true,
        "group_impact_ready": true,
        "group_context_pack_ready": true,
        "service_map_ready": true,
        "architecture_graph_api_ready": true,
        "architecture_graph_ui_ready": false
    })))
}

pub(crate) async fn architecture_group_status(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let context = federation
        .resolve_context(&group_id)
        .map_err(federation_error)?;
    Ok(Json(json!({
        "status": "ok",
        "group": context,
        "federation_ready": true,
        "matching_ready": true,
        "route_matching_ready": true,
        "messaging_matching_ready": true,
        "package_contract_infra_matching_ready": true,
        "group_impact_ready": true,
        "group_context_pack_ready": true,
        "service_map_ready": true,
        "architecture_graph_api_ready": true,
        "architecture_graph_ui_ready": false,
        "local_only": true
    })))
}

pub(crate) async fn architecture_group_summary(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let summary = federation.summary(&group_id).map_err(federation_error)?;
    Ok(Json(json!({
        "status": "ok",
        "summary": summary,
        "federation_ready": true,
        "matching_ready": true,
        "route_matching_ready": true,
        "messaging_matching_ready": true,
        "package_contract_infra_matching_ready": true,
        "group_impact_ready": true,
        "group_context_pack_ready": true,
        "service_map_ready": true,
        "architecture_graph_api_ready": true,
        "architecture_graph_ui_ready": false,
        "local_only": true
    })))
}

pub(crate) async fn architecture_group_impact(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
    payload: Result<Json<GroupImpactRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let report = federation
        .group_impact(&group_id, request)
        .map_err(federation_error)?;
    Ok(Json(json!(report)))
}

pub(crate) async fn architecture_group_graph(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
    Query(query): Query<ArchitectureGraphQuery>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let report = federation
        .architecture_graph(&group_id, query.into_request())
        .map_err(federation_error)?;
    Ok(Json(json!(report)))
}

pub(crate) async fn architecture_group_service_map(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
    Query(query): Query<ServiceMapQuery>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let report = federation
        .service_map(&group_id, query.into_request())
        .map_err(federation_error)?;
    Ok(Json(json!(report)))
}

pub(crate) async fn architecture_group_dependency_matches(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
    Query(query): Query<DependencyMatchesQuery>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let report = federation
        .dependency_matches(&group_id, query.into_options())
        .map_err(federation_error)?;
    Ok(Json(json!({
        "status": "ok",
        "group_id": report.group_id,
        "group_name": report.group_name,
        "matching_kind": report.matching_kind,
        "match_count": report.match_count,
        "matches": report.matches,
        "warnings": report.warnings,
        "local_only": report.local_only,
        "federation_ready": report.federation_ready,
        "dependency_matching_ready": report.dependency_matching_ready,
        "package_contract_infra_matching_ready": report.dependency_matching_ready,
        "branch": report.branch
    })))
}

pub(crate) async fn architecture_group_message_matches(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
    Query(query): Query<MessageMatchesQuery>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let report = federation
        .message_matches(&group_id, query.into_options())
        .map_err(federation_error)?;
    Ok(Json(json!({
        "status": "ok",
        "group_id": report.group_id,
        "group_name": report.group_name,
        "matching_kind": report.matching_kind,
        "match_count": report.match_count,
        "matches": report.matches,
        "warnings": report.warnings,
        "local_only": report.local_only,
        "federation_ready": report.federation_ready,
        "messaging_matching_ready": report.messaging_matching_ready,
        "branch": report.branch
    })))
}

pub(crate) async fn architecture_group_route_matches(
    State(state): State<ControlState>,
    Path(group_id): Path<String>,
    Query(query): Query<RouteMatchesQuery>,
) -> Result<Json<Value>, ControlError> {
    let federation = GroupFederation::from_registry_path((*state.registry_path).clone())
        .map_err(ControlError::internal)?;
    let report = federation
        .route_matches(&group_id, query.into_options())
        .map_err(federation_error)?;
    Ok(Json(json!({
        "status": "ok",
        "group_id": report.group_id,
        "group_name": report.group_name,
        "matching_kind": report.matching_kind,
        "match_count": report.match_count,
        "matches": report.matches,
        "warnings": report.warnings,
        "local_only": report.local_only,
        "federation_ready": report.federation_ready,
        "route_matching_ready": report.route_matching_ready,
        "branch": report.branch
    })))
}

fn federation_error(error: ContractError) -> ControlError {
    let message = error.to_string();
    if message.contains("group not found") {
        ControlError::not_found(message)
    } else {
        ControlError::bad_request(message)
    }
}
