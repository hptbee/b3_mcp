# B3 Web UI

The web UI is a local-only Next.js frontend for inspecting and controlling the B3 MCP code intelligence platform through `b3-control-server`.

It does not access SQLite directly, own indexing logic, generate embeddings, upload source code, use SaaS auth, or emit telemetry. The UI can trigger local indexing through the control server.

## Run the Control Server

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

The control server binds to `127.0.0.1:7777` by default.

To enable local file watching and changed-file indexing:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777 --watch --debounce-ms 500
```

To populate the local database before opening the UI:

```powershell
cargo run -p b3-control --bin b3-control-server -- init --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db"
```

## Run the Web UI

```powershell
cd apps/web-ui
npm install
npm run dev
```

Open `http://127.0.0.1:8888`.

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

The Project Status section shows the current project path, database path,
indexed file/symbol/edge counts, last index status, parse failures, and local
duration. It also includes `Run Index` and `Reindex Project` buttons. Reindex is
safe incremental reindexing in this phase and skips unchanged files. Scoped
indexing is available through the control server API/CLI, but dedicated scoped
indexing controls in the Web UI are deferred.

Node.js REST route metadata is available through the local control API, but the
current Web UI does not include a dedicated route browser yet.

Next.js route metadata is available through the same local route API with
`framework=nextjs`, but the current Web UI does not include dedicated Next.js
route/page views yet.

Angular route and component metadata is available through the same local route
and component APIs with `framework=angular`, but the current Web UI does not
include dedicated Angular views yet.

ASP.NET Core route metadata is available through the same local route API with
`framework=aspnetcore`, but the current Web UI does not include dedicated
ASP.NET Core views yet.

ORM/database access metadata is available through the local data access API,
but the current Web UI does not include dedicated ORM/database views yet.

Realtime/socket metadata is available through the local realtime API, but the
current Web UI does not include dedicated realtime/socket views yet.

Messaging/event-driven metadata is available through the local messaging API,
but the current Web UI does not include dedicated messaging views yet.

Cloud/infrastructure metadata is available through the local infrastructure
API, but the current Web UI does not include dedicated infrastructure views yet.

React/TSX component metadata is available through the local control API, but
the current Web UI does not include a dedicated component browser yet.

## Registry Visibility

Phase 8.8 registry and project group support is CLI-only for now. Web UI
registry visibility is deferred until control server registry APIs are added.
The current UI remains single-project and continues to use the configured local
control server.

## Language Capabilities

The control server now exposes language backend metadata at `/api/languages`,
but the Web UI does not render a dedicated language support table yet. This is
deferred to a later UI pass. Current capability truth is Rust implemented,
JavaScript/TypeScript/JSX/TSX basic indexing, Node REST basic static route
metadata, React/TSX basic static component metadata, Next.js basic static route
and boundary metadata, Angular basic static decorator/route/component metadata,
C# basic static Web API metadata, Go basic static symbols/imports/route hints,
and LSP disabled by default.

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
- `parser_worker_started`
- `parser_worker_completed`
- `parser_worker_timeout`
- `parser_worker_crashed`
- `parse_failed`
- `parse_retried`
- `parse_failure_recorded`

Manual index runs can emit the same indexing and parser lifecycle events.

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
- Phase 10.4 exposes local hybrid search through `POST /api/search/hybrid` and
  the MCP `semantic_search` tool on top of SQLite-backed storage and `b3-query`
  ranking, but no Web UI redesign or dedicated semantic search UI is included.
  Phase 10.5 adds local fixture-based benchmark output, but no Web UI redesign
  or benchmark dashboard is included.
- Phase 11.0 adds architecture contracts and `GET /api/architecture/status`,
  and Phase 11.1 adds group federation status/summary endpoints, but no
  architecture graph UI or service map UI is included yet.
- Phase 11.1.1 adds local benchmark JSON/terminal output for context
  efficiency and tool-call reduction. It does not add a benchmark dashboard,
  architecture graph UI, service map UI, or Web UI redesign.
- Phase 11.2 adds a local Control API route/API matching endpoint, but no
  architecture graph UI, service map UI, dedicated route-match browser, or Web
  UI redesign is included.
- Phase 11.3 adds a local Control API messaging matching endpoint, but no
  architecture graph UI, service map UI, dedicated message-match browser, or
  Web UI redesign is included.
- Phase 11.4 adds a local Control API package/contract/infra matching endpoint,
  but no architecture graph UI, service map UI, dedicated dependency-match
  browser, group impact UI, or Web UI redesign is included.
- Phase 11.5 adds a local Control API group impact/context-pack endpoint, but
  no architecture graph UI, service map UI, dedicated impact browser, graph
  visualization, or Web UI redesign is included.
- Parser subprocess isolation diagnostics are available through the control server, but the UI only shows them through the existing diagnostics/raw event surfaces.
- A dedicated Node.js REST route browser is deferred; route metadata is
  currently available through `GET /api/routes`.
- A dedicated React component browser is deferred; component metadata is
  currently available through `GET /api/components`.
- Dedicated Next.js route/page views are deferred; Next.js metadata is currently
  available through `GET /api/routes?framework=nextjs`.
- Dedicated Angular views are deferred; Angular route/component metadata is
  currently available through `GET /api/routes?framework=angular` and
  `GET /api/components?framework=angular`.
- Dedicated ASP.NET Core views are deferred; ASP.NET Core route metadata is
  currently available through `GET /api/routes?framework=aspnetcore`.
- Dedicated ORM/database views are deferred; data access metadata is currently
  available through `GET /api/data-access`.
- Dedicated realtime/socket views are deferred; realtime metadata is currently
  available through `GET /api/realtime`.
- Dedicated messaging views are deferred; messaging metadata is currently
  available through `GET /api/messaging`.
- Dedicated cloud/infrastructure views are deferred; infrastructure metadata is
  currently available through `GET /api/infrastructure`.
- Dedicated Go views are deferred; Go symbols use existing symbol/query
  surfaces and Go route hints are available through `GET /api/routes`.
- Dedicated WPF/XAML views are deferred; WPF metadata is currently available
  through `GET /api/wpf`.
- Future technology intelligence views are deferred until the underlying backend
  intelligence is completed.
- The broader Web UI Developer Console Refresh remains deferred until after
  core intelligence and semantic search work.
- Config mutation/save is deferred.
- SSE currently reflects the minimal control-server event stream.
