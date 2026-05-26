use super::*;

pub(crate) async fn query_operation(
    Path(operation): Path<String>,
    State(state): State<ControlState>,
    payload: Result<Json<QueryApiRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<QueryApiResponse>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;

    match operation.as_str() {
        "find-symbol" => find_symbol(state, request).await,
        "search-code" => placeholder_query(operation, request),
        "find-callers" => placeholder_query(operation, request),
        "find-callees" => placeholder_query(operation, request),
        "related-symbols" => placeholder_query(operation, request),
        "impact-analysis" => placeholder_query(operation, request),
        "context-pack" => context_pack_placeholder(request),
        "trace-dependency" => placeholder_query(operation, request),
        "detect-cycles" => placeholder_query(operation, request),
        _ => Err(ControlError::not_found("unknown query operation")),
    }
}

pub(crate) async fn hybrid_search(
    State(state): State<ControlState>,
    payload: Result<Json<HybridSearchApiRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ControlError> {
    let request = payload
        .map_err(|rejection| ControlError::bad_request(rejection.body_text()))?
        .0;
    request.validate()?;

    let provider_id = request
        .provider_id
        .clone()
        .unwrap_or_else(|| state.app_config.embedding.provider_id.as_str().to_string());
    let dimension = request
        .dimension
        .unwrap_or(state.app_config.embedding.dimension);
    let mut hybrid = HybridSearchRequest::new(
        CoreQueryScope::new(
            ProjectId::new(
                request
                    .project_id
                    .clone()
                    .unwrap_or_else(|| "default".to_string()),
            ),
            BranchId::new(
                request
                    .branch_id
                    .clone()
                    .unwrap_or_else(|| "main".to_string()),
            ),
        ),
        request.query.clone(),
    );
    hybrid.provider_id = Some(provider_id.clone());
    hybrid.dimension = Some(dimension);
    hybrid.language = request.language.clone();
    hybrid.framework = request.framework.clone();
    hybrid.source_kind = request
        .source_kind
        .as_deref()
        .map(parse_source_kind)
        .transpose()?;
    hybrid.path_prefix = request.path_prefix.clone();
    hybrid.limit = request.limit.unwrap_or(10);
    hybrid.explain = request.explain.unwrap_or(false);
    hybrid.min_score = request.min_score;
    if let Some(weight) = request.lexical_weight {
        hybrid.lexical_weight = weight;
    }
    if let Some(weight) = request.vector_weight {
        hybrid.vector_weight = weight;
    }
    if let Some(weight) = request.metadata_weight {
        hybrid.metadata_weight = weight;
    }

    let storage = state.storage.lock().await;
    let response = HybridSearchEngine::new(&*storage, &*storage)
        .search(hybrid)
        .map_err(ControlError::from)?;

    Ok(Json(json!({
        "query": request.query,
        "results": response.results.into_iter().map(hybrid_result_json).collect::<Vec<_>>(),
        "warnings": response.warnings,
        "provider_id": provider_id,
        "dimension": dimension,
        "local_only": true,
        "semantic_search_ready": true,
        "hybrid_ranking": true
    })))
}

pub(crate) async fn find_symbol(
    state: ControlState,
    request: QueryApiRequest,
) -> Result<Json<QueryApiResponse>, ControlError> {
    let symbol = request
        .symbol
        .or(request.query)
        .ok_or_else(|| ControlError::bad_request("find-symbol requires symbol or query"))?;
    let project_id = ProjectId::new(request.scope.project_id);
    let symbols = state
        .storage
        .lock()
        .await
        .find_symbol(&project_id, &symbol)
        .map_err(ControlError::internal)?;
    let limit = bounded_limit(request.limit);
    let matches = symbols
        .into_iter()
        .take(limit)
        .map(|symbol| QueryMatch {
            id: symbol.id.as_str().to_string(),
            name: symbol.name,
            file_id: Some(symbol.file_id.as_str().to_string()),
            path: None,
            score: None,
        })
        .collect();

    Ok(Json(QueryApiResponse {
        operation: "find-symbol".to_string(),
        status: "ok",
        partial: false,
        message: None,
        matches,
        include_trace: request.include_trace.unwrap_or(false),
        trace: Vec::new(),
        full_file_dump_included: false,
        query_result: Some(QueryResultDto::from(QueryResult {
            summary: format!("symbol lookup for {symbol}"),
            returned_tokens: 0,
            expansion_handles: Vec::new(),
        })),
    }))
}

fn placeholder_query(
    operation: String,
    request: QueryApiRequest,
) -> Result<Json<QueryApiResponse>, ControlError> {
    let token_budget = request.token_budget.unwrap_or(DEFAULT_TOKEN_BUDGET);
    let query_text = request.query.or(request.symbol).unwrap_or_default();
    let query = QueryRequest::new(query_text, token_budget);
    let result = QueryResult {
        summary: format!(
            "{operation} is exposed through the control API; ranking engine integration is deferred"
        ),
        returned_tokens: 0,
        expansion_handles: Vec::new(),
    };

    Ok(Json(QueryApiResponse {
        operation,
        status: "not_implemented",
        partial: true,
        message: Some("query engine integration is deferred; no fake results returned".to_string()),
        matches: Vec::new(),
        include_trace: request.include_trace.unwrap_or(false),
        trace: Vec::new(),
        full_file_dump_included: false,
        query_result: Some(QueryResultDto {
            summary: result.summary,
            returned_tokens: result.returned_tokens,
            expansion_handles: result.expansion_handles,
            request_token_budget: Some(query.token_budget),
        }),
    }))
}
