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

Scoped indexing can preview or run targeted index work without broadening to
the whole project:

```powershell
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db" --scope "path:src/orders" --dry-run
cargo run -p b3-control --bin b3-control-server -- reindex --project "." --database ".b3/b3.db" --scope "glob:**/*.controller.ts"
```

Supported scope forms include `project`, `path:<relative-path>`,
`file:<relative-file>`, `glob:<pattern>`, `language:<id>`,
`framework:<id>`, `route:<path>`, `component:<name>`, `module:<name>`,
`data_access:<technology>`, `realtime:<technology>`,
`messaging.topic:<topic>`, `messaging.queue:<queue>`,
`messaging.routing_key:<routing-key>`, and `infrastructure:<technology>`.
Target scopes use existing indexed metadata and may return zero matches until a
broader path/project index has populated the local database.

This phase is single-project only. Multi-repo registry and project groups stay
metadata-only.

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

- `POST /api/index/preview`
- `POST /api/index/run`
- `POST /api/index/reindex`
- `GET /api/index/status`

`POST /api/index/run` and `POST /api/index/reindex` accept optional JSON
fields:

```json
{
  "scope": "framework:aspnetcore",
  "dry_run": true,
  "force": false
}
```

Dry-run responses include matched file counts, sample files, matched languages,
matched frameworks, existing metadata targets when available, warnings, and
skipped reasons. Dry-run does not mutate SQLite.

Symbolic editing:

- `POST /api/edit/preview`
- `POST /api/edit/apply`

`POST /api/edit/preview` accepts a local edit request and returns a deterministic
plan, compact snippets, warnings, and unified diff text without mutating files.
`POST /api/edit/apply` uses the same request shape, but mutates only when
`mode` is `apply` and `dry_run` is `false`. Apply re-reads the target file,
checks the planned text/hash, creates a local backup by default, and returns a
reindex-recommended warning. Phase 12 is single-file only; MCP edit tools remain
deferred.

Rename/refactor:

- `POST /api/refactor/rename/preview`
- `POST /api/refactor/rename/apply`

`POST /api/refactor/rename/preview` returns a bounded local rename plan for an
indexed symbol target and conservative identifier occurrences. Preview does not
mutate files. `POST /api/refactor/rename/apply` mutates only when `mode` is
`apply` and `dry_run` is `false`; it validates all changed files before writes,
creates backups by default, and returns `reindex_recommended=true`. Phase 13 is
not an IDE-grade semantic rename engine and does not run compilers, formatters,
language servers, package managers, Git commands, or external APIs.

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
- `POST /api/search/hybrid`

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
- `GET /api/architecture/status`
- `GET /api/architecture/groups`
- `GET /api/architecture/groups/{group_id}/status`
- `GET /api/architecture/groups/{group_id}/summary`
- `GET /api/architecture/groups/{group_id}/route-matches`
- `GET /api/architecture/groups/{group_id}/message-matches`
- `GET /api/architecture/groups/{group_id}/dependency-matches`
- `POST /api/architecture/groups/{group_id}/impact`
- `GET /api/architecture/groups/{group_id}/graph`
- `GET /api/architecture/groups/{group_id}/service-map`
- `GET /api/vector/status`
- `GET /api/vector/providers`
- `GET /api/vector/stats`
- `GET /api/languages`
- `GET /api/lsp/status`
- `GET /api/lsp/servers`
- `GET /api/routes`
- `GET /api/components`
- `GET /api/data-access`
- `GET /api/realtime`
- `GET /api/messaging`
- `GET /api/infrastructure`
- `GET /api/wpf`
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
- ORM/database access intelligence is `Basic`, static, and local for EF Core,
  Dapper, Prisma, TypeORM, and Sequelize package/use detection plus obvious
  query callsites and operations.
- Realtime/socket intelligence is `Basic`, static, and local for WebSocket,
  Socket.IO, SignalR, and minimal RSocket package/request metadata.
- Messaging/event-driven intelligence is `Basic`, static, and local for AMQP,
  RabbitMQ, Kafka, Google Pub/Sub, and NestJS messaging package/use detection
  plus obvious producer/consumer callsites and literal topics, queues,
  exchanges, routing keys, and patterns.
- Cloud/infrastructure intelligence is `Basic`, static, and local for
  Dockerfile, Docker Compose, Kubernetes YAML, Terraform, and visible GCP/GKE
  hints, including images, services, workloads, ports, env keys, providers,
  resources, modules, variables, and outputs.
- Go language support is `Basic`, static, and local for `.go` and `go.mod`
  detection, packages, imports, functions, receiver methods, structs,
  interfaces, type declarations, const/var declarations, local call edges, and
  conservative HTTP route hints.
