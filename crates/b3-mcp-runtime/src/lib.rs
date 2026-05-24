//! Thin MCP runtime boundary.
//!
//! This crate owns protocol-facing concerns only: tool names, request DTOs,
//! validation, error mapping, and calls into the query engine. Heavy indexing,
//! graph traversal, ranking, storage, embeddings, and UI logic stay behind this
//! boundary.

use b3_compaction::{compact_command_output, CommandOutputInput, CommandOutputSummary};
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
    fmt,
    io::{BufRead, Write},
    path::PathBuf,
    str::FromStr,
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
    pub tool_profile: ToolProfileName,
}

impl RuntimeBootstrapConfig {
    pub fn local_project(project_path: impl Into<PathBuf>) -> Self {
        let project_path = project_path.into();
        let database_path = project_path.join(".b3").join("b3.db");
        Self {
            project_path,
            database_path,
            tool_profile: ToolProfileName::default(),
        }
    }
}

pub fn serve_local_stdio(config: RuntimeBootstrapConfig) -> Result<(), String> {
    let storage = SqliteStorage::open(&config.database_path).map_err(|error| error.message)?;
    let engine = LocalQueryEngine::new(storage, QueryEngineConfig::default());
    let router = McpQueryToolRouter::with_profile(engine, config.tool_profile);
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
        "tools/list" => Some(json_rpc_result(id, tools_list_result(router.profile()))),
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
    CompactCommandOutput,
}

impl QueryToolName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FindSymbol => "find_symbol",
            Self::SearchCode => "search_code",
            Self::FindCallers => "find_callers",
            Self::FindCallees => "find_callees",
            Self::RelatedSymbols => "related_symbols",
            Self::ImpactAnalysis => "impact_analysis",
            Self::GetContextPack => "get_context_pack",
            Self::TraceDependency => "trace_dependency",
            Self::DetectCycles => "detect_cycles",
            Self::SavingsReport => "savings_report",
            Self::CompactCommandOutput => "compact_command_output",
        }
    }

    pub fn from_tool_name(value: &str) -> Option<Self> {
        match value {
            "find_symbol" => Some(Self::FindSymbol),
            "search_code" => Some(Self::SearchCode),
            "find_callers" => Some(Self::FindCallers),
            "find_callees" => Some(Self::FindCallees),
            "related_symbols" => Some(Self::RelatedSymbols),
            "impact_analysis" => Some(Self::ImpactAnalysis),
            "get_context_pack" => Some(Self::GetContextPack),
            "trace_dependency" => Some(Self::TraceDependency),
            "detect_cycles" => Some(Self::DetectCycles),
            "savings_report" => Some(Self::SavingsReport),
            "compact_command_output" => Some(Self::CompactCommandOutput),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolProfileName {
    Tiny,
    Optimized,
    Full,
    Debug,
    Readonly,
    Editing,
    WebApp,
    Enterprise,
}

impl Default for ToolProfileName {
    fn default() -> Self {
        Self::Optimized
    }
}

impl fmt::Display for ToolProfileName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Tiny => "tiny",
            Self::Optimized => "optimized",
            Self::Full => "full",
            Self::Debug => "debug",
            Self::Readonly => "readonly",
            Self::Editing => "editing",
            Self::WebApp => "web-app",
            Self::Enterprise => "enterprise",
        })
    }
}

