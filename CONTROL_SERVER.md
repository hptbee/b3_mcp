# B3 Control Server

The control server is local developer tooling for the future localhost UI. It is an adapter over the core contracts and local SQLite storage; it does not index files, generate embeddings, run semantic search, or handle MCP protocol traffic.

## Run

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

By default the server binds to `127.0.0.1:7777`.

Watch mode keeps the local index warm for changed files only:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777 --watch --debounce-ms 500
```

Watch mode ignores generated/vendor/runtime directories such as `.git`, `target`, `node_modules`, `.next`, and `.b3`.

## Localhost Security Model

- Local-only by default.
- Non-local bind addresses are rejected unless `--allow-non-local-bind` is passed explicitly.
- No auth is required for localhost mode.
- No telemetry is emitted.
- Source code is never uploaded.
- CORS accepts localhost UI origins only.

This server is intended for local development and agent control surfaces, not public internet exposure.

## Endpoints

Health and project status:

- `GET /health`
- `GET /api/status`
- `GET /api/projects`
- `GET /api/project`

Query adapter endpoints:

- `POST /api/query/find-symbol`
- `POST /api/query/search-code`
- `POST /api/query/find-callers`
- `POST /api/query/find-callees`
- `POST /api/query/related-symbols`
- `POST /api/query/impact-analysis`
- `POST /api/query/context-pack`
- `POST /api/query/trace-dependency`
- `POST /api/query/detect-cycles`

Graph adapter endpoints:

- `GET /api/graph/summary`
- `POST /api/graph/neighbors`
- `POST /api/graph/path`
- `POST /api/graph/cycles`
- `POST /api/graph/centrality`

Diagnostics and config:

- `GET /api/savings/summary`
- `GET /api/diagnostics`
- `GET /api/capabilities`
- `GET /api/config`
- `POST /api/config/validate`
- `GET /api/events`

The event stream emits server, watcher, debounce, indexing, and parser worker lifecycle events when watch mode is enabled.

## Examples

```powershell
curl http://127.0.0.1:7777/health
curl http://127.0.0.1:7777/api/status
curl http://127.0.0.1:7777/api/graph/summary
```

```powershell
curl -X POST http://127.0.0.1:7777/api/query/find-symbol `
  -H "Content-Type: application/json" `
  -d "{\"symbol\":\"run\",\"scope\":{\"project_id\":\"project\"},\"limit\":20}"
```

```powershell
curl -X POST http://127.0.0.1:7777/api/graph/neighbors `
  -H "Content-Type: application/json" `
  -d "{\"scope\":{\"project_id\":\"project\"},\"node_id\":\"node\",\"depth\":1,\"limit\":50}"
```

## Request Shape

Query and graph POST endpoints require a nested `scope` object:

```json
{
  "scope": {
    "project_id": "project",
    "branch_id": "main",
    "path_prefix": "crates/"
  },
  "limit": 50,
  "include_trace": false
}
```

Limits are bounded by the server. Full-file dumps and full graph dumps are disabled by default.

## Known Limitations

- Some query endpoints still return adapter placeholders while deeper query
  engine integration remains staged.
- Graph endpoints are bounded and backed by local SQLite graph data, but they
  intentionally avoid whole-repo unbounded dumps.
- Parser subprocess isolation is implemented as a local JSONL worker boundary,
  but in-process parsing remains the default compatibility mode.
- Embeddings and Qdrant integration are not implemented in this phase.
- Advanced parser diagnostics UI is deferred.

## Parser Diagnostics

`GET /api/diagnostics` includes parser isolation state:

- `parser.isolation_mode`
- `parser.timeout_ms`
- `parser.max_retries`
- `parser.worker_path`
- `parser.parse_failure_count`
- `parser.recent_parse_failures`

Parser failures are read from local SQLite table `parse_failures`. The control
server does not parse files itself; it only exposes diagnostics recorded by the
indexing boundary.

Parser-related SSE event names:

- `parser_worker_started`
- `parser_worker_completed`
- `parser_worker_timeout`
- `parser_worker_crashed`
- `parse_failed`
- `parse_retried`
- `parse_failure_recorded`
