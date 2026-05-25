# MCP Query Tools

Phase 6 exposes query-engine capabilities through thin MCP tool adapters.
Phase 8.5 adds a local command-output compaction adapter. The runtime validates
requests, maps DTOs, calls the owning layer, and returns stable serializable
responses. It does not perform indexing, storage operations, ranking,
traversal, embeddings, command execution, or UI work.

## Local Stdio Server

Run the local stdio server with:

```powershell
b3-mcp-runtime serve --project . --profile optimized
```

`optimized` is the default, so omitting `--profile` is equivalent. A local
database path can be supplied explicitly:

```powershell
b3-mcp-runtime serve --project . --database .b3/b3.db --profile full
```

The bootstrap is local-only: it opens SQLite storage, initializes the query
engine, and starts the JSON-RPC stdio loop. It does not call external APIs,
start telemetry, index files, or run embeddings.

The runtime also accepts `--tool-profile` as an alias for `--profile`.

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

The `b3` helper can generate or apply these snippets locally:

```powershell
cargo run -p b3-cli -- install --agent codex --project "." --database ".b3/b3.db" --profile optimized --dry-run
cargo run -p b3-cli -- install --agent cursor --project "." --database ".b3/b3.db" --profile optimized --dry-run
```

All tools require `scope.project_id` and `scope.branch_id`.

The exception is `compact_command_output`, which operates only on command text
and stdout/stderr supplied by the caller. It does not execute commands.

## Tool Profiles

Profiles slim `tools/list` to the tools most useful for the current workflow.
Hidden tools are not callable. A hidden tool call returns a structured MCP error
with code `tool_not_enabled`, the tool name, the current profile, and a
suggestion to use `full` or `debug` when appropriate.

| Profile | Tools | Count |
|---|---|---:|
| `tiny` | `search_code`, `find_symbol`, `get_context_pack`, `compact_command_output`, `savings_report` | 5 |
| `optimized` | `find_symbol`, `search_code`, `related_symbols`, `impact_analysis`, `get_context_pack`, `compact_command_output`, `savings_report` | 7 |
| `full` | all current tools | 11 |
| `debug` | all current tools | 11 |
| `readonly` | all current tools for now; future mutation tools must be hidden | 11 |
| `editing` | same as `optimized` for now; reserved for future symbolic editing tools | 7 |
| `web-app` | same as `optimized` for now; future web workflow tools may prioritize TypeScript, JavaScript, C#, ASP.NET Core, React, Next.js, Angular, Node.js, REST APIs, routes, components, data access metadata, realtime/socket metadata, messaging metadata, and infrastructure metadata | 7 |
| `enterprise` | `find_symbol`, `search_code`, `related_symbols`, `impact_analysis`, `get_context_pack`, `trace_dependency`, `detect_cycles`, `compact_command_output`, `savings_report` | 9 |

Future mutation tools such as `preview_edit`, `apply_edit`, and `rename_symbol`
must remain hidden from `readonly` and appear only in `editing`, `full`, or
`debug` when explicitly allowed.

## Manifest Slimming

`tools/list` returns only enabled tools for the selected profile. Tool
descriptions are concise, and input schemas preserve required fields without
long examples or roadmap text. Existing request DTOs remain compatible,
including nested scope fields:

- `scope.project_id`
- `scope.branch_id`

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

### `compact_command_output`
- Purpose: compact provided stdout/stderr into a smaller local summary.
- Input: `CompactCommandOutputRequest`
- Output: `CommandOutputSummary`
- Example: `{"command":"cargo test","stdout":"test result: ok","stderr":"","exit_code":0,"max_bytes":4000}`
- Supported families: `git`, `cargo`, `dotnet`, `npm`, `pnpm`, `yarn`, `ng`,
  `tsc`, `eslint`, `docker`, `docker compose`, `rg`, `grep`, `cat`, `tree`,
  and unknown commands.
- Token saving: estimates savings from byte reduction and preserves key errors,
  warnings, exit status, stderr, and truncation metadata.
- Safety: does not execute commands, shell out, call external APIs, call an LLM,
  upload output, or emit telemetry.

## Validation

The runtime rejects empty queries/symbol IDs, missing scope, zero or excessive
limits, invalid depth, invalid token budgets, invalid confidence values, and
unknown edge filters with structured `McpToolError` responses. The compaction
tool rejects empty commands, missing stdout/stderr, and invalid byte budgets.
