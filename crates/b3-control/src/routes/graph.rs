use super::*;

pub(crate) async fn graph_summary(
    State(state): State<ControlState>,
) -> Result<Json<GraphSummaryResponse>, ControlError> {
    let summary = state
        .storage
        .lock()
        .await
        .graph_summary(None, None)
        .map_err(ControlError::internal)?;

    Ok(Json(GraphSummaryResponse {
        project_id: summary.project_id.unwrap_or_else(|| "default".to_string()),
        branch_id: summary.branch_id.unwrap_or_else(|| "main".to_string()),
        node_count: summary.node_count,
        edge_count: summary.edge_count,
        symbol_count: summary.symbol_count,
        file_count: summary.file_count,
        edge_type_counts: summary
            .edge_type_counts
            .into_iter()
            .map(CountDto::from)
            .collect(),
        node_kind_counts: summary
            .node_kind_counts
            .into_iter()
            .map(CountDto::from)
            .collect(),
        centrality_snapshot_status: if summary.centrality_snapshot_count > 0 {
            "available".to_string()
        } else {
            "empty".to_string()
        },
        max_depth: MAX_GRAPH_DEPTH,
        branch_aware: true,
        full_graph_dump_included: false,
        partial: false,
        message: "summary is read from local SQLite graph storage".to_string(),
    }))
}

pub(crate) async fn graph_neighbors(
    State(state): State<ControlState>,
    payload: Result<Json<GraphNeighborsRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphNeighborsResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let max_depth = checked_graph_depth(request.max_depth.or(request.depth))?;
    let limit = checked_graph_limit(request.limit)?;
    let min_confidence = checked_confidence(request.min_confidence)?;
    let edge_types = request.edge_types.clone().unwrap_or_default();
    let direction = request.direction.unwrap_or_default();

    let storage = state.storage.lock().await;
    let seed = resolve_seed_node(
        &storage,
        &project_id,
        &branch_id,
        request
            .seed_node_id
            .as_deref()
            .or(request.node_id.as_deref()),
        request.seed_symbol_id.as_deref(),
    )?;

    let Some(seed) = seed else {
        return Ok(Json(GraphNeighborsResponse {
            project_id,
            branch_id,
            seed_node_id: None,
            depth: max_depth,
            limit,
            nodes: Vec::new(),
            edges: Vec::new(),
            partial: false,
            full_graph_dump_included: false,
            message: "seed node or symbol was not found".to_string(),
        }));
    };

    let graph = bounded_neighbor_graph(
        &storage,
        &project_id,
        &branch_id,
        &seed,
        direction,
        &edge_types,
        max_depth,
        min_confidence,
        limit,
    )?;

    Ok(Json(GraphNeighborsResponse {
        project_id,
        branch_id,
        seed_node_id: Some(seed.id),
        depth: max_depth,
        limit,
        nodes: graph.nodes,
        edges: graph.edges,
        partial: graph.partial,
        full_graph_dump_included: false,
        message: "bounded neighbor graph read from local SQLite".to_string(),
    }))
}

pub(crate) async fn graph_path(
    State(state): State<ControlState>,
    payload: Result<Json<GraphPathRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphPathResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let max_depth = checked_graph_depth(request.max_depth.or(request.depth))?;
    let limit = checked_graph_limit(request.limit)?;
    let min_confidence = checked_confidence(request.min_confidence)?;
    let edge_types = request.edge_types.clone().unwrap_or_default();
    let source = request
        .source_node_id
        .as_deref()
        .or(request.from_node_id.as_deref())
        .ok_or_else(|| ControlError::bad_request("source_node_id is required"))?;
    let target = request
        .target_node_id
        .as_deref()
        .or(request.to_node_id.as_deref())
        .ok_or_else(|| ControlError::bad_request("target_node_id is required"))?;

    let storage = state.storage.lock().await;
    let response = shortest_path(
        &storage,
        &project_id,
        &branch_id,
        source,
        target,
        &edge_types,
        max_depth,
        min_confidence,
        limit,
    )?;
    Ok(Json(response))
}

pub(crate) async fn graph_cycles(
    State(state): State<ControlState>,
    payload: Result<Json<GraphCyclesRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphCyclesResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let limit = checked_graph_limit(request.limit)?;
    let min_confidence = checked_confidence(request.min_confidence)?;
    let edge_types = request.edge_types.clone().unwrap_or_default();
    let storage = state.storage.lock().await;
    let edges = filtered_edges(
        storage.graph_edges_scoped(&project_id, &branch_id, min_confidence, limit + 1)?,
        &edge_types,
    );
    let bounded_warning = (edges.len() > limit)
        .then(|| "edge scan reached the requested limit; cycle results may be partial".to_string());
    let cycles = cycle_groups(edges.into_iter().take(limit).collect());

    Ok(Json(GraphCyclesResponse {
        project_id,
        branch_id,
        cycle_count: cycles.len(),
        scc_groups: cycles,
        bounded_warning,
        partial: false,
    }))
}

pub(crate) async fn graph_centrality(
    State(state): State<ControlState>,
    payload: Result<Json<GraphCentralityRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<GraphCentralityResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;
    let project_id = request.scope.project_id.clone();
    let branch_id = request.scope.branch_id_or_default();
    let limit = checked_graph_limit(request.limit)?;
    let rows = state
        .storage
        .lock()
        .await
        .centrality_snapshot(&project_id, &branch_id, limit)
        .map_err(ControlError::internal)?;

    let message = if rows.is_empty() {
        "no cached centrality snapshot is available; not computing automatically".to_string()
    } else {
        "cached centrality snapshot returned from local SQLite".to_string()
    };

    Ok(Json(GraphCentralityResponse {
        project_id,
        branch_id,
        nodes: rows.into_iter().map(GraphCentralityNodeDto::from).collect(),
        calculated: !message.starts_with("no cached"),
        message,
    }))
}

pub(crate) async fn savings_summary(
    State(state): State<ControlState>,
) -> Result<Json<SavingsSummaryResponse>, ControlError> {
    let summary = state
        .storage
        .lock()
        .await
        .savings_summary()
        .map_err(ControlError::internal)?;
    Ok(Json(SavingsSummaryResponse::from(summary)))
}
