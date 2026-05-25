# B3 Control Server

The control server is local developer tooling for the future localhost UI. It is an adapter over the core contracts and local SQLite storage; it can trigger the indexer, but it does not own indexing logic, generate embeddings, run semantic search, or handle MCP protocol traffic.

## Run

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

By default the server binds to `127.0.0.1:7777`. The Web UI development
server runs separately on `http://127.0.0.1:8888` and uses this local control
server URL by default.

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

## Project Init And Manual Index

Initialize a single local project database without indexing files:

```powershell
cargo run -p b3-control --bin b3-control-server -- init --project "." --database ".b3/b3.db"
```

Index the project once:

```powershell
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db"
```

Reindex currently uses the same safe incremental behavior as `index`: unchanged
files are skipped by content hash and deleted files are cleaned for the current
branch. It does not delete unrelated project data.

```powershell
cargo run -p b3-control --bin b3-control-server -- reindex --project "." --database ".b3/b3.db"
```

This phase is single-project only. Multi-repo registry and project groups are
deferred to Phase 8.8.

The `b3` install helper prints these init/index/serve commands as local next
steps after generating Codex or Cursor MCP config. It does not run init,
indexing, or the control server automatically.

## Registry And Groups

Phase 8.8 adds local registry and project group commands to the `b3` CLI. The
registry is metadata-only JSON at `~/.b3/registry.json` by default, and each
registered project still uses its own repo-local `.b3/b3.db`.

Control server registry APIs are deferred. Existing single-project control
server commands and endpoints do not require the registry.

## Endpoints

Health and project status:

- `GET /health`
- `GET /api/status`
- `GET /api/projects`
- `GET /api/project`

Manual indexing:

- `POST /api/index/run`
- `POST /api/index/reindex`
- `GET /api/index/status`

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
- `GET /api/languages`
- `GET /api/lsp/status`
- `GET /api/lsp/servers`
- `GET /api/routes`
- `GET /api/components`
- `GET /api/config`
- `POST /api/config/validate`
- `GET /api/events`

The event stream emits server, watcher, debounce, indexing, and parser worker lifecycle events. Manual index requests can emit `indexing_started`, `file_indexed`, `file_skipped`, `indexing_completed`, `indexing_failed`, and `parse_failed`.

## Language Capabilities

`GET /api/capabilities` includes language backend metadata. `GET
/api/languages` returns the language registry directly.

Current truth:

- Rust has `Good` tree-sitter support.
- JavaScript, TypeScript, JSX, and TSX have `Basic` local tree-sitter support.
- C# has `Basic` local static ASP.NET Core / Web API support.
- Node.js REST route intelligence is `Basic`, static, and local for Express,
  NestJS, and Fastify.
- React/TSX component intelligence is `Basic`, static, and local for common
  component declarations, props type names, JSX usages, and hooks.
- Next.js intelligence is `Basic`, static, and local for App Router and Pages
  Router file-system routes, app route handlers, exported HTTP methods, dynamic
  segments, and `"use client"` / `"use server"` boundaries.
- Angular intelligence is `Basic`, static, and local for common decorators,
  components, services, modules, route configs, selectors, template/style
  references, and constructor DI type names.
- ASP.NET Core / C# Web API intelligence is `Basic`, static, and local for
  `.csproj` ASP.NET Core detection, controllers, common route attributes,
  composed action routes, and constructor DI type names.
- LSP metadata is exposed, but LSP remains disabled by default.
- Semantic search and deeper framework intelligence remain deferred.

LSP endpoints are metadata-only in Phase 9.1. They report the local LSP backend foundation, disabled-by-default config, and configured server availability; they do not install language servers, contact cloud services, or add MCP tools.

`GET /api/languages` reports Rust as Good tree-sitter support, JavaScript/TypeScript/JSX/TSX as Basic local tree-sitter support, and C# as Basic static ASP.NET Core / Web API support.

`GET /api/routes` returns locally indexed Node.js REST, Next.js, Angular, and
ASP.NET Core route
metadata from `Route` symbols. Optional filters include `project_id`,
`branch_id`, `framework`, `method`, `path`, and `limit`. Next.js records use
`framework=nextjs` and include a `route_kind` such as `page`, `layout`,
`loading`, `error`, `not_found`, or `api`. Angular route config records use
`framework=angular` and `route_kind=route`. ASP.NET Core route records use
`framework=aspnetcore` and `route_kind=api`.

`GET /api/components` returns locally indexed React and Angular component metadata from
component symbols. Optional filters include `project_id`, `branch_id`,
`framework`, `name`, `file`, and `limit`.

Route support is intentionally basic and static. It does not execute `npm`,
`node`, `tsc`, `eslint`, `dotnet`, framework CLIs, package registries, app code, or
runtime routing. It does not infer deep middleware order, Nest module graphs,
guards/interceptors/pipes, deep dependency injection, or request lifecycles.
For Next.js it also does not run `next dev`, `next build`, package scripts, or
deployment tooling and does not infer full RSC semantics, middleware execution
order, Vercel behavior, auth behavior, or deep data fetching semantics.
For Angular it also does not run `ng`, the Angular compiler, package scripts,
or app code and does not infer full template type semantics, lifecycle runtime,
deep DI/module graph behavior, RxJS/NgRx flow, guards, or resolvers.

Component support is intentionally basic and static. It does not execute
`npm`, `node`, `tsc`, `eslint`, React dev servers, package registries, app code,
or runtime rendering. It does not infer state machines, deep hook semantics,
full JSX tree graphs, CSS/layout, or framework-specific router behavior.

## Examples

```powershell
curl http://127.0.0.1:7777/health
curl http://127.0.0.1:7777/api/status
curl http://127.0.0.1:7777/api/graph/summary
curl http://127.0.0.1:7777/api/index/status
```

```powershell
curl -X POST http://127.0.0.1:7777/api/index/run
curl -X POST http://127.0.0.1:7777/api/index/reindex
curl http://127.0.0.1:7777/api/lsp/status
curl http://127.0.0.1:7777/api/routes?framework=express
curl http://127.0.0.1:7777/api/routes?framework=nextjs
curl http://127.0.0.1:7777/api/routes?framework=aspnetcore
curl http://127.0.0.1:7777/api/components?framework=react
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
- Dedicated route browser UI is deferred; route metadata is exposed through the
  control API.
- Dedicated component browser UI is deferred; component metadata is exposed
  through the control API.
- Manual indexing is synchronous for this phase; `GET /api/index/status`
  reports the current or last run for the current server process.

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
