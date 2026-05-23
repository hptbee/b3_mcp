//! Thin MCP runtime boundary.
//!
//! This crate owns protocol-facing concerns only: tool names, request DTOs,
//! validation, error mapping, and calls into the query engine. Heavy indexing,
//! graph traversal, ranking, storage, embeddings, and UI logic stay behind this
//! boundary.

use b3_core::{
    BranchId, ContextPackResponse, ContractError, EdgeKind, FindCalleesResponse,
    FindCallersResponse, FindSymbolResponse, ImpactAnalysisResponse, ProjectId, QueryScope,
    RelatedSymbolsResponse, SearchCodeResponse, SymbolId,
};
use b3_query::{DependencyPath, LocalQueryEngine, QueryEngineConfig};
use b3_storage::SqliteStorage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
};

pub use b3_core::PRODUCT_NAME;

const MAX_LIMIT: usize = 100;
const MAX_DEPTH: usize = 16;
const MAX_TOKEN_BUDGET: usize = 64_000;
const MAX_CYCLE_NODES: usize = 2_000;
const JSONRPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInfo {
    pub name: &'static str,
    pub protocol: &'static str,
    pub boundary: RuntimeBoundary,
}

pub fn runtime_info() -> RuntimeInfo {
    RuntimeInfo {
        name: PRODUCT_NAME,
        protocol: "mcp",
        boundary: RuntimeBoundary::ProtocolOnly,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBootstrapConfig {
    pub project_path: PathBuf,
    pub database_path: PathBuf,
}

impl RuntimeBootstrapConfig {
    pub fn local_project(project_path: impl Into<PathBuf>) -> Self {
        let project_path = project_path.into();
        let database_path = project_path.join(".b3").join("b3.db");
        Self {
            project_path,
            database_path,
        }
    }
}

pub fn serve_local_stdio(config: RuntimeBootstrapConfig) -> Result<(), String> {
    let storage = SqliteStorage::open(&config.database_path).map_err(|error| error.message)?;
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let router = McpQueryToolRouter::new(engine);
    serve_stdio(router, std::io::stdin().lock(), std::io::stdout())
}

pub fn serve_stdio<E, R, W>(
    router: McpQueryToolRouter<E>,
    reader: R,
    mut writer: W,
) -> Result<(), String>
where
    E: QueryToolExecutor,
    R: BufRead,
    W: Write,
{
    for line in reader.lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }

        let outcome = handle_json_rpc_line(&router, &line)?;
        if let Some(response) = outcome.response {
            serde_json::to_writer(&mut writer, &response).map_err(|error| error.to_string())?;
            writer.write_all(b"\n").map_err(|error| error.to_string())?;
            writer.flush().map_err(|error| error.to_string())?;
        }
        if outcome.shutdown {
            break;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonRpcOutcome {
    pub response: Option<Value>,
    pub shutdown: bool,
}

pub fn handle_json_rpc_line<E>(
    router: &McpQueryToolRouter<E>,
    line: &str,
) -> Result<JsonRpcOutcome, String>
where
    E: QueryToolExecutor,
{
    let request: JsonRpcRequest = serde_json::from_str(line).map_err(|error| error.to_string())?;
    Ok(handle_json_rpc_request(router, request))
}

pub fn handle_json_rpc_request<E>(
    router: &McpQueryToolRouter<E>,
    request: JsonRpcRequest,
) -> JsonRpcOutcome
where
    E: QueryToolExecutor,
{
    let id = request.id.clone();
    let response = match request.method.as_str() {
        "initialize" => Some(json_rpc_result(id, initialize_result())),
        "ping" => Some(json_rpc_result(id, json!({}))),
        "tools/list" => Some(json_rpc_result(id, tools_list_result())),
        "tools/call" => Some(match dispatch_tool_call(router, request.params) {
            Ok(result) => json_rpc_result(id, result),
            Err(error) => json_rpc_tool_error(id, -32602, error),
        }),
        "shutdown" => Some(json_rpc_result(id, Value::Null)),
        "notifications/initialized" => None,
        "exit" => None,
        _ => Some(json_rpc_error(id, -32601, "method not found")),
    };
    let shutdown = matches!(request.method.as_str(), "shutdown" | "exit");

    JsonRpcOutcome { response, shutdown }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBoundary {
    ProtocolOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponsibility {
    StdioTransport,
    JsonRpc,
    ToolRouting,
    Streaming,
    Cancellation,
    SessionLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryToolName {
    FindSymbol,
    SearchCode,
    FindCallers,
    FindCallees,
    RelatedSymbols,
    ImpactAnalysis,
    GetContextPack,
    TraceDependency,
    DetectCycles,
    SavingsReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeDto {
    pub project_id: String,
    pub branch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindSymbolRequest {
    pub scope: ScopeDto,
    pub query: String,
    pub limit: Option<usize>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchCodeRequest {
    pub scope: ScopeDto,
    pub query: String,
    pub limit: Option<usize>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindCallersRequest {
    pub scope: ScopeDto,
    pub symbol_id: String,
    pub max_depth: Option<usize>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindCalleesRequest {
    pub scope: ScopeDto,
    pub symbol_id: String,
    pub max_depth: Option<usize>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedSymbolsRequest {
    pub scope: ScopeDto,
    pub symbol_id: String,
    pub max_depth: Option<usize>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAnalysisRequest {
    pub scope: ScopeDto,
    pub symbol_id: String,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPackRequest {
    pub scope: ScopeDto,
    pub query: String,
    pub token_budget: usize,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceDependencyRequest {
    pub scope: ScopeDto,
    pub source_symbol_id: String,
    pub target_symbol_id: String,
    pub edge_filters: Vec<String>,
    pub max_depth: Option<usize>,
    pub min_confidence: Option<u16>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectCyclesRequest {
    pub scope: ScopeDto,
    pub edge_filters: Vec<String>,
    pub max_nodes: Option<usize>,
    pub min_confidence: Option<u16>,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsReportRequest {
    pub scope: ScopeDto,
    pub include_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceDependencyResponse {
    pub found: bool,
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub path_length: usize,
    pub confidence_summary: Option<u16>,
    pub trace_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectCyclesResponse {
    pub cycles: Vec<Vec<String>>,
    pub scanned_nodes: usize,
    pub summary_count: usize,
    pub trace_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavingsReportResponse {
    pub estimated_tokens_saved: usize,
    pub returned_tokens: usize,
    pub avoided_file_reads: usize,
    pub avoided_search_calls: usize,
    pub trace_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub purpose: String,
    pub input_schema: String,
    pub output_schema: String,
    pub example: String,
    pub token_saving_behavior: String,
    pub trace_behavior: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolError {
    pub code: String,
    pub message: String,
}

pub type McpToolResult<T> = Result<T, McpToolError>;

pub trait QueryToolExecutor {
    fn find_symbol(&self, request: FindSymbolRequest) -> McpToolResult<FindSymbolResponse>;
    fn search_code(&self, request: SearchCodeRequest) -> McpToolResult<SearchCodeResponse>;
    fn find_callers(&self, request: FindCallersRequest) -> McpToolResult<FindCallersResponse>;
    fn find_callees(&self, request: FindCalleesRequest) -> McpToolResult<FindCalleesResponse>;
    fn related_symbols(
        &self,
        request: RelatedSymbolsRequest,
    ) -> McpToolResult<RelatedSymbolsResponse>;
    fn impact_analysis(
        &self,
        request: ImpactAnalysisRequest,
    ) -> McpToolResult<ImpactAnalysisResponse>;
    fn get_context_pack(&self, request: ContextPackRequest) -> McpToolResult<ContextPackResponse>;
    fn trace_dependency(
        &self,
        request: TraceDependencyRequest,
    ) -> McpToolResult<TraceDependencyResponse>;
    fn detect_cycles(&self, request: DetectCyclesRequest) -> McpToolResult<DetectCyclesResponse>;
    fn savings_report(&self, request: SavingsReportRequest)
        -> McpToolResult<SavingsReportResponse>;
}

pub struct McpQueryToolRouter<E> {
    executor: E,
}

impl<E> McpQueryToolRouter<E>
where
    E: QueryToolExecutor,
{
    pub fn new(executor: E) -> Self {
        Self { executor }
    }

    pub fn find_symbol(&self, request: FindSymbolRequest) -> McpToolResult<FindSymbolResponse> {
        validate_text("query", &request.query)?;
        validate_limit(request.limit.unwrap_or(20))?;
        validate_scope(&request.scope)?;
        self.executor.find_symbol(request)
    }

    pub fn search_code(&self, request: SearchCodeRequest) -> McpToolResult<SearchCodeResponse> {
        validate_text("query", &request.query)?;
        validate_limit(request.limit.unwrap_or(20))?;
        validate_scope(&request.scope)?;
        self.executor.search_code(request)
    }

    pub fn find_callers(&self, request: FindCallersRequest) -> McpToolResult<FindCallersResponse> {
        validate_symbol_id(&request.symbol_id)?;
        validate_depth(request.max_depth.unwrap_or(2))?;
        validate_scope(&request.scope)?;
        self.executor.find_callers(request)
    }

    pub fn find_callees(&self, request: FindCalleesRequest) -> McpToolResult<FindCalleesResponse> {
        validate_symbol_id(&request.symbol_id)?;
        validate_depth(request.max_depth.unwrap_or(2))?;
        validate_scope(&request.scope)?;
        self.executor.find_callees(request)
    }

    pub fn related_symbols(
        &self,
        request: RelatedSymbolsRequest,
    ) -> McpToolResult<RelatedSymbolsResponse> {
        validate_symbol_id(&request.symbol_id)?;
        validate_depth(request.max_depth.unwrap_or(2))?;
        validate_scope(&request.scope)?;
        self.executor.related_symbols(request)
    }

    pub fn impact_analysis(
        &self,
        request: ImpactAnalysisRequest,
    ) -> McpToolResult<ImpactAnalysisResponse> {
        validate_symbol_id(&request.symbol_id)?;
        validate_scope(&request.scope)?;
        self.executor.impact_analysis(request)
    }

    pub fn get_context_pack(
        &self,
        request: ContextPackRequest,
    ) -> McpToolResult<ContextPackResponse> {
        validate_text("query", &request.query)?;
        validate_token_budget(request.token_budget)?;
        validate_scope(&request.scope)?;
        self.executor.get_context_pack(request)
    }

    pub fn trace_dependency(
        &self,
        request: TraceDependencyRequest,
    ) -> McpToolResult<TraceDependencyResponse> {
        validate_symbol_id(&request.source_symbol_id)?;
        validate_symbol_id(&request.target_symbol_id)?;
        validate_depth(request.max_depth.unwrap_or(4))?;
        validate_confidence(request.min_confidence.unwrap_or(0))?;
        validate_edge_filters(&request.edge_filters)?;
        validate_scope(&request.scope)?;
        self.executor.trace_dependency(request)
    }

    pub fn detect_cycles(
        &self,
        request: DetectCyclesRequest,
    ) -> McpToolResult<DetectCyclesResponse> {
        validate_max_nodes(request.max_nodes.unwrap_or(512))?;
        validate_confidence(request.min_confidence.unwrap_or(0))?;
        validate_edge_filters(&request.edge_filters)?;
        validate_scope(&request.scope)?;
        self.executor.detect_cycles(request)
    }

    pub fn savings_report(
        &self,
        request: SavingsReportRequest,
    ) -> McpToolResult<SavingsReportResponse> {
        validate_scope(&request.scope)?;
        self.executor.savings_report(request)
    }
}

impl<R> QueryToolExecutor for LocalQueryEngine<R>
where
    R: b3_core::QueryRepository + b3_core::TokenSavingsRepository + b3_core::CentralityRepository,
{
    fn find_symbol(&self, request: FindSymbolRequest) -> McpToolResult<FindSymbolResponse> {
        self.find_symbol_response(
            &scope(&request.scope),
            &request.query,
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn search_code(&self, request: SearchCodeRequest) -> McpToolResult<SearchCodeResponse> {
        self.search_code_response(
            &scope(&request.scope),
            &request.query,
            request.limit.unwrap_or(20),
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn find_callers(&self, request: FindCallersRequest) -> McpToolResult<FindCallersResponse> {
        self.find_callers_response(
            &scope(&request.scope),
            &SymbolId::new(request.symbol_id),
            request.max_depth.unwrap_or(2),
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn find_callees(&self, request: FindCalleesRequest) -> McpToolResult<FindCalleesResponse> {
        self.find_callees_response(
            &scope(&request.scope),
            &SymbolId::new(request.symbol_id),
            request.max_depth.unwrap_or(2),
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn related_symbols(
        &self,
        request: RelatedSymbolsRequest,
    ) -> McpToolResult<RelatedSymbolsResponse> {
        self.related_symbols_response(
            &scope(&request.scope),
            &SymbolId::new(request.symbol_id),
            request.max_depth.unwrap_or(2),
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn impact_analysis(
        &self,
        request: ImpactAnalysisRequest,
    ) -> McpToolResult<ImpactAnalysisResponse> {
        self.impact_analysis_response(
            &scope(&request.scope),
            &SymbolId::new(request.symbol_id),
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn get_context_pack(&self, request: ContextPackRequest) -> McpToolResult<ContextPackResponse> {
        self.context_pack_response_for_query(
            &scope(&request.scope),
            &request.query,
            request.token_budget,
            request.include_trace,
        )
        .map_err(tool_error)
    }

    fn trace_dependency(
        &self,
        request: TraceDependencyRequest,
    ) -> McpToolResult<TraceDependencyResponse> {
        let path = self
            .dependency_path(
                &scope(&request.scope),
                &SymbolId::new(request.source_symbol_id),
                &SymbolId::new(request.target_symbol_id),
                &edge_filters(&request.edge_filters)?,
                request.max_depth.unwrap_or(4),
                request.min_confidence.unwrap_or(0),
            )
            .map_err(tool_error)?;
        Ok(path_response(path, request.include_trace))
    }

    fn detect_cycles(&self, request: DetectCyclesRequest) -> McpToolResult<DetectCyclesResponse> {
        let cycles = self
            .detect_cycles(
                &scope(&request.scope),
                &edge_filters(&request.edge_filters)?,
                request.max_nodes.unwrap_or(512),
                request.min_confidence.unwrap_or(0),
            )
            .map_err(tool_error)?;
        Ok(cycles_response(cycles, request.include_trace))
    }

    fn savings_report(
        &self,
        request: SavingsReportRequest,
    ) -> McpToolResult<SavingsReportResponse> {
        Ok(SavingsReportResponse {
            estimated_tokens_saved: 0,
            returned_tokens: 0,
            avoided_file_reads: 0,
            avoided_search_calls: 0,
            trace_included: request.include_trace,
        })
    }
}

pub fn registered_tools() -> Vec<ToolDefinition> {
    vec![
        tool_doc(
            "find_symbol",
            "Find exact symbols by name.",
            "FindSymbolRequest",
            "FindSymbolResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"query":"run","limit":10,"include_trace":true}"#,
        ),
        tool_doc(
            "search_code",
            "Search indexed code with SQLite FTS/BM25.",
            "SearchCodeRequest",
            "SearchCodeResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"query":"helper call","limit":20,"include_trace":false}"#,
        ),
        tool_doc(
            "find_callers",
            "Find bounded inbound CALLS graph neighbors.",
            "FindCallersRequest",
            "FindCallersResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","max_depth":2,"include_trace":true}"#,
        ),
        tool_doc(
            "find_callees",
            "Find bounded outbound CALLS graph neighbors.",
            "FindCalleesRequest",
            "FindCalleesResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","max_depth":2,"include_trace":true}"#,
        ),
        tool_doc(
            "related_symbols",
            "Find bounded related symbols over safe graph edges.",
            "RelatedSymbolsRequest",
            "RelatedSymbolsResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","max_depth":2,"include_trace":false}"#,
        ),
        tool_doc(
            "impact_analysis",
            "Return basic graph impact analysis from indexed data.",
            "ImpactAnalysisRequest",
            "ImpactAnalysisResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","include_trace":true}"#,
        ),
        tool_doc(
            "get_context_pack",
            "Build a token-budgeted context pack without full-file dumps.",
            "ContextPackRequest",
            "ContextPackResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"query":"run","token_budget":2000,"include_trace":true}"#,
        ),
        tool_doc(
            "trace_dependency",
            "Find an unweighted shortest dependency path.",
            "TraceDependencyRequest",
            "TraceDependencyResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"source_symbol_id":"a","target_symbol_id":"b","edge_filters":["calls"],"max_depth":4,"min_confidence":8000,"include_trace":false}"#,
        ),
        tool_doc(
            "detect_cycles",
            "Detect bounded graph cycles with SCC analysis.",
            "DetectCyclesRequest",
            "DetectCyclesResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"edge_filters":["calls"],"max_nodes":512,"min_confidence":0,"include_trace":false}"#,
        ),
        tool_doc(
            "savings_report",
            "Return token-savings report placeholder until ledger aggregation is exposed.",
            "SavingsReportRequest",
            "SavingsReportResponse",
            r#"{"scope":{"project_id":"p","branch_id":"main"},"include_trace":false}"#,
        ),
    ]
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": PRODUCT_NAME,
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn tools_list_result() -> Value {
    let tools = registered_tools()
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.purpose,
                "inputSchema": {
                    "type": "object",
                    "title": tool.input_schema,
                    "description": tool.example
                }
            })
        })
        .collect::<Vec<_>>();
    json!({ "tools": tools })
}

fn dispatch_tool_call<E>(
    router: &McpQueryToolRouter<E>,
    params: Option<Value>,
) -> McpToolResult<Value>
where
    E: QueryToolExecutor,
{
    let params = params.ok_or_else(|| validation_error("tools/call params are required"))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| validation_error("tools/call name is required"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let response = match name {
        "find_symbol" => serde_json::to_value(router.find_symbol(decode(arguments)?)?),
        "search_code" => serde_json::to_value(router.search_code(decode(arguments)?)?),
        "find_callers" => serde_json::to_value(router.find_callers(decode(arguments)?)?),
        "find_callees" => serde_json::to_value(router.find_callees(decode(arguments)?)?),
        "related_symbols" => serde_json::to_value(router.related_symbols(decode(arguments)?)?),
        "impact_analysis" => serde_json::to_value(router.impact_analysis(decode(arguments)?)?),
        "get_context_pack" => serde_json::to_value(router.get_context_pack(decode(arguments)?)?),
        "trace_dependency" => serde_json::to_value(router.trace_dependency(decode(arguments)?)?),
        "detect_cycles" => serde_json::to_value(router.detect_cycles(decode(arguments)?)?),
        "savings_report" => serde_json::to_value(router.savings_report(decode(arguments)?)?),
        _ => return Err(validation_error("unknown tool name")),
    }
    .map_err(|error| McpToolError {
        code: "serialization_error".to_string(),
        message: error.to_string(),
    })?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": response.to_string()
            }
        ],
        "isError": false
    }))
}

fn decode<T>(value: Value) -> McpToolResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(value).map_err(|error| McpToolError {
        code: "invalid_request".to_string(),
        message: error.to_string(),
    })
}

fn json_rpc_result(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn json_rpc_tool_error(id: Option<Value>, code: i64, error: McpToolError) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": error.message,
            "data": {
                "code": error.code
            }
        }
    })
}

fn tool_doc(
    name: &str,
    purpose: &str,
    input_schema: &str,
    output_schema: &str,
    example: &str,
) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        purpose: purpose.to_string(),
        input_schema: input_schema.to_string(),
        output_schema: output_schema.to_string(),
        example: example.to_string(),
        token_saving_behavior: "Uses indexed symbols, FTS, graph traversal, or context packs to avoid raw file reads.".to_string(),
        trace_behavior: "Set include_trace=true to include retrieval trace where the underlying query response supports it.".to_string(),
    }
}

fn validate_scope(scope: &ScopeDto) -> McpToolResult<()> {
    validate_text("project_id", &scope.project_id)?;
    validate_text("branch_id", &scope.branch_id)
}

fn validate_text(field: &str, value: &str) -> McpToolResult<()> {
    if value.trim().is_empty() {
        Err(validation_error(&format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}

fn validate_symbol_id(value: &str) -> McpToolResult<()> {
    validate_text("symbol_id", value)
}

fn validate_limit(value: usize) -> McpToolResult<()> {
    if value == 0 {
        Err(validation_error("limit must be greater than zero"))
    } else if value > MAX_LIMIT {
        Err(validation_error("limit exceeds runtime maximum"))
    } else {
        Ok(())
    }
}

fn validate_depth(value: usize) -> McpToolResult<()> {
    if value == 0 {
        Err(validation_error("max_depth must be greater than zero"))
    } else if value > MAX_DEPTH {
        Err(validation_error("max_depth exceeds runtime maximum"))
    } else {
        Ok(())
    }
}

fn validate_token_budget(value: usize) -> McpToolResult<()> {
    if value == 0 {
        Err(validation_error("token_budget must be greater than zero"))
    } else if value > MAX_TOKEN_BUDGET {
        Err(validation_error("token_budget exceeds runtime maximum"))
    } else {
        Ok(())
    }
}

fn validate_max_nodes(value: usize) -> McpToolResult<()> {
    if value == 0 {
        Err(validation_error("max_nodes must be greater than zero"))
    } else if value > MAX_CYCLE_NODES {
        Err(validation_error("max_nodes exceeds runtime limit"))
    } else {
        Ok(())
    }
}

fn validate_confidence(value: u16) -> McpToolResult<()> {
    if value > 10_000 {
        Err(validation_error("min_confidence must be <= 10000"))
    } else {
        Ok(())
    }
}

fn validate_edge_filters(filters: &[String]) -> McpToolResult<()> {
    for filter in filters {
        parse_edge_kind(filter)?;
    }
    Ok(())
}

fn scope(scope: &ScopeDto) -> QueryScope {
    QueryScope::new(
        ProjectId::new(scope.project_id.clone()),
        BranchId::new(scope.branch_id.clone()),
    )
}

fn edge_filters(filters: &[String]) -> McpToolResult<Vec<EdgeKind>> {
    filters
        .iter()
        .map(|filter| parse_edge_kind(filter))
        .collect()
}

fn parse_edge_kind(value: &str) -> McpToolResult<EdgeKind> {
    match value {
        "contains" => Ok(EdgeKind::Contains),
        "imports" => Ok(EdgeKind::Imports),
        "calls" => Ok(EdgeKind::Calls),
        "references" => Ok(EdgeKind::References),
        "implements" => Ok(EdgeKind::Implements),
        "inherits" => Ok(EdgeKind::Inherits),
        "depends_on" => Ok(EdgeKind::DependsOn),
        "tests" => Ok(EdgeKind::Tests),
        "routes_to" => Ok(EdgeKind::RoutesTo),
        "reads_config" => Ok(EdgeKind::ReadsConfig),
        "writes_config" => Ok(EdgeKind::WritesConfig),
        "similar_to" => Ok(EdgeKind::SimilarTo),
        "touches" => Ok(EdgeKind::Touches),
        "decides" => Ok(EdgeKind::Decides),
        _ => Err(validation_error("unknown edge filter")),
    }
}

fn path_response(path: DependencyPath, trace_included: bool) -> TraceDependencyResponse {
    TraceDependencyResponse {
        found: path.found,
        node_ids: path
            .nodes
            .into_iter()
            .map(|node| node.id.as_str().to_string())
            .collect(),
        edge_ids: path
            .edges
            .into_iter()
            .map(|edge| edge.edge_id.as_str().to_string())
            .collect(),
        path_length: path.path_length,
        confidence_summary: path.confidence_summary,
        trace_included,
    }
}

fn cycles_response(
    cycles: b3_query::CycleDetectionResult,
    trace_included: bool,
) -> DetectCyclesResponse {
    DetectCyclesResponse {
        cycles: cycles
            .cycles
            .into_iter()
            .map(|cycle| {
                cycle
                    .node_ids
                    .into_iter()
                    .map(|id| id.as_str().to_string())
                    .collect()
            })
            .collect(),
        scanned_nodes: cycles.scanned_nodes,
        summary_count: cycles.summary_count,
        trace_included,
    }
}

fn tool_error(error: ContractError) -> McpToolError {
    McpToolError {
        code: "query_error".to_string(),
        message: error.message,
    }
}

fn validation_error(message: &str) -> McpToolError {
    McpToolError {
        code: "invalid_request".to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b3_core::{
        FindSymbolResponse, ImpactRiskLevel, QuerySavingsEstimateDto, QueryTraceDto,
        SearchCodeResponse, SymbolDto,
    };

    #[derive(Default)]
    struct MockExecutor;

    impl QueryToolExecutor for MockExecutor {
        fn find_symbol(&self, request: FindSymbolRequest) -> McpToolResult<FindSymbolResponse> {
            Ok(FindSymbolResponse {
                symbols: vec![symbol("run")],
                trace_id: "trace-find".to_string(),
                trace: request.include_trace.then_some(trace("find_symbol")),
            })
        }

        fn search_code(&self, request: SearchCodeRequest) -> McpToolResult<SearchCodeResponse> {
            Ok(SearchCodeResponse {
                symbols: vec![symbol("search")],
                trace_id: "trace-search".to_string(),
                trace: request.include_trace.then_some(trace("search_code")),
            })
        }

        fn find_callers(&self, _request: FindCallersRequest) -> McpToolResult<FindCallersResponse> {
            Ok(FindCallersResponse {
                callers: Vec::new(),
                trace_id: "trace-callers".to_string(),
                trace: None,
            })
        }

        fn find_callees(&self, _request: FindCalleesRequest) -> McpToolResult<FindCalleesResponse> {
            Ok(FindCalleesResponse {
                callees: Vec::new(),
                trace_id: "trace-callees".to_string(),
                trace: None,
            })
        }

        fn related_symbols(
            &self,
            _request: RelatedSymbolsRequest,
        ) -> McpToolResult<RelatedSymbolsResponse> {
            Ok(RelatedSymbolsResponse {
                related: Vec::new(),
                trace_id: "trace-related".to_string(),
                trace: None,
            })
        }

        fn impact_analysis(
            &self,
            _request: ImpactAnalysisRequest,
        ) -> McpToolResult<ImpactAnalysisResponse> {
            Ok(ImpactAnalysisResponse {
                impacted: Vec::new(),
                risk_score: 0,
                risk_level: ImpactRiskLevel::Low,
                risk_reasons: Vec::new(),
                risk_signals: Vec::new(),
                impacted_symbols: Vec::new(),
                impacted_files: Vec::new(),
                related_tests: Vec::new(),
                missing_tests: true,
                dependency_paths: Vec::new(),
                cycles_involved: Vec::new(),
                trace_id: "trace-impact".to_string(),
                trace: None,
            })
        }

        fn get_context_pack(
            &self,
            request: ContextPackRequest,
        ) -> McpToolResult<ContextPackResponse> {
            Ok(ContextPackResponse {
                items: Vec::new(),
                returned_tokens: 0,
                token_budget: request.token_budget,
                skipped_items: Vec::new(),
                truncation_reason: None,
                expansion_handles: Vec::new(),
                trace_id: "trace-pack".to_string(),
                trace: request.include_trace.then_some(trace("get_context_pack")),
            })
        }

        fn trace_dependency(
            &self,
            request: TraceDependencyRequest,
        ) -> McpToolResult<TraceDependencyResponse> {
            Ok(TraceDependencyResponse {
                found: true,
                node_ids: vec![request.source_symbol_id, request.target_symbol_id],
                edge_ids: vec!["edge".to_string()],
                path_length: 1,
                confidence_summary: Some(9_000),
                trace_included: request.include_trace,
            })
        }

        fn detect_cycles(
            &self,
            request: DetectCyclesRequest,
        ) -> McpToolResult<DetectCyclesResponse> {
            Ok(DetectCyclesResponse {
                cycles: Vec::new(),
                scanned_nodes: request.max_nodes.unwrap_or(10),
                summary_count: 0,
                trace_included: request.include_trace,
            })
        }

        fn savings_report(
            &self,
            request: SavingsReportRequest,
        ) -> McpToolResult<SavingsReportResponse> {
            Ok(SavingsReportResponse {
                estimated_tokens_saved: 10,
                returned_tokens: 1,
                avoided_file_reads: 1,
                avoided_search_calls: 1,
                trace_included: request.include_trace,
            })
        }
    }

    #[test]
    fn validates_tool_requests_before_mapping() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let error = router
            .find_symbol(FindSymbolRequest {
                scope: scope_dto(),
                query: " ".to_string(),
                limit: Some(10),
                include_trace: false,
            })
            .expect_err("empty query");

        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn serializes_response_dtos() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = router
            .find_symbol(FindSymbolRequest {
                scope: scope_dto(),
                query: "run".to_string(),
                limit: Some(10),
                include_trace: true,
            })
            .expect("response");
        let json = serde_json::to_string(&response).expect("serialize");

        assert!(json.contains("trace-find"));
        assert!(json.contains("run"));
    }

    #[test]
    fn include_trace_controls_trace_output() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let with_trace = router
            .search_code(SearchCodeRequest {
                scope: scope_dto(),
                query: "helper".to_string(),
                limit: Some(5),
                include_trace: true,
            })
            .expect("trace");
        let without_trace = router
            .search_code(SearchCodeRequest {
                scope: scope_dto(),
                query: "helper".to_string(),
                limit: Some(5),
                include_trace: false,
            })
            .expect("no trace");

        assert!(with_trace.trace.is_some());
        assert!(without_trace.trace.is_none());
    }

    #[test]
    fn context_pack_response_has_no_full_file_dump() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = router
            .get_context_pack(ContextPackRequest {
                scope: scope_dto(),
                query: "run".to_string(),
                token_budget: 100,
                include_trace: false,
            })
            .expect("pack");

        assert!(response.items.is_empty());
        assert_eq!(response.returned_tokens, 0);
    }

    #[test]
    fn impact_analysis_response_includes_risk_fields() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = router
            .impact_analysis(ImpactAnalysisRequest {
                scope: scope_dto(),
                symbol_id: "symbol-run".to_string(),
                include_trace: false,
            })
            .expect("impact");
        let json = serde_json::to_string(&response).expect("serialize");

        assert!(json.contains("risk_score"));
        assert!(json.contains("missing_tests"));
    }

    #[test]
    fn maps_dependency_and_cycle_tools_thinly() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let path = router
            .trace_dependency(TraceDependencyRequest {
                scope: scope_dto(),
                source_symbol_id: "a".to_string(),
                target_symbol_id: "b".to_string(),
                edge_filters: vec!["calls".to_string()],
                max_depth: Some(2),
                min_confidence: Some(1),
                include_trace: true,
            })
            .expect("path");
        let cycles = router
            .detect_cycles(DetectCyclesRequest {
                scope: scope_dto(),
                edge_filters: vec!["calls".to_string()],
                max_nodes: Some(10),
                min_confidence: Some(0),
                include_trace: false,
            })
            .expect("cycles");

        assert!(path.found);
        assert_eq!(cycles.summary_count, 0);
    }

    #[test]
    fn documents_all_tools() {
        let docs = registered_tools();
        assert_eq!(docs.len(), 10);
        assert!(docs.iter().any(|tool| tool.name == "get_context_pack"));
        assert!(docs
            .iter()
            .all(|tool| !tool.token_saving_behavior.is_empty()));
    }

    #[test]
    fn json_rpc_lists_and_calls_tools() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let list =
            handle_json_rpc_line(&router, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
                .expect("list");
        let call = handle_json_rpc_line(
            &router,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"find_symbol","arguments":{"scope":{"project_id":"project","branch_id":"main"},"query":"run","limit":10,"include_trace":true}}}"#,
        )
        .expect("call");

        assert!(list
            .response
            .expect("list response")
            .to_string()
            .contains("find_symbol"));
        assert!(call
            .response
            .expect("call response")
            .to_string()
            .contains("trace-find"));
    }

    #[test]
    fn json_rpc_maps_validation_errors() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = handle_json_rpc_line(
            &router,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"search_code","arguments":{"scope":{"project_id":"project","branch_id":"main"},"query":"","limit":10,"include_trace":false}}}"#,
        )
        .expect("error")
        .response
        .expect("response");

        assert!(response.to_string().contains("invalid_request"));
    }

    #[test]
    fn stdio_loop_writes_responses_and_shuts_down() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let input = std::io::Cursor::new(
            b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"shutdown\"}\n",
        );
        let mut output = Vec::new();

        serve_stdio(router, input, &mut output).expect("serve");
        let text = String::from_utf8(output).expect("utf8");

        assert!(text.contains("protocolVersion"));
        assert!(text.contains("\"id\":2"));
    }

    fn scope_dto() -> ScopeDto {
        ScopeDto {
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
        }
    }

    fn symbol(name: &str) -> SymbolDto {
        SymbolDto {
            symbol_id: format!("symbol-{name}"),
            file_id: "file".to_string(),
            name: name.to_string(),
            kind: "Function".to_string(),
            start_line: 1,
            end_line: 1,
            visibility: None,
            score: 100,
            why: "test".to_string(),
        }
    }

    fn trace(intent: &str) -> QueryTraceDto {
        QueryTraceDto {
            trace_id: format!("trace-{intent}"),
            query_input: "input".to_string(),
            query_intent: intent.to_string(),
            project_id: "project".to_string(),
            branch_id: "main".to_string(),
            exact_symbol_hits: Vec::new(),
            fts_hits: Vec::new(),
            graph_traversal_steps: Vec::new(),
            ranking_decisions: Vec::new(),
            context_items_selected: Vec::new(),
            context_items_skipped: Vec::new(),
            truncation_reason: None,
            token_budget_used: 0,
            token_budget: 0,
            token_savings_estimate: Some(QuerySavingsEstimateDto {
                returned_tokens: 0,
                avoided_file_reads: 0,
                avoided_search_calls: 0,
                estimated_tokens_saved: 0,
            }),
            warnings: Vec::new(),
        }
    }
}
