use super::*;

pub(crate) async fn routes(
    State(state): State<ControlState>,
    Query(query): Query<RoutesQuery>,
) -> Result<Json<RoutesResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let routes = state
        .storage
        .lock()
        .await
        .routes(
            &project_id,
            &branch_id,
            query.framework.as_deref(),
            query.method.as_deref(),
            query.path.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(RouteDto::from)
        .collect();

    Ok(Json(RoutesResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        routes,
    }))
}

pub(crate) async fn components(
    State(state): State<ControlState>,
    Query(query): Query<ComponentsQuery>,
) -> Result<Json<ComponentsResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let components = state
        .storage
        .lock()
        .await
        .components(
            &project_id,
            &branch_id,
            query.framework.as_deref(),
            query.name.as_deref(),
            query.file.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(ComponentDto::from)
        .collect();

    Ok(Json(ComponentsResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        components,
    }))
}

pub(crate) async fn data_access(
    State(state): State<ControlState>,
    Query(query): Query<DataAccessQuery>,
) -> Result<Json<DataAccessResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let records = state
        .storage
        .lock()
        .await
        .data_access(
            &project_id,
            &branch_id,
            query.technology.as_deref(),
            query.kind.as_deref(),
            query.operation.as_deref(),
            query.file.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(DataAccessDto::from)
        .collect();

    Ok(Json(DataAccessResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        data_access: records,
    }))
}

pub(crate) async fn realtime(
    State(state): State<ControlState>,
    Query(query): Query<RealtimeQuery>,
) -> Result<Json<RealtimeResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let records = state
        .storage
        .lock()
        .await
        .realtime(
            &project_id,
            &branch_id,
            query.technology.as_deref(),
            query.kind.as_deref(),
            query.event.as_deref(),
            query.file.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(RealtimeDto::from)
        .collect();

    Ok(Json(RealtimeResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        realtime: records,
    }))
}

pub(crate) async fn messaging(
    State(state): State<ControlState>,
    Query(query): Query<MessagingQuery>,
) -> Result<Json<MessagingResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let records = state
        .storage
        .lock()
        .await
        .messaging(
            &project_id,
            &branch_id,
            query.technology.as_deref(),
            query.kind.as_deref(),
            query.topic.as_deref(),
            query.queue.as_deref(),
            query.routing_key.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(MessagingDto::from)
        .collect();

    Ok(Json(MessagingResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        messaging: records,
    }))
}

pub(crate) async fn infrastructure(
    State(state): State<ControlState>,
    Query(query): Query<InfrastructureQuery>,
) -> Result<Json<InfrastructureResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let records = state
        .storage
        .lock()
        .await
        .infrastructure(
            &project_id,
            &branch_id,
            query.technology.as_deref(),
            query.kind.as_deref(),
            query.name.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(InfrastructureDto::from)
        .collect();

    Ok(Json(InfrastructureResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        infrastructure: records,
    }))
}

pub(crate) async fn wpf(
    State(state): State<ControlState>,
    Query(query): Query<WpfQuery>,
) -> Result<Json<WpfResponse>, ControlError> {
    let project_id = query.project_id.unwrap_or_else(|| "default".to_string());
    let branch_id = query.branch_id.unwrap_or_else(|| "main".to_string());
    let limit = bounded_limit(query.limit);
    let records = state
        .storage
        .lock()
        .await
        .wpf(
            &project_id,
            &branch_id,
            query.kind.as_deref(),
            query.binding.as_deref(),
            query.command.as_deref(),
            limit,
        )
        .map_err(ControlError::internal)?
        .into_iter()
        .map(WpfDto::from)
        .collect();

    Ok(Json(WpfResponse {
        status: "ok".to_string(),
        project_id,
        branch_id,
        wpf: records,
    }))
}