impl FromStr for ToolProfileName {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "tiny" => Ok(Self::Tiny),
            "optimized" => Ok(Self::Optimized),
            "full" => Ok(Self::Full),
            "debug" => Ok(Self::Debug),
            "readonly" => Ok(Self::Readonly),
            "editing" => Ok(Self::Editing),
            "web-app" => Ok(Self::WebApp),
            "enterprise" => Ok(Self::Enterprise),
            _ => Err(format!(
                "invalid tool profile: {value}; supported profiles: tiny, optimized, full, debug, readonly, editing, web-app, enterprise"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExposurePolicy {
    ProfileFiltered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolManifestMode {
    Slim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolProfileConfig {
    pub name: ToolProfileName,
    pub exposure_policy: ToolExposurePolicy,
    pub manifest_mode: ToolManifestMode,
}

impl Default for ToolProfileConfig {
    fn default() -> Self {
        Self::new(ToolProfileName::default())
    }
}

impl ToolProfileConfig {
    pub fn new(name: ToolProfileName) -> Self {
        Self {
            name,
            exposure_policy: ToolExposurePolicy::ProfileFiltered,
            manifest_mode: ToolManifestMode::Slim,
        }
    }

    pub fn enabled_tools(&self) -> &'static [QueryToolName] {
        match self.name {
            ToolProfileName::Tiny => &[
                QueryToolName::SearchCode,
                QueryToolName::FindSymbol,
                QueryToolName::GetContextPack,
                QueryToolName::CompactCommandOutput,
                QueryToolName::SavingsReport,
            ],
            ToolProfileName::Optimized | ToolProfileName::Editing | ToolProfileName::WebApp => &[
                QueryToolName::FindSymbol,
                QueryToolName::SearchCode,
                QueryToolName::RelatedSymbols,
                QueryToolName::ImpactAnalysis,
                QueryToolName::GetContextPack,
                QueryToolName::CompactCommandOutput,
                QueryToolName::SavingsReport,
            ],
            ToolProfileName::Full | ToolProfileName::Debug | ToolProfileName::Readonly => &[
                QueryToolName::FindSymbol,
                QueryToolName::SearchCode,
                QueryToolName::FindCallers,
                QueryToolName::FindCallees,
                QueryToolName::RelatedSymbols,
                QueryToolName::ImpactAnalysis,
                QueryToolName::GetContextPack,
                QueryToolName::TraceDependency,
                QueryToolName::DetectCycles,
                QueryToolName::SavingsReport,
                QueryToolName::CompactCommandOutput,
            ],
            ToolProfileName::Enterprise => &[
                QueryToolName::FindSymbol,
                QueryToolName::SearchCode,
                QueryToolName::RelatedSymbols,
                QueryToolName::ImpactAnalysis,
                QueryToolName::GetContextPack,
                QueryToolName::TraceDependency,
                QueryToolName::DetectCycles,
                QueryToolName::CompactCommandOutput,
                QueryToolName::SavingsReport,
            ],
        }
    }

    pub fn is_enabled(&self, tool: QueryToolName) -> bool {
        self.enabled_tools().contains(&tool)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactCommandOutputRequest {
    pub command: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub working_directory: Option<String>,
    pub max_bytes: Option<usize>,
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
    fn compact_command_output(
        &self,
        request: CompactCommandOutputRequest,
    ) -> McpToolResult<CommandOutputSummary>;
}

pub struct McpQueryToolRouter<E> {
    executor: E,
    profile: ToolProfileConfig,
}

impl<E> McpQueryToolRouter<E>
where
    E: QueryToolExecutor,
{
    pub fn new(executor: E) -> Self {
        Self::with_profile(executor, ToolProfileName::default())
    }

    pub fn with_profile(executor: E, profile: ToolProfileName) -> Self {
        Self {
            executor,
            profile: ToolProfileConfig::new(profile),
        }
    }

    pub fn profile(&self) -> &ToolProfileConfig {
        &self.profile
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

    pub fn compact_command_output(
        &self,
        request: CompactCommandOutputRequest,
    ) -> McpToolResult<CommandOutputSummary> {
        validate_text("command", &request.command)?;
        if request.stdout.is_empty() && request.stderr.is_empty() {
            return Err(validation_error("stdout or stderr must be provided"));
        }
        if let Some(max_bytes) = request.max_bytes {
            if !(256..=128_000).contains(&max_bytes) {
                return Err(validation_error("max_bytes must be between 256 and 128000"));
            }
        }
        self.executor.compact_command_output(request)
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

    fn compact_command_output(
        &self,
        request: CompactCommandOutputRequest,
    ) -> McpToolResult<CommandOutputSummary> {
        Ok(compact_command_output(CommandOutputInput {
            command: request.command,
            argv: request.argv,
            stdout: request.stdout,
            stderr: request.stderr,
            exit_code: request.exit_code,
            working_directory: request.working_directory,
            max_bytes: request.max_bytes,
        }))
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
        tool_doc(
            "compact_command_output",
            "Compact provided shell output without executing commands.",
            "CompactCommandOutputRequest",
            "CommandOutputSummary",
            r#"{"command":"cargo test","stdout":"test result: ok","stderr":"","exit_code":0,"max_bytes":4000}"#,
        ),
    ]
}

pub fn registered_tools_for_profile(profile: &ToolProfileConfig) -> Vec<ToolDefinition> {
    registered_tools()
        .into_iter()
        .filter(|tool| {
            QueryToolName::from_tool_name(&tool.name)
                .map(|name| profile.is_enabled(name))
                .unwrap_or(false)
        })
        .collect()
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

fn tools_list_result(profile: &ToolProfileConfig) -> Value {
    let tools = registered_tools_for_profile(profile)
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.purpose,
                "inputSchema": input_schema_for_tool(&tool.name, &tool.input_schema)
            })
        })
        .collect::<Vec<_>>();
    json!({
        "tools": tools,
        "profile": profile.name.to_string()
    })
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
    let tool_name =
        QueryToolName::from_tool_name(name).ok_or_else(|| validation_error("unknown tool name"))?;
    if !router.profile().is_enabled(tool_name) {
        return Err(tool_disabled_error(name, router.profile().name));
    }
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
        "compact_command_output" => {
            serde_json::to_value(router.compact_command_output(decode(arguments)?)?)
        }
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

fn input_schema_for_tool(name: &str, title: &str) -> Value {
    let scope = json!({
        "type": "object",
        "required": ["project_id", "branch_id"],
        "properties": {
            "project_id": { "type": "string" },
            "branch_id": { "type": "string" }
        }
    });
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    if name != "compact_command_output" {
        properties.insert("scope".to_string(), scope);
        required.push("scope");
    }

    match name {
        "find_symbol" | "search_code" => {
            properties.insert("query".to_string(), json!({ "type": "string" }));
            properties.insert(
                "limit".to_string(),
                json!({ "type": "integer", "minimum": 1 }),
            );
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.extend(["query", "include_trace"]);
        }
        "find_callers" | "find_callees" | "related_symbols" => {
            properties.insert("symbol_id".to_string(), json!({ "type": "string" }));
            properties.insert(
                "max_depth".to_string(),
                json!({ "type": "integer", "minimum": 1 }),
            );
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.extend(["symbol_id", "include_trace"]);
        }
        "impact_analysis" => {
            properties.insert("symbol_id".to_string(), json!({ "type": "string" }));
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.extend(["symbol_id", "include_trace"]);
        }
        "get_context_pack" => {
            properties.insert("query".to_string(), json!({ "type": "string" }));
            properties.insert(
                "token_budget".to_string(),
                json!({ "type": "integer", "minimum": 1 }),
            );
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.extend(["query", "token_budget", "include_trace"]);
        }
        "trace_dependency" => {
            properties.insert("source_symbol_id".to_string(), json!({ "type": "string" }));
            properties.insert("target_symbol_id".to_string(), json!({ "type": "string" }));
            properties.insert(
                "edge_filters".to_string(),
                json!({ "type": "array", "items": { "type": "string" } }),
            );
            properties.insert(
                "max_depth".to_string(),
                json!({ "type": "integer", "minimum": 1 }),
            );
            properties.insert(
                "min_confidence".to_string(),
                json!({ "type": "integer", "minimum": 0, "maximum": 10000 }),
            );
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.extend([
                "source_symbol_id",
                "target_symbol_id",
                "edge_filters",
                "include_trace",
            ]);
        }
        "detect_cycles" => {
            properties.insert(
                "edge_filters".to_string(),
                json!({ "type": "array", "items": { "type": "string" } }),
            );
            properties.insert(
                "max_nodes".to_string(),
                json!({ "type": "integer", "minimum": 1 }),
            );
            properties.insert(
                "min_confidence".to_string(),
                json!({ "type": "integer", "minimum": 0, "maximum": 10000 }),
            );
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.extend(["edge_filters", "include_trace"]);
        }
        "savings_report" => {
            properties.insert("include_trace".to_string(), json!({ "type": "boolean" }));
            required.push("include_trace");
        }
        "compact_command_output" => {
            properties.insert("command".to_string(), json!({ "type": "string" }));
            properties.insert(
                "argv".to_string(),
                json!({ "type": "array", "items": { "type": "string" } }),
            );
            properties.insert("stdout".to_string(), json!({ "type": "string" }));
            properties.insert("stderr".to_string(), json!({ "type": "string" }));
            properties.insert(
                "exit_code".to_string(),
                json!({ "type": ["integer", "null"] }),
            );
            properties.insert(
                "working_directory".to_string(),
                json!({ "type": ["string", "null"] }),
            );
            properties.insert(
                "max_bytes".to_string(),
                json!({ "type": ["integer", "null"], "minimum": 256, "maximum": 128000 }),
            );
            required.extend(["command", "exit_code"]);
        }
        _ => {}
    }

    json!({
        "type": "object",
        "title": title,
        "required": required,
        "properties": properties
    })
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

fn tool_disabled_error(tool_name: &str, profile: ToolProfileName) -> McpToolError {
    McpToolError {
        code: "tool_not_enabled".to_string(),
        message: format!(
            "tool '{tool_name}' is not enabled in current profile '{profile}'; use full or debug profile if this tool is required"
        ),
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

        fn compact_command_output(
            &self,
            request: CompactCommandOutputRequest,
        ) -> McpToolResult<CommandOutputSummary> {
            Ok(compact_command_output(CommandOutputInput {
                command: request.command,
                argv: request.argv,
                stdout: request.stdout,
                stderr: request.stderr,
                exit_code: request.exit_code,
                working_directory: request.working_directory,
                max_bytes: request.max_bytes,
            }))
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
        assert_eq!(docs.len(), 11);
        assert!(docs.iter().any(|tool| tool.name == "get_context_pack"));
        assert!(docs
            .iter()
            .any(|tool| tool.name == "compact_command_output"));
        assert!(docs
            .iter()
            .all(|tool| !tool.token_saving_behavior.is_empty()));
    }

    #[test]
    fn parses_tool_profiles_and_rejects_invalid_values() {
        assert_eq!("tiny".parse::<ToolProfileName>(), Ok(ToolProfileName::Tiny));
        assert_eq!(
            "web-app".parse::<ToolProfileName>(),
            Ok(ToolProfileName::WebApp)
        );
        assert!("business-app".parse::<ToolProfileName>().is_err());
    }

    #[test]
    fn default_profile_is_optimized() {
        assert_eq!(ToolProfileName::default(), ToolProfileName::Optimized);
        assert_eq!(ToolProfileConfig::default().enabled_tools().len(), 7);
    }

    #[test]
    fn profiles_expose_expected_tool_counts() {
        let expected = [
            (ToolProfileName::Tiny, 5),
            (ToolProfileName::Optimized, 7),
            (ToolProfileName::Full, 11),
            (ToolProfileName::Debug, 11),
            (ToolProfileName::Readonly, 11),
            (ToolProfileName::Editing, 7),
            (ToolProfileName::WebApp, 7),
            (ToolProfileName::Enterprise, 9),
        ];

        for (profile, count) in expected {
            let config = ToolProfileConfig::new(profile);
            assert_eq!(config.enabled_tools().len(), count, "{profile}");
            assert!(config.is_enabled(QueryToolName::CompactCommandOutput));
        }
    }

    #[test]
    fn readonly_profile_currently_exposes_only_readonly_tools() {
        let config = ToolProfileConfig::new(ToolProfileName::Readonly);
        assert_eq!(config.enabled_tools().len(), 11);
        assert!(config
            .enabled_tools()
            .iter()
            .all(|tool| QueryToolName::from_tool_name(tool.as_str()).is_some()));
    }

    #[test]
    fn registered_tools_are_filtered_by_profile() {
        let tiny = registered_tools_for_profile(&ToolProfileConfig::new(ToolProfileName::Tiny));
        let enterprise =
            registered_tools_for_profile(&ToolProfileConfig::new(ToolProfileName::Enterprise));

        assert_eq!(tiny.len(), 5);
        assert!(tiny.iter().any(|tool| tool.name == "search_code"));
        assert!(!tiny.iter().any(|tool| tool.name == "impact_analysis"));
        assert_eq!(enterprise.len(), 9);
        assert!(enterprise
            .iter()
            .any(|tool| tool.name == "trace_dependency"));
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

        let list_response = list.response.expect("list response");
        assert_eq!(
            list_response["result"]["tools"]
                .as_array()
                .expect("tools")
                .len(),
            7
        );
        assert_eq!(list_response["result"]["profile"], "optimized");
        assert!(call
            .response
            .expect("call response")
            .to_string()
            .contains("trace-find"));
    }

    #[test]
    fn tools_list_respects_full_and_tiny_profiles() {
        let full = McpQueryToolRouter::with_profile(MockExecutor, ToolProfileName::Full);
        let tiny = McpQueryToolRouter::with_profile(MockExecutor, ToolProfileName::Tiny);

        let full_response =
            handle_json_rpc_line(&full, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
                .expect("full")
                .response
                .expect("response");
        let tiny_response =
            handle_json_rpc_line(&tiny, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)
                .expect("tiny")
                .response
                .expect("response");

        assert_eq!(
            full_response["result"]["tools"].as_array().unwrap().len(),
            11
        );
        assert_eq!(
            tiny_response["result"]["tools"].as_array().unwrap().len(),
            5
        );
    }

    #[test]
    fn hidden_tool_call_returns_profile_aware_error() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = handle_json_rpc_line(
            &router,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"find_callers","arguments":{"scope":{"project_id":"project","branch_id":"main"},"symbol_id":"symbol-run","max_depth":1,"include_trace":false}}}"#,
        )
        .expect("call")
        .response
        .expect("response");

        assert_eq!(response["error"]["data"]["code"], "tool_not_enabled");
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("optimized"));
    }

    #[test]
    fn full_profile_keeps_trace_tools_callable() {
        let router = McpQueryToolRouter::with_profile(MockExecutor, ToolProfileName::Full);
        let response = handle_json_rpc_line(
            &router,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"trace_dependency","arguments":{"scope":{"project_id":"project","branch_id":"main"},"source_symbol_id":"a","target_symbol_id":"b","edge_filters":["calls"],"max_depth":2,"min_confidence":0,"include_trace":false}}}"#,
        )
        .expect("call")
        .response
        .expect("response");

        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"found\":true"));
    }

    #[test]
    fn full_profile_keeps_all_current_tools_callable() {
        let router = McpQueryToolRouter::with_profile(MockExecutor, ToolProfileName::Full);
        let calls = [
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"find_symbol","arguments":{"scope":{"project_id":"project","branch_id":"main"},"query":"run","limit":10,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_code","arguments":{"scope":{"project_id":"project","branch_id":"main"},"query":"run","limit":10,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"find_callers","arguments":{"scope":{"project_id":"project","branch_id":"main"},"symbol_id":"symbol-run","max_depth":1,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"find_callees","arguments":{"scope":{"project_id":"project","branch_id":"main"},"symbol_id":"symbol-run","max_depth":1,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"related_symbols","arguments":{"scope":{"project_id":"project","branch_id":"main"},"symbol_id":"symbol-run","max_depth":1,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"impact_analysis","arguments":{"scope":{"project_id":"project","branch_id":"main"},"symbol_id":"symbol-run","include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_context_pack","arguments":{"scope":{"project_id":"project","branch_id":"main"},"query":"run","token_budget":100,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"trace_dependency","arguments":{"scope":{"project_id":"project","branch_id":"main"},"source_symbol_id":"a","target_symbol_id":"b","edge_filters":["calls"],"max_depth":2,"min_confidence":0,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"detect_cycles","arguments":{"scope":{"project_id":"project","branch_id":"main"},"edge_filters":["calls"],"max_nodes":10,"min_confidence":0,"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"savings_report","arguments":{"scope":{"project_id":"project","branch_id":"main"},"include_trace":false}}}"#,
            r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"compact_command_output","arguments":{"command":"cargo test","stdout":"test result: ok","stderr":"done","exit_code":0,"max_bytes":1000}}}"#,
        ];

        for call in calls {
            let response = handle_json_rpc_line(&router, call)
                .expect("call")
                .response
                .expect("response");
            assert!(response.get("error").is_none(), "{response}");
            assert_eq!(response["result"]["isError"], false);
        }
    }

    #[test]
    fn compact_command_output_is_available_in_optimized_profile() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = handle_json_rpc_line(
            &router,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"compact_command_output","arguments":{"command":"cargo test","stdout":"error[E0425]: missing","stderr":"thread panicked","exit_code":101,"max_bytes":1000}}}"#,
        )
        .expect("call")
        .response
        .expect("response");

        assert!(response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("compacted_output"));
    }

    #[test]
    fn slim_manifest_keeps_required_nested_scope_schema() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response =
            handle_json_rpc_line(&router, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
                .expect("list")
                .response
                .expect("response");
        let tools = response["result"]["tools"].as_array().expect("tools");
        let find_symbol = tools
            .iter()
            .find(|tool| tool["name"] == "find_symbol")
            .expect("find_symbol");
        let schema = &find_symbol["inputSchema"];

        assert_eq!(schema["type"], "object");
        assert!(schema["required"].to_string().contains("scope"));
        assert_eq!(
            schema["properties"]["scope"]["properties"]["project_id"]["type"],
            "string"
        );
        assert_eq!(
            schema["properties"]["scope"]["properties"]["branch_id"]["type"],
            "string"
        );
    }

    #[test]
    fn compact_command_output_tool_is_local_transform_only() {
        let router = McpQueryToolRouter::new(MockExecutor);
        let response = router
            .compact_command_output(CompactCommandOutputRequest {
                command: "cargo test".to_string(),
                argv: Vec::new(),
                stdout: "error[E0425]: missing\n".to_string(),
                stderr: "thread panicked\n".to_string(),
                exit_code: Some(101),
                working_directory: None,
                max_bytes: Some(1_000),
            })
            .expect("compact");

        assert_eq!(response.command_family, b3_compaction::CommandFamily::Cargo);
        assert!(response.compacted_output.contains("status: failed"));
        assert!(response.estimated_token_savings <= response.original_byte_estimate / 4);
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