- .NET Desktop / WPF intelligence is `Basic`, static, and local for modern and
  older WPF project hints, XAML Application/Window/UserControl/Page/
  ResourceDictionary metadata, `x:Class`, code-behind hints, binding paths,
  command bindings, resource references, static DataContext hints, and
  ViewModel naming hints.
- Scoped indexing is available for path/file/glob/language/framework scopes
  and for existing metadata targets such as routes, components, data access,
  realtime, messaging, and infrastructure.
- LSP metadata is exposed, but LSP remains disabled by default.
- Vector architecture is available locally through read-only status/providers/
  stats endpoints. Phase 10.2 adds SQLite vector persistence and local
  brute-force cosine search over filtered SQLite candidates. Embeddings remain
  disabled by default, `vector_search_ready` is true for raw local vector
  search. Phase 10.3 adds hybrid ranking inside `b3-query`, and Phase 10.4
  exposes local hybrid search through `POST /api/search/hybrid` plus the MCP
  `semantic_search` tool. `semantic_search_ready` is true for this local/offline
  hybrid path. Phase 10.5 benchmarks this path with local fixture-based quality
  metrics only.
- Cross-project semantic search and deeper framework intelligence remain deferred.
- Phase 11.0 architecture contracts are available locally. Phase 11.1 adds
  local group federation endpoints. `GET /api/architecture/status` reports
  architecture contracts, group federation, route matching, messaging matching,
  package/contract/infra matching, group impact, group context packs,
  architecture graph API, and service maps ready, while architecture graph UI is
  not ready yet. Group endpoints read the local registry and
  existing repo-local project DBs only; no global database merge, cloud graph
  database, hosted vector database, telemetry, or remote lookup is required.
- Phase 11.1.1 adds benchmark-only efficiency checks for the existing
  `POST /api/search/hybrid` path and MCP profile/tool availability. It does not
  add Control API endpoints, service maps, group impact APIs, route/API
  matching, messaging matching, telemetry, or external service calls.
- Phase 11.2 adds read-only
  `GET /api/architecture/groups/{group_id}/route-matches`. The endpoint reads
  registry-defined local project DBs independently, returns static route/API
  match candidates with confidence/evidence/warnings, and reports
  `route_matching_ready = true`. It does not merge DBs, execute HTTP requests,
  fetch remote OpenAPI documents, call external APIs, or add service-map/group
  impact APIs.
- Phase 11.3 adds read-only
  `GET /api/architecture/groups/{group_id}/message-matches`. The endpoint reads
  existing local messaging metadata from registry project DBs, returns static
  producer/consumer match candidates with confidence/evidence/warnings, and
  reports `messaging_matching_ready = true`. It does not connect to RabbitMQ,
  Kafka, Pub/Sub, or any broker; it does not publish/consume messages, call
  cloud APIs, merge DBs, or add service-map/group impact APIs.
- Phase 11.4 adds read-only
  `GET /api/architecture/groups/{group_id}/dependency-matches`. The endpoint
  reads local manifest, contract/schema, and infrastructure metadata from
  registry project DBs, returns static package/contract/infra match candidates
  with confidence/evidence/warnings, and reports
  `dependency_matching_ready = true`. It does not run package managers, Docker,
  Kubernetes, Terraform, cloud CLIs, remote schema fetches, schema validation,
  cloud APIs, DB merges, or service-map/group impact APIs.
- Phase 11.5 adds read-only
  `POST /api/architecture/groups/{group_id}/impact`. The endpoint resolves a
  local seed, traverses existing route/message/dependency match candidates
  within bounded depth/limit settings, and can return a bounded cross-repo
  context pack. It does not run package managers, Docker, Kubernetes,
  Terraform, cloud CLIs, brokers, runtime HTTP calls, cloud APIs, DB merges,
  service-map APIs, or graph UI behavior.
- Phase 11.6 adds read-only
  `GET /api/architecture/groups/{group_id}/graph` and
  `GET /api/architecture/groups/{group_id}/service-map`. These endpoints build
  bounded static graph and project-level service-map responses on demand from
  existing route, messaging, dependency, impact, and federation metadata. They
  do not persist a global graph, merge DBs, run package managers, run Docker,
  Kubernetes, Terraform, or cloud CLIs, connect to brokers, execute HTTP
  requests, call external APIs, or implement a graph UI.
- Phase 11.7 adds benchmark coverage and docs for these Phase 11 Control/API
  capabilities through `cargo run -p b3-bench -- baseline`. The benchmark uses
  library/API-equivalent local calls and does not start network listeners, call
  external APIs, execute runtime HTTP requests, connect to brokers, merge DBs,
  or implement graph UI.

