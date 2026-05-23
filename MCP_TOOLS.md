# MCP Query Tools

Phase 6 exposes query-engine capabilities through thin MCP tool adapters. The
runtime validates requests, maps DTOs, calls the query layer, and returns stable
serializable responses. It does not perform indexing, storage operations,
ranking, traversal, embeddings, or UI work.

## Local Stdio Server

Run the local stdio server with:

```powershell
b3-mcp-runtime serve --project .
```

By default, the runtime opens `.b3/b3.db` under the project directory. A local
database path can be supplied explicitly:

```powershell
b3-mcp-runtime serve --project . --database .b3/b3.db
```

The bootstrap is local-only: it opens SQLite storage, initializes the query
engine, and starts the JSON-RPC stdio loop. It does not call external APIs,
start telemetry, index files, or run embeddings.

## Client Config Examples

Cursor:

```json
{
  "mcpServers": {
    "b3-code-intelligence": {
      "command": "path/to/b3-mcp-runtime",
      "args": ["serve", "--project", "."]
    }
  }
}
```

Codex-style generic MCP config:

```json
{
  "mcpServers": {
    "b3-code-intelligence": {
      "command": "path/to/b3-mcp-runtime",
      "args": ["serve", "--project", "."]
    }
  }
}
```

All tools require `scope.project_id` and `scope.branch_id`.

## Tools

### `find_symbol`
- Purpose: exact symbol lookup by name.
- Input: `FindSymbolRequest`
- Output: `FindSymbolResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"query":"run","limit":10,"include_trace":true}`
- Token saving: uses indexed symbols instead of file scans.
- Trace: optional retrieval trace.

### `search_code`
- Purpose: FTS/BM25 lexical search over indexed symbols and file content.
- Input: `SearchCodeRequest`
- Output: `SearchCodeResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"query":"helper call","limit":20,"include_trace":false}`
- Token saving: returns ranked symbol snippets, not full files.
- Trace: optional FTS/ranking trace.

### `find_callers`
- Purpose: inbound `CALLS` graph lookup.
- Input: `FindCallersRequest`
- Output: `FindCallersResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","max_depth":2,"include_trace":true}`
- Token saving: uses indexed call edges.
- Trace: optional traversal trace.

### `find_callees`
- Purpose: outbound `CALLS` graph lookup.
- Input: `FindCalleesRequest`
- Output: `FindCalleesResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","max_depth":2,"include_trace":true}`
- Token saving: uses indexed call edges.
- Trace: optional traversal trace.

### `related_symbols`
- Purpose: bounded related-symbol graph traversal with cached centrality used as
  a secondary ranking signal when a local snapshot exists.
- Input: `RelatedSymbolsRequest`
- Output: `RelatedSymbolsResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","max_depth":2,"include_trace":false}`
- Token saving: returns compact related symbol DTOs.
- Trace: optional traversal trace.

### `impact_analysis`
- Purpose: explainable impact analysis from graph callers, related symbols,
  risk scoring, related-test discovery, and cached centrality when available.
- Input: `ImpactAnalysisRequest`
- Output: `ImpactAnalysisResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"symbol_id":"symbol-a","include_trace":true}`
- Token saving: avoids broad repository scans.
- Trace: optional impact trace including traversal, risk signals, test matching,
  and centrality contribution when a PageRank snapshot exists.

### `get_context_pack`
- Purpose: token-budgeted context pack.
- Input: `ContextPackRequest`
- Output: `ContextPackResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"query":"run","token_budget":2000,"include_trace":true}`
- Token saving: returns compact snippets with expansion handles.
- Trace: optional context selection and skip trace. Ranking can include cached
  centrality boosts, but exact/lexical matches remain primary.

## Centrality Snapshots

Phase 6.2 adds local graph centrality snapshots owned by the query/storage
layers. PageRank is computed explicitly over bounded, branch-scoped graph data
and persisted in SQLite for later query use. The MCP runtime does not compute
centrality; it only returns query-engine responses.

Default PageRank edge filters are `calls`, `references`, `imports`,
`depends_on`, `implements`, and `inherits`. Missing edge types are handled
without error.

### `trace_dependency`
- Purpose: BFS shortest dependency path.
- Input: `TraceDependencyRequest`
- Output: `TraceDependencyResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"source_symbol_id":"a","target_symbol_id":"b","edge_filters":["calls"],"max_depth":4,"min_confidence":8000,"include_trace":false}`
- Token saving: uses graph paths instead of manual exploration.
- Trace: response marks trace inclusion; detailed query trace remains future work.

### `detect_cycles`
- Purpose: bounded SCC cycle detection.
- Input: `DetectCyclesRequest`
- Output: `DetectCyclesResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"edge_filters":["calls"],"max_nodes":512,"min_confidence":0,"include_trace":false}`
- Token saving: uses indexed graph edges.
- Trace: response marks trace inclusion; detailed query trace remains future work.

### `savings_report`
- Purpose: savings report response placeholder until ledger aggregation is exposed.
- Input: `SavingsReportRequest`
- Output: `SavingsReportResponse`
- Example: `{"scope":{"project_id":"p","branch_id":"main"},"include_trace":false}`
- Token saving: reports savings counters when aggregation is available.
- Trace: response marks trace inclusion.

## Validation

The runtime rejects empty queries/symbol IDs, missing scope, zero or excessive
limits, invalid depth, invalid token budgets, invalid confidence values, and
unknown edge filters with structured `McpToolError` responses.
