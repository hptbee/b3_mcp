# B3 Web UI

The web UI is a local-only Next.js frontend for inspecting and controlling the B3 MCP code intelligence platform through `b3-control-server`.

It does not access SQLite directly, run indexing, generate embeddings, upload source code, use SaaS auth, or emit telemetry.

## Run the Control Server

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

The control server binds to `127.0.0.1:7777` by default.

To enable local file watching and changed-file indexing:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777 --watch --debounce-ms 500
```

## Run the Web UI

```powershell
cd apps/web-ui
npm install
npm run dev
```

Open `http://127.0.0.1:3000`.

## Environment Variables

`NEXT_PUBLIC_B3_API_BASE_URL` controls the control-server base URL.

Default:

```text
http://127.0.0.1:7777
```

Example:

```powershell
$env:NEXT_PUBLIC_B3_API_BASE_URL="http://127.0.0.1:7777"
npm run dev
```

## Sections

- Dashboard
- Project Status
- Query Playground
- Query Trace
- Graph Explorer
- Token Savings
- Diagnostics
- Config Viewer
- Capabilities
- Logs / Events

## Logs / Events

The Logs / Events section connects to `GET /api/events` with SSE and displays whatever local server events are emitted.

Watch mode can emit:

- `watcher_started`
- `file_changed`
- `file_created`
- `file_deleted`
- `file_renamed`
- `debounce_flushed`
- `indexing_started`
- `file_indexed`
- `file_skipped`
- `indexing_completed`
- `indexing_failed`

Events are local-only and are not sent to any remote service.

## Graph Explorer

The Graph Explorer section uses React Flow to visualize graph nodes and edges returned by the local control server. When the local SQLite graph has indexed nodes and edges, the graph endpoints return real bounded data. Empty repositories return empty graph responses rather than fake data.

It calls:

- `GET /api/graph/summary`
- `POST /api/graph/neighbors`
- `POST /api/graph/path`
- `POST /api/graph/cycles`
- `POST /api/graph/centrality`

Controls are bounded by default:

- `project_id = "default"`
- `branch_id = "main"`
- `direction = "both"`
- `max_depth = 1`
- `limit = 50`
- `min_confidence = 0`

The UI never requests an unbounded graph dump. Depth inputs are capped in the UI, limits are capped in the UI, and every request includes an explicit scope.

Graph Explorer panels:

- Neighbor expansion from a seed node or selected node
- Node and edge property inspector
- Dependency path query
- Cycle detection result viewer
- Centrality result table

Centrality is cached-only: if no centrality snapshot exists, the UI shows an empty result with the control server message instead of triggering expensive computation.

## Query Trace

The Query Trace section sends query requests with `include_trace = true` and renders any explainability fields returned by the control server.

Supported query endpoints:

- `POST /api/query/find-symbol`
- `POST /api/query/search-code`
- `POST /api/query/impact-analysis`
- `POST /api/query/context-pack`
- `POST /api/query/related-symbols`
- `POST /api/query/trace-dependency`

Default trace controls:

- `project_id = "default"`
- `branch_id = "main"`
- `token_budget = 1200`
- `max_depth = 2`
- `limit = 20`
- `min_confidence = 0`
- `include_trace = true`

The request uses nested scope:

```json
{
  "scope": {
    "project_id": "default",
    "branch_id": "main"
  },
  "include_trace": true
}
```

Trace timeline stages are shown only when present:

- query input and intent
- exact and FTS hits
- graph traversal steps
- ranking decisions
- selected and skipped context items
- truncation metadata
- token budget usage
- token savings estimates
- warnings

Ranking decisions are displayed as score contribution rows when returned. Missing fields are left as `not returned`; the UI does not infer scores.

The Context Pack Inspector separates selected and skipped items, shows skip reasons and provenance when available, and collapses long snippets by default to avoid huge raw file dumps.

The Raw JSON panel exposes request, response, and trace payloads for debugging future MCP responses.

## Local-Only Security Model

- UI talks only to the configured localhost control server.
- No telemetry is included.
- No cloud auth is included.
- No external API is required.
- No source upload is implemented.

## Known Limitations

- Advanced graph layouts and whole-repo visual analysis are deferred.
- Centrality requires a cached snapshot; it is not computed automatically by the UI.
- Query trace quality depends on trace fields returned by the control server query endpoints.
- Embeddings and semantic search are deferred.
- Parser subprocess isolation is deferred.
- Config mutation/save is deferred.
- SSE currently reflects the minimal control-server event stream.