LSP endpoints are metadata-only in Phase 9.1. They report the local LSP backend foundation, disabled-by-default config, and configured server availability; they do not install language servers, contact cloud services, or add MCP tools.

`GET /api/languages` reports Rust as Good tree-sitter support, JavaScript/TypeScript/JSX/TSX as Basic local tree-sitter support, C# as Basic static ASP.NET Core / Web API support, Go as Basic static support, Python/Java/Kotlin/PHP/Ruby as Basic static backend language support, and Phase 15 C/C++/Swift/Objective-C/Dart plus YAML/JSON/TOML/XML/HTML/CSS/SCSS/Three.js/WebGL/ksqlDB as Basic static support or Basic static hints.

Phase 14 backend language support is static/local/offline only. It exposes symbols/imports and conservative route/data-access/messaging hints through the existing APIs, and it does not run package managers, compilers, runtimes, language servers, external APIs, cloud services, telemetry, or internet access.

Phase 15 systems/mobile/config/web support is static/local/offline only. It exposes conservative symbols, imports/includes, safe config key paths, package/dependency names, template/style/asset references, route/client hints, XAML metadata hardening, Three.js/WebGL hints, and ksqlDB messaging/data-flow hints through existing symbol and metadata shapes. It does not run compilers, package managers, formatters, runtimes, browsers, WebGL, Docker/Kubernetes/Terraform, Kafka, ksqlDB, brokers, databases, language servers, external APIs, cloud services, telemetry, or internet access.

Phase 16 config/data/web hardening is static/local/offline only. It adds shared secret redaction, safe env-example parsing, key-only/redacted handling for real env files, static env/config reference hints, hardened YAML/JSON/TOML/XML metadata, HTML/template route and asset hints, CSS/SCSS media/import/asset hints, SQL table-reference metadata, stricter ksqlDB dependency metadata, and stronger Three.js/WebGL asset hints. It does not read OS environment, run SQL, connect to databases, Kafka, ksqlDB, RabbitMQ, brokers, browsers, WebGL, cloud services, or external APIs.

Phase 17 adds a quality-audit status block to `GET /api/capabilities`. It reports that the support matrix, capability truthfulness, fixture coverage, metadata consistency, secret redaction, false-positive guardrails, cross-surface integration, and benchmark claims were audited. The block explicitly keeps runtime validation, compiler-grade parsing, IDE-grade refactor, architecture graph UI, full Git Intelligence, mandatory LSP, external APIs, cloud services, and telemetry as false/deferred.

## Phase 21 Git Intelligence API Plan

Phase 21.4 adds internal changed-file and diff-summary readers only. No Git
endpoints are exposed yet and `GET /api/capabilities` is unchanged in this
checkpoint.

Planned Phase 21 Control API endpoints are:

- `GET /api/git/status`
- `GET /api/git/branches`
- `GET /api/git/changed-files`
- `GET /api/git/diff-summary`
- `POST /api/git/compare`
- `POST /api/git/impact`

Future Git endpoints must be local-only and read-only. They may report no-git,
dirty worktree, detached HEAD, stale index, changed files, local branch, and
local diff information, but must not run mutating Git commands, call GitHub,
GitLab, Bitbucket, or other remote APIs, checkout branches, fetch/pull/push,
stash/reset/clean, write `.git`, modify the working tree, or auto-reindex.

Manual reindex actions and any auto-index toggle remain future Control/Web UI
work. Auto-index must be off or conservative by default and must never run on
branch change, commit change, detached HEAD, conflicts, unknown Git state,
no-git projects, excessive changed files, unsafe delete/rename batches, indexed
branch mismatch, or indexed commit mismatch.

The indexed Git snapshot is persisted internally for later stale-index
detection, and Phase 21.3 can evaluate freshness internally, but neither is
exposed through a public Control API yet.

Auto full reindex after branch or commit changes is forbidden. Phase 21.3 does
not execute auto-index; Phase 21.4 still does not execute auto-index and only
adds bounded changed-file detail for later APIs/UI.

`GET /api/routes` returns locally indexed Node.js REST, Next.js, Angular,
ASP.NET Core, and Go route
metadata from `Route` symbols. Optional filters include `project_id`,
`branch_id`, `framework`, `method`, `path`, and `limit`. Next.js records use
`framework=nextjs` and include a `route_kind` such as `page`, `layout`,
`loading`, `error`, `not_found`, or `api`. Angular route config records use
`framework=angular` and `route_kind=route`. ASP.NET Core route records use
`framework=aspnetcore`. Go route hints use `framework=go_net_http`, `gin`,
`echo`, `fiber`, or `chi` when detected.
`framework=aspnetcore` and `route_kind=api`.

`GET /api/components` returns locally indexed React and Angular component metadata from
component symbols. Optional filters include `project_id`, `branch_id`,
`framework`, `name`, `file`, and `limit`.

`GET /api/data-access` returns locally indexed static data access metadata from
symbols with `data_access.*` metadata. Optional filters include `project_id`,
`branch_id`, `technology`, `kind`, `operation`, `file`, and `limit`. Supported
technology values are `ef_core`, `dapper`, `prisma`, `typeorm`, `sequelize`,
and `raw_sql` for driver/raw-SQL hints.

`GET /api/realtime` returns locally indexed static realtime/socket metadata from
symbols with `realtime.*` metadata. Optional filters include `project_id`,
`branch_id`, `technology`, `kind`, `event`, `file`, and `limit`. Supported
technology values are `websocket`, `socketio`, `signalr`, and `rsocket`.
Responses include technology, kind, direction, event/channel/hub/method/endpoint
where available, file, symbol, class/function name, line range, confidence, and
source kind.

`GET /api/messaging` returns locally indexed static messaging/event-driven
metadata from symbols with `messaging.*` metadata. Optional filters include
`project_id`, `branch_id`, `technology`, `kind`, `topic`, `queue`,
`routing_key`, and `limit`. Supported technology values include `amqp`,
`rabbitmq`, `kafka`, `google_pubsub`, `pubsub`, and `nestjs_messaging`.
Responses include technology, kind, direction, topic/queue/exchange/routing key
/pattern where available, file, symbol, class/function/method name, line range,
confidence, and source kind.

`GET /api/infrastructure` returns locally indexed static cloud/infrastructure
metadata from symbols with `infrastructure.*` metadata. Optional filters include
`project_id`, `branch_id`, `technology`, `kind`, `name`, and `limit`.
Supported technology values include `docker`, `docker_compose`, `kubernetes`,
`terraform`, `gcp`, and `gke`. Responses include technology, kind, name,
resource type, provider, image, service/container name, namespace,
ports/env keys/labels/selectors where available, file, symbol, line range,
confidence, and source kind.

`GET /api/wpf` returns locally indexed static .NET Desktop / WPF metadata from
symbols with `wpf.*` metadata. Optional filters include `project_id`,
`branch_id`, `kind`, `binding`, `command`, and `limit`. Responses include
technology, kind, name, `x_class`, code-behind path, ViewModel hint, binding
paths, command bindings, resource keys, resource sources, DataContext hint,
file, symbol, line range, confidence, and source kind.

Route support is intentionally basic and static. It does not execute `npm`,
`node`, `tsc`, `eslint`, `dotnet`, framework CLIs, package registries, app code, or
runtime routing. It does not infer deep middleware order, Nest module graphs,
guards/interceptors/pipes, deep dependency injection, or request lifecycles.
For Next.js it also does not run `next dev`, `next build`, package scripts, or
deployment tooling and does not infer full RSC semantics, middleware execution
order, Vercel behavior, auth behavior, or deep data fetching semantics.

Data access support is intentionally basic and static. It does not connect to
databases, execute SQL, run migrations, run `dotnet`, `node`, `npm`, Prisma
generate, TypeORM CLI, Sequelize CLI, package registries, app code, or runtime
ORM behavior. It does not infer full SQL semantics, full LINQ semantics,
schema introspection, or cross-project data lineage.
For Angular it also does not run `ng`, the Angular compiler, package scripts,
or app code and does not infer full template type semantics, lifecycle runtime,
deep DI/module graph behavior, RxJS/NgRx flow, guards, or resolvers.

WPF support is intentionally basic and static. It does not run Visual Studio,
MSBuild, `dotnet`, a WPF app, a XAML compiler, or designer tooling. It does not
perform full binding type checking, runtime DataContext inference, deep MVVM
framework analysis, or cross-project view/model matching.

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
curl http://127.0.0.1:7777/api/data-access?technology=ef_core
curl http://127.0.0.1:7777/api/realtime?technology=socketio
curl http://127.0.0.1:7777/api/messaging?technology=kafka
curl http://127.0.0.1:7777/api/infrastructure?technology=kubernetes
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
- Dedicated realtime/socket browser UI is deferred; realtime metadata is exposed
  through `GET /api/realtime`.
- Dedicated messaging browser UI is deferred; messaging metadata is exposed
  through `GET /api/messaging`.
- Dedicated cloud/infrastructure browser UI is deferred; infrastructure
  metadata is exposed through `GET /api/infrastructure`.
- Dedicated Go browser UI is deferred; Go symbols are available through
  existing symbol/query APIs and Go route hints through `GET /api/routes`.
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
