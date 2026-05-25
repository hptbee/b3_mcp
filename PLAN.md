# B3 Project Plan

B3 is a local-first, offline-first, free-by-default AI-native code intelligence platform for coding agents and local developer workflows.

This document is the source of truth for the detailed roadmap. `README.md` should remain concise and traditional.

Current roadmap status:

- Completed: Phase 8.5 - Command Output Compaction
- Completed: Phase 8.5.1 - Project Init + Manual Index Command
- Completed: Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup
- Completed: Phase 8.6 - MCP Tool Profiles + Manifest Slimming
- Completed: Phase 8.7 - Agent Install Helper + Hook Integration Foundation
- Completed: Phase 8.8 - Multi-repo Registry + Project Groups
- Completed: Phase 9.0 - Language Backend Architecture
- Completed: Phase 9.1 - LSP Backend MVP
- Completed: Phase 9.2 - Web Application Priority Support A
- Completed: Phase 9.2.1 - Node.js / REST API Intelligence
- Completed: Phase 9.2.2 - React / TSX Component Intelligence
- Next: Phase 9.2.3 - Next.js Intelligence

---

## Vision

B3 helps AI coding agents understand local repositories without repeatedly grepping, reading full files, or dumping large context.

B3 combines:

- Rust-native core
- AST-aware indexing
- persistent local code graph
- SQLite/FTS storage
- query engine
- context packing
- token-saving retrieval
- MCP runtime
- MCP tool profiles and manifest slimming
- localhost control server
- local web UI
- graph explorer
- query trace UI
- file watcher
- parser isolation
- benchmark baseline
- command output compaction
- project init/index workflow
- MCP tool profiles
- agent install helper
- multi-repo registry
- project groups
- language backend architecture
- future LSP backend
- local CLI install/doctor/uninstall helper
- local multi-repo registry
- project group metadata
- language backend architecture
- local LSP backend foundation
- JavaScript/TypeScript/JSX/TSX basic indexing
- future framework, route, messaging, realtime, infrastructure, and cloud intelligence
- future symbolic editing
- future session memory
- future local embeddings and vector search

B3 is not a SaaS product. It must work locally and offline by default.

---

## Hard Requirement: Offline and Free

B3 must remain offline-first and free-by-default.

Core functionality must not require:

- external APIs
- cloud services
- hosted vector databases
- SaaS authentication
- telemetry
- paid UI kits
- paid backend services
- paid/proprietary plugins
- OpenAI / Anthropic / Gemini / cloud embedding APIs
- JetBrains paid plugin
- internet access

Allowed in core:

- local SQLite
- local FTS5
- local parser worker
- local benchmark harness
- local command output compaction
- local JSON registry
- local CLI helpers
- local LSP servers when explicitly enabled
- local embeddings when implemented later
- local vector storage/index when implemented later
- local Qdrant only as an optional local component when implemented later

External/cloud/paid integrations are allowed only as optional plugins:

- disabled by default
- not required for install
- not required for tests
- not required for benchmarks
- not required for core features

This requirement overrides all roadmap decisions.

---

## Design Principles

1. **Local-first and offline-first** â€” B3 must work without internet access.
2. **Free-by-default** â€” core functionality must be free to run locally.
3. **MCP runtime stays thin** â€” protocol/tool adapter only.
4. **Storage owns persistence** â€” SQLite schema, migrations, transactions, repositories, and adapters live in `b3-storage`.
5. **Indexer owns indexing** â€” parsing, discovery, hashing, incremental indexing, watcher, parser worker, parse failures, and language extraction live in `b3-indexer`.
6. **Query owns intelligence** â€” graph traversal, ranking, context packing, impact analysis, dependency tracing, cycle detection, and query trace live in `b3-query`.
7. **Control server is an adapter** â€” localhost HTTP/SSE only; no storage internals or query intelligence.
8. **Web UI talks only to local control server** â€” no cloud calls.
9. **Graph first, semantic second** â€” AST, graph, and FTS are core; embeddings are optional future secondary signals.
10. **Benchmark before optimization** â€” measure first, optimize measured bottlenecks only.
11. **Refactor only after verified milestones** â€” prefer small targeted refactors.
12. **Language support must be layered** â€” language syntax support, framework/library detection, and framework intelligence are separate layers.
13. **LSP complements tree-sitter** â€” tree-sitter for fast indexing; local LSP for semantic operations.
14. **Project model supports standalone and grouped projects** â€” `1 project = 1 .b3/b3.db` by default.
15. **Framework detection must be conservative** â€” do not invent routes, handlers, components, topics, or infrastructure relationships with low confidence.
16. **UI should follow data maturity** â€” major UI refresh should happen after enough backend intelligence exists to display useful information.

---

## Current Status

Completed:

```text
Phase 8.5 - Command Output Compaction
Phase 8.5.1 - Project Init + Manual Index Command
Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup
Phase 8.6 - MCP Tool Profiles + Manifest Slimming
Phase 8.7 - Agent Install Helper + Hook Integration Foundation
Phase 8.8 - Multi-repo Registry + Project Groups
Phase 9.0 - Language Backend Architecture
Phase 9.1 - LSP Backend MVP
Phase 9.2 - Web Application Priority Support A
Phase 9.2.1 - Node.js / REST API Intelligence
Phase 9.2.2 - React / TSX Component Intelligence
```

Recommended next:

```text
Phase 9.2.3 - Next.js Intelligence
```

Then:

```text
Phase 9.2.4 â€” Angular Intelligence
Phase 9.2.5 â€” ASP.NET Core / C# Web API Intelligence
Phase 9.2.6 â€” ORM / Database Access Intelligence
Phase 9.2.7 â€” Realtime / Socket Intelligence
Phase 9.2.8 â€” Messaging / Event-driven Intelligence
Phase 9.2.9 â€” Cloud / Infrastructure Intelligence
Phase 9.2.10 â€” Go Language Support
```

---

## Completed Phases

- Phase 1 â€” Workspace / Scaffold
- Phase 1.5 â€” Contracts / Boundaries
- Phase 2 â€” SQLite Storage / Schema Foundation
- Phase 3 â€” Incremental Indexer Skeleton
- Phase 3.1 â€” Indexer Audit / Cleanup
- Pre-Phase-4 â€” Plugin Contracts / Docs / CI
- Phase 4 â€” Real Rust Parsing + Storage Adapter
- Phase 4.1 â€” Project/Branch Auto Ensure + Deleted File Cleanup
- Phase 5 â€” Query Engine + Graph Traversal + Context Pack
- Phase 5.1 â€” Query Hardening + Retrieval Explainability
- Phase 5.2 â€” Ranking Algorithms Upgrade
- Phase 6 â€” MCP Tools over Query Engine
- Phase 6.0.1 â€” Live MCP Runtime Wiring
- Phase 6.1 â€” Impact Intelligence
- Phase 6.2 â€” PageRank / Centrality
- Phase 6.3 â€” MCP Runtime Hardening + Real-world Smoke Test
- Phase 7 â€” Control Server + Localhost API
- Phase 7.1 â€” Web UI Foundation
- Phase 7.2 â€” Graph Explorer UI
- Phase 7.2.1 â€” Real Graph API Wiring
- Phase 7.3 â€” Query Trace UI
- Phase 8 â€” File Watcher + Daemon Mode
- Phase 8.1 â€” Parser Isolation
- Phase 8.2 â€” Benchmark Harness + Performance Baseline
- Phase 8.3 â€” Refactor Checkpoint A
- Phase 8.4 â€” Performance Optimization Pass A
- Phase 8.5 â€” Command Output Compaction
- Phase 8.5.1 - Project Init + Manual Index Command
- Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup
- Phase 8.6 - MCP Tool Profiles + Manifest Slimming
- Phase 8.7 - Agent Install Helper + Hook Integration Foundation
- Phase 8.8 - Multi-repo Registry + Project Groups
- Phase 9.0 - Language Backend Architecture
- Phase 8.5.1 â€” Project Init + Manual Index Command
- Phase 8.5.1.1 â€” Repository Structure Audit + Folder/File Cleanup + Web UI Port 8888
- Phase 8.6 â€” MCP Tool Profiles + Manifest Slimming
- Phase 8.7 â€” Agent Install Helper + Hook Integration Foundation
- Phase 8.8 â€” Multi-repo Registry + Project Groups
- Phase 9.0 â€” Language Backend Architecture
- Phase 9.1 â€” LSP Backend MVP
- Phase 9.2 â€” Web Application Priority Support A
- Phase 9.2.1 â€” Node.js / REST API Intelligence
- Phase 9.2.2 â€” React / TSX Component Intelligence

---

## Current Capabilities

B3 has 11 current MCP tools. The default `optimized` MCP profile exposes 7
tools in `tools/list`; `full` and `debug` expose all 11.
### MCP Runtime

B3 exposes 11 MCP tools in the `full` and `debug` profiles:

- `find_symbol`
- `search_code`
- `find_callers`
- `find_callees`
- `related_symbols`
- `impact_analysis`
- `get_context_pack`
- `trace_dependency`
- `detect_cycles`
- `savings_report`
- `compact_command_output`

Default profile:

```text
optimized
```

The `optimized` profile exposes 7 high-value tools by default to reduce manifest noise and token overhead:

- `find_symbol`
- `search_code`
- `related_symbols`
- `impact_analysis`
- `get_context_pack`
- `compact_command_output`
- `savings_report`

Example:

```text
Business Application
|-- Backend API
|-- Frontend App
|-- Worker Service
|-- Desktop Client
`-- Runtime Infrastructure
```

Default storage model:

```text
1 project = 1 repo-local .b3/b3.db
```

Future registry:

```text
~/.b3/registry.json
```

The global registry tracks projects and groups, but each project keeps its own local DB by default.

---

## Roadmap

## Phase 8.5.1 â€” Project Init + Manual Index Command

### Purpose

Make B3 usable from the UI and CLI by exposing a clear project init/index workflow.

B3 already has indexing capability internally, but users need an obvious way to:

```text
init project -> index project -> open UI -> see files/symbols/edges
```

### Scope

Add local-only project init/index commands and APIs.

CLI commands:

- `b3-control-server init --project <path> --database <path>`
- `b3-control-server index --project <path> --database <path>`
- `b3-control-server reindex --project <path> --database <path>`

If a dedicated CLI binary exists later, these can become:

- `b3 init`
- `b3 index`
- `b3 reindex`

Control API:

- `POST /api/index/run`
- `POST /api/index/reindex`
- `GET /api/index/status`

Web UI:

- Add `Run Index` button.
- Add `Reindex Project` button if safe.
- Show indexing status.
- Show indexed files/symbols/edges after indexing.
- Show indexing errors and parse failure summary.

Events:

- `indexing_started`
- `file_indexed`
- `file_skipped`
- `indexing_completed`
- `indexing_failed`
- `parse_failed`

Implementation status:

- Completed with `b3-control-server init`, `index`, and `reindex`.
- `reindex` is safe incremental reindexing in this phase; unchanged files are
  skipped by content hash and deleted files are cleaned for the current branch.
- Control API exposes `POST /api/index/run`, `POST /api/index/reindex`, and
  `GET /api/index/status`.
- Web UI Project Status exposes `Run Index`, `Reindex Project`, last status,
  counts, parse failures, and errors.
- Single-project mode only; multi-repo registry remains deferred to Phase 8.8.

### Rules

- Local-only.
- No cloud.
- No telemetry.
- No external API.
- No language packs.
- No LSP.
- No embeddings.
- No multi-repo registry yet.
- Use existing indexer/storage/query boundaries.
- Do not move indexing logic into control server.
- Control server only triggers indexer behavior.

### Verification

Run:

```bash
cargo fmt
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo run -p b3-bench -- baseline
```

Manual smoke:

- init B3 repo
- run index
- open UI
- verify files/symbols/edges are non-zero for Rust repo
- query a Rust symbol

---

## Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup

### Purpose

Clean up the repository layout after the project init/index workflow and make
the active documentation easier for humans and agents to navigate.

### Scope

- Keep active project docs at the repository root.
- Move reference and historical docs under `docs/reference/` and `docs/archive/`.
- Review `.gitignore` for local/generated artifacts.
- Update the Web UI default development port to `8888`.
- Keep the control server default on `7777`.

### Implementation Status

- Completed.
- Historical and reference docs are preserved under `docs/`.
- Web UI `npm run dev` defaults to `http://127.0.0.1:8888`.
- Root active docs remain the source of truth.
- No runtime feature behavior changed beyond the Web UI development port.

---

## Phase 8.6 â€” MCP Tool Profiles + Manifest Slimming

### Purpose

Reduce MCP tool manifest/token overhead and allow B3 to expose different tool sets depending on workflow.

Inspired by:

- Token Savior tool profiles
- Serena mode/profile system

### Profiles
Available MCP profiles:

- `tiny`
- `optimized`
- `full`
- `debug`
- `readonly`
- `editing`
- `web-app`
- `enterprise`

Hidden tools return a structured `tool_not_enabled` error.

### Control Server

Control server:

```text
http://127.0.0.1:7777
```

Current local APIs include project status, health, diagnostics, graph/query APIs, index APIs, capabilities, language metadata, and LSP metadata.

Registry control APIs are deferred. Registry management is currently CLI-only.

### Web UI

Web UI:

```text
http://127.0.0.1:8888
```

Current UI remains a local single-project developer console. A larger UI refresh is deferred until after more route, component, messaging, infrastructure, semantic search, and project-group data is available.

### CLI

The `b3` CLI currently supports:

```bash
b3-mcp-runtime serve --profile optimized
```

or:

```bash
b3-mcp-runtime serve --tool-profile optimized
```

### Rules

- Existing tools stay compatible.
- Hidden tools return a structured profile-aware error.
- `tools/list` returns only enabled tools and includes the selected profile in
  the result metadata.
- Manifest descriptions and input schemas are concise but still preserve
  required validation details.
- Nested scope compatibility is preserved:
  - `scope.project_id`
  - `scope.branch_id`

### Implementation Status

- Completed in `b3-mcp-runtime`.
- `optimized` is the default profile.
- CLI supports `--profile` and `--tool-profile`.
- Invalid profile names fail with a clear structured CLI error string.
- `tools/list` counts: `optimized` 7, `tiny` 5, `full` 11, `debug` 11,
  `enterprise` 9.
- `readonly` exposes all current readonly tools; future mutation tools must be
  hidden there.
- `editing` is reserved for future symbolic editing and currently uses the
  lower-risk optimized set.
- Benchmarks record selected profile and tool count metadata.
- No installer, hooks, registry, project group runtime, language packs, LSP,
  embeddings, session memory, symbolic editing, command execution, telemetry,
  or cloud dependency were added.

---

## Phase 8.7 â€” Agent Install Helper + Hook Integration

### Purpose

Make setup easier for local agents.

Inspired by:

- GitNexus setup UX
- Token Savior init
- Context Mode hooks

### Commands

- `b3 install --agent cursor`
- `b3 install --agent codex`
- `b3 install --dry-run`
- `b3 install --backup`
- `b3 install`
- `b3 doctor`
- `b3 uninstall`
- `b3 register`
- `b3 unregister`
- `b3 list`
- `b3 status`
- `b3 group create`
- `b3 group add`
- `b3 group remove`
- `b3 group list`
- `b3 group status`

### Implementation Status

- Completed as a small `b3-cli` crate with binary name `b3`.
- Supports `codex` and `cursor`.
- `claude` remains docs/future only.
- Install defaults to dry-run. Writing requires `--apply` or `--write`.
- Applies create backups by default unless `--no-backup` is passed.
- Codex config target defaults to `%USERPROFILE%\.codex\config.toml`.
- Cursor config target defaults to `<project>/.cursor/mcp.json`.
- `--config` can override the target config path for safe manual tests.
- Existing unrelated Codex TOML text and Cursor `mcpServers` entries are
  preserved.
- Repeated install updates the same server entry and does not duplicate it.
- Invalid Cursor JSON and malformed Codex section headers are not overwritten.
- `doctor` performs local path/profile/port checks only.
- `uninstall` removes only the named B3 server entry, preserves unrelated
  config, supports dry-run, and backs up before apply.
- Hook integration is foundation only: docs/output state `hooks_enabled=false`;
  no shell interception, command execution, telemetry, or shell profile edits
  are implemented.

### Rules
CLI commands are local-only and dry-run/apply oriented where destructive writes are possible.

### Command Output Compaction

`compact_command_output` summarizes provided stdout/stderr only. It does not execute commands.

Supported families include:

- git
- cargo
- dotnet
- npm
- pnpm
- yarn
- ng
- tsc
- eslint
- docker
- docker compose
- rg
- grep
- cat
- tree
- unknown/generic

---

## Current Language Support

Support levels are intentionally honest.

### Rust â€” Good

Backend:

- `tree-sitter-rust`

Capabilities:

- file detection
- parsing
- symbol extraction
- import extraction
- basic relationship extraction

### JavaScript â€” Basic

Backend:

- `tree-sitter-javascript`

Capabilities:

- `.js`, `.mjs`, `.cjs` detection
- parsing
- functions
- arrow/function variables
- classes
- methods
- imports / exports
- CommonJS `require`
- conservative `CONTAINS` and `IMPORTS` relationships

### TypeScript â€” Basic

Backend:

- `tree-sitter-typescript`

Capabilities:

- `.ts`, `.mts`, `.cts` detection
- parsing
- functions
- arrow/function variables
- classes
- methods
- interfaces
- type aliases
- enums
- imports / exports
- conservative `CONTAINS` and `IMPORTS` relationships

### JSX / TSX â€” Basic

Backend:

- JavaScript/TypeScript tree-sitter grammars

Capabilities:

- `.jsx`, `.tsx` detection
- parsing
- component-like declarations where structurally obvious
- imports / exports
- conservative relationships

### C# â€” Detect-only

B3 can detect `.cs` files but does not yet provide full parser or semantic support.

### Other Planned Languages

Other planned languages are detect-only or unsupported unless explicitly implemented.

---

## Language Backend Status

B3 has a language backend architecture.

The model separates:

- language detection
- backend kind
- backend capabilities
- support level
- selection policy
- unsupported fallback

Backend kinds:

- Tree-sitter
- LSP
- Static config
- Unknown

Support levels:

- Unsupported
- Basic
- Good
- Advanced

Tree-sitter is used for fast local indexing. LSP is used for future semantic operations.

---

## LSP Backend Status

B3 has an LSP Backend MVP.

Current behavior:

- local-only
- disabled by default
- no language server installation
- no downloads
- no cloud
- no paid backend requirement
- missing language servers are non-fatal

Implemented foundation:

- local stdio process manager
- bounded stderr capture
- startup and request timeouts
- JSON-RPC/LSP framing
- `initialize`
- `initialized`
- `shutdown`
- `exit`
- `didOpen`
- `didChange`
- `definition`
- `references`
- `implementation`
- diagnostics parsing
- `/api/lsp/status`
- `/api/lsp/servers`

LSP complements tree-sitter indexing. It does not replace the local SQLite code graph.

---

## Project Model

B3 supports both standalone projects and project groups.

### Standalone Project

A standalone project is an independent repository with its own local `.b3` database.

Examples:

```text
D:\Project\b3_mcp\.b3\b3.db
D:\Project\BackendApi\.b3\b3.db
D:\Project\FrontendApp\.b3\b3.db
```

Default storage model:

```text
1 project = 1 repo-local .b3/b3.db
```

### Registry

B3 has a local JSON registry:

```text
~/.b3/registry.json
```

Implementation status:

- Completed as local JSON registry support in `b3-cli`.
- `B3_HOME` can override the registry home for tests/smoke runs.
- `--registry <path>` can target an explicit registry file.
- Registry schema version is `1`.
- Registry is metadata only; each project keeps its repo-local `.b3/b3.db`.
- Registry file is created only by explicit registry write commands.
- Invalid registry JSON fails clearly.
- Writes use deterministic pretty JSON and a temp-file rename.
- Destructive updates create backups by default where applicable.
- No filesystem scan, cloud sync, telemetry, hosted DB, or global SQLite DB is
  used.

### Project Commands

- `b3 register <project-path>`
- `b3 unregister <project-id>`
- `b3 list`
- `b3 status <project-id>`
The registry stores metadata only:

- projects
- groups
- tags
- paths
- database paths
- timestamps
- notes

The registry does not merge project databases. Each project keeps its own repo-local DB by default.

### Project Group

Deferred:

- `b3 open`
- `b3 clean`
- `b3 group delete`
- control server registry write APIs
- web UI registry visibility
- installer `--project-id` lookup

### Registry Example
A project group is a metadata grouping of related projects.

Example:

```text
Business Application
â”œâ”€â”€ Backend API
â”œâ”€â”€ Frontend App
â”œâ”€â”€ Worker Service
â”œâ”€â”€ Desktop Client
â””â”€â”€ Runtime Infrastructure
```

Project groups are currently metadata-only.

Deferred:

- project list
- group list
- project switcher
- group switcher
- project status table
- group overview
- run index per project
- open project
- remove from registry

### API

Add:

- `GET /api/projects`
- `POST /api/projects/register`
- `DELETE /api/projects/:project_id`
- `GET /api/projects/:project_id/status`
- `POST /api/projects/:project_id/index`
- `POST /api/projects/:project_id/reindex`
- `GET /api/groups`
- `POST /api/groups`
- `POST /api/groups/:group_id/projects/:project_id`
- `DELETE /api/groups/:group_id/projects/:project_id`

### Rules

- Still local-only.
- Each project keeps its own DB by default.
- Registry is metadata only.
- Cross-project intelligence is not required in Phase 8.8.
- Group-level deep analysis is deferred to Phase 11.
- Existing single-project commands do not require the registry.
- Groups are metadata only and do not merge graphs or execute cross-project
  queries.
- control server registry APIs
- Web UI registry view
- installer `--project-id`
- cross-project graph merging
- cross-project architecture intelligence

---

## Technology Intelligence Model

B3 separates language support from technology intelligence.

Language support teaches B3 how to parse code syntax.

Technology intelligence teaches B3 how to understand application architecture, including:

- routes
- controllers
- services
- components
- modules
- middleware
- dependency injection
- message handlers
- queues
- topics
- exchanges
- realtime events
- cloud resources
- infrastructure manifests
- deployment relationships

Planned technology intelligence includes the following groups.

### Web Backend

### Implementation Status

- Completed shared contracts in `b3-core`.
- Added local language detection by extension and selected filenames.
- Added backend capability registry with support levels.
- Rust is represented as available `tree-sitter-rust` with `Good` support.
- Planned languages report detect-file capability only and are not available as
  parser/LSP backends yet.
- Added `LanguageBackendConfig` defaults:
  - `selection_policy = PreferTreeSitter`
  - `enable_lsp = false`
  - `enable_experimental_languages = false`
- Added control capability reporting through `/api/capabilities` and
  `/api/languages`.
- Existing Rust indexing behavior remains unchanged.
- No LSP runtime, non-Rust parser, embeddings, semantic search, symbolic
  editing, or domain intelligence was added.

### Tree-sitter Responsibilities
- Node.js
- Express
- NestJS
- Fastify
- ASP.NET Core
- Go web services

### Web Frontend

- React
- Angular
- Vue
- Svelte

### Realtime

- WebSocket
- Socket.IO
- SignalR
- RSocket

### Messaging / Event-driven

- AMQP
- RabbitMQ
- Kafka
- Google Pub/Sub
- generic Pub/Sub messaging
- ksqlDB

### Cloud / Infrastructure

- GCP
- GKE
- Kubernetes
- Terraform
- Docker
- Docker Compose

### Data / ORM

- Entity Framework Core
- Dapper
- Prisma
- TypeORM
- Sequelize

### Desktop / Graphics

- WPF
- Avalonia if useful
- Electron if useful
- Three.js
- WebGL

---

## Roadmap

## Phase 9.2.1 â€” Node.js / REST API Intelligence

Status: Completed with basic static/local REST route intelligence.

### Purpose

Add first REST API intelligence for Node.js web applications.

### Scope

Support:

- local package.json technology detection helper for Express, NestJS, Fastify,
  TypeScript, and detect-only frontend packages
- Express route extraction for direct `app.*`, `router.*`, chained
  `router.route(...).get(...)`, and `app.use(...)` patterns
- NestJS controller/method decorator route extraction with class/method path
  composition
- Fastify shorthand and basic route-object extraction
- route metadata encoded on local `Route` symbols
- route-to-handler `REFERENCES` edges where a handler/method symbol is
  resolvable by the current static extractor
- read-only `GET /api/routes` with project, branch, framework, method, path,
  and limit filters
- route cleanup through normal deleted-file index cleanup

### Express Targets

Detect common patterns:

- `app.get("/users", handler)`
- `app.post("/users", handler)`
- `app.put("/users/:id", handler)`
- `app.patch("/users/:id", handler)`
- `app.delete("/users/:id", handler)`
- `app.use("/users", router)`
- `router.get("/", handler)`
- `router.post("/", handler)`
- `router.route("/users").get(handler).post(handler)`

### NestJS Targets

Detect common patterns:

- `@Controller("users")`
- `@Get()`
- `@Get(":id")`
- `@Post()`
- `@Put()`
- `@Patch()`
- `@Delete()`
- class route prefix + method route composition

### Fastify Targets

Detect if low-risk:

- `fastify.get("/users", handler)`
- `fastify.post("/users", handler)`
- `fastify.route({ method, url, handler })`

### Out of Scope

- React component graph
- Angular intelligence
- ASP.NET Core
- Go
- WebSocket/RSocket/SignalR
- Kafka/RabbitMQ/AMQP/PubSub
- GCP/GKE/Terraform
- symbolic editing
- embeddings
- cross-project intelligence
- deep middleware execution order
- runtime request tracing or dynamic runtime route generation
- Nest module graph, guards, interceptors, pipes, and deep dependency injection
- request lifecycle inference

---

## Phase 9.2.2 â€” React / TSX Component Intelligence

Status: Completed with basic static/local React component intelligence.

### Purpose

Understand basic React component structure.

### Scope

- function components
- arrow function components
- class components
- default and named exports
- props interfaces/types
- local package/import detection for React
- JSX component usages and parent-child component references where safe
- basic hook detection
- component metadata encoded on component symbols
- read-only `GET /api/components` with project, branch, framework, name, file,
  and limit filters

### Out of Scope

- full runtime rendering behavior
- state machine inference
- deep hook semantics or dependency-array analysis
- full JSX tree graph
- CSS/layout intelligence
- Next.js routing and app/page route intelligence, handled in Phase 9.2.3
- automatic editing
- Angular, C#, realtime, messaging, cloud/infrastructure, Go, embeddings,
  semantic search, and cross-project intelligence

---

## Phase 9.2.3 â€” Next.js Intelligence

### Purpose

Add basic static intelligence for Next.js applications on top of React / TSX
support.

### Scope

- detect Next.js from `package.json`
- detect `next.config.js`, `next.config.mjs`, and `next.config.ts`
- detect App Router structure under `app/`
- detect Pages Router structure under `pages/`
- map page, layout, loading, error, and not-found files to routes where safe
- detect dynamic routes like `[id]`, `[...slug]`, and `[[...slug]]`
- detect `app/api/**/route.ts` and `route.js` API handlers
- detect route handler methods: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`,
  `OPTIONS`, and `HEAD`
- detect `"use client"` boundaries
- basic static server/client component classification
- preserve React component intelligence from Phase 9.2.2

### Out of Scope

- running `next dev`
- running `next build`
- runtime rendering
- full React Server Components semantics
- middleware execution order
- Vercel/deployment intelligence
- NextAuth/auth intelligence
- deep data fetching semantics
- symbolic editing

---

## Phase 9.2.4 â€” Angular Intelligence

### Purpose

Understand Angular application structure.

### Scope

- components
- services
- modules
- decorators
- route config
- templates where safe
- dependency injection basics

### Out of Scope

- full template type checking
- Angular compiler integration
- runtime behavior
- advanced RxJS flow inference

---

## Phase 9.2.5 â€” ASP.NET Core / C# Web API Intelligence

### Purpose

Add first real C# web API intelligence.

### Scope

- C# parser or LSP-backed support
- controllers
- actions
- route attributes
- dependency injection basics
- service relationships
- request/response DTOs where safe

### Out of Scope

- full Roslyn replacement
- deep EF query analysis
- symbolic editing
- rename/refactor

---

## Phase 9.2.6 â€” ORM / Database Access Intelligence

### Purpose

Understand common data access patterns.

### Scope

- Entity Framework Core
- Dapper
- Prisma
- TypeORM
- Sequelize
- repository/service relationships
- query callsites
- data model references where safe

### Out of Scope

- full SQL optimizer
- DB connection execution
- schema migration execution
- runtime query tracing

---

## Phase 9.2.7 â€” Realtime / Socket Intelligence

### Purpose

Understand realtime communication flows.

### Scope

- WebSocket
- Socket.IO
- SignalR
- RSocket
- event handlers
- event/channel names
- hub methods
- client/server flow metadata
- route or service links where safe

### Out of Scope

- live socket tracing
- runtime packet capture
- command execution
- telemetry

---

## Phase 9.2.8 â€” Messaging / Event-driven Intelligence

### Purpose

Understand asynchronous messaging flows.

### Scope

- AMQP
- RabbitMQ
- Kafka
- Google Pub/Sub
- generic Pub/Sub messaging
- ksqlDB
- producers
- consumers
- topics
- queues
- exchanges
- routing keys
- consumer groups
- streams/tables
- event contract impact

### Out of Scope

- broker connection
- message consumption
- cloud service calls
- telemetry

---

## Phase 9.2.9 â€” Cloud / Infrastructure Intelligence

### Purpose

Understand infrastructure and deployment metadata.

### Scope

- Docker
- Docker Compose
- Kubernetes manifests
- GCP resources where statically detectable
- GKE workload/config metadata
- Terraform resources/modules/variables/outputs
- service-to-infra relationships where safe

### Out of Scope

- cloud API calls
- Terraform plan/apply
- kubectl execution
- gcloud execution
- live cluster inspection

---

## Phase 9.2.10 â€” Go Language Support

### Purpose

Add basic Go indexing support.

### Scope

- `.go` detection
- packages
- imports
- functions
- structs
- interfaces
- methods
- basic relationships where safe

### Future Go Framework Support

- `net/http`
- Gin
- Echo
- Fiber
- gRPC

---

## Phase 9.3 â€” Symbolic Editing MVP

### Purpose

Add safe local code-editing tools based on indexed symbols.

### Tools

- `replace_symbol_body`
- `insert_before_symbol`
- `insert_after_symbol`
- `preview_edit`
- `apply_edit`
- `reindex_after_edit`

### Rules

- dry-run first
- show affected files
- require explicit apply
- reindex changed files
- no blind text replacement
- no full-file rewrite unless needed
- readonly profile must hide mutation tools
- Do not install or download language servers.
- Missing language servers are unavailable, not fatal.
- Rust tree-sitter indexing remains the best supported path.

---

## Phase 9.4 â€” Rename / Refactor MVP

### Purpose

Add safe rename/refactor support using LSP where available.

### Tools

- `rename_symbol`
- `preview_rename`
- `find_declaration`
- `find_implementations`
- `find_references_lsp`

### Rules

- use LSP when available
- preview affected edits
- apply atomically where possible
- reindex after apply
- fallback to readonly behavior when LSP capability is missing

---

## Phase 9.5 â€” Additional Backend Language Support

### Purpose

Add Basic or Good support for additional backend languages.

### Targets

- Python
- Java
- PHP
- Ruby

Go is handled earlier in Phase 9.2.10 because it is part of the planned web/backend priority stack.

---

## Phase 9.6 â€” Systems / Mobile Language Support

### Purpose

Add Basic support for systems and mobile-adjacent languages where useful.

### Targets

- C
- C++
- Swift
- Kotlin
- Dart if useful

---

## Phase 9.7 â€” Config / Data / Web File Support

### Purpose

Improve indexing for configuration, data, and web file types.

### Targets

- SQL
- YAML
- JSON
- HTML
- CSS / SCSS
- TOML
- XML
- XAML if not already handled by WPF intelligence

### Support Level

- file detection
- top-level symbols/keys
- imports/includes where applicable
- FTS
- config key extraction

---

## Phase 9.8 â€” Language and Technology Quality Audit

### Purpose

Measure extraction correctness and risk across implemented languages and technologies.

### Measure

- symbol extraction quality
- relationship extraction quality
- route extraction quality
- component extraction quality
- messaging extraction quality
- infrastructure extraction quality
- false positives
- false negatives
- parser crash/error rate
- LSP timeout/failure rate

---

## Phase 9.9 â€” Refactor Checkpoint B

### Purpose

Clean language and technology backend abstractions after real-world usage.

Rules:

- no speculative rewrite
- preserve DTO compatibility where possible
- keep MCP runtime thin
- keep parser/indexing logic in indexer

---

## Phase 9.10 â€” Performance Optimization Pass B

### Purpose

Optimize measured multi-language and technology intelligence bottlenecks.

Rules:

- benchmark first
- optimize measured bottlenecks only
- no telemetry
- no unrelated refactor

---

## Phase 10 â€” Local Embeddings + Vector Search

### Purpose

Add semantic retrieval as a secondary signal.

### Allowed Local/Free Providers

- local embedding model abstraction
- fastembed if feasible
- Candle if feasible
- Ollama if used as an optional local provider
- local vector index/storage
- local Qdrant only as an optional local component

### Rules

- no cloud embeddings required
- no OpenAI / Anthropic / Gemini required
- no hosted vector database required
- no internet required at runtime
- cloud providers are optional plugins only, disabled by default
- semantic signal remains secondary to AST/graph/FTS

---

## Phase 10.1 â€” Semantic Context Upgrade

### Purpose

Use local semantic signals to improve context quality.

### Scope

- semantic search
- hybrid search: FTS + graph + vector
- graph-aware reranking
- semantic fallback
- duplicate deduplication
- embedding freshness tracking
- benchmark context quality impact

---

## Phase 10.2 â€” Session Memory + Context Virtualization

### Purpose

Add local session continuity inspired by Context Mode and Token Savior.

### Scope

- `SessionEvent`
- `SessionSnapshot`
- `DecisionMemory`
- `TaskMemory`
- `ErrorFixMemory`
- `ResumeContext`
- compact/resume support
- local SQLite memory
- no telemetry

---

## Phase 10.3 â€” Transcript Discovery / Token Opportunity Report

### Purpose

Scan local agent transcripts/logs to find missed opportunities for B3 tools.

### Scope

- detect repeated grep/read chains
- detect missed context-pack opportunities
- detect long command output opportunities
- produce local adoption report
- suggest better B3 tool usage

Rules:

- local logs only
- no uploads
- no telemetry

---

## Phase 10.4 â€” Web UI Developer Console Refresh

### Purpose

Upgrade the B3 Web UI into a polished local developer console after core language, route, framework, messaging, infrastructure, semantic search, and project-group capabilities are mature enough to display useful data.

### Planned UI Stack

- Tailwind CSS
- shadcn/ui-style local components
- Radix UI primitives where useful
- lucide-react icons

### Scope

- app shell
- sidebar navigation
- dashboard cards
- project status
- indexing status
- language support page
- technology intelligence page
- route view
- component view
- service view
- registry/project group visibility
- diagnostics/events improvements
- better empty/loading/error states

### Rules

- no paid UI kit
- no telemetry
- no cloud dependency
- no runtime internet requirement
- no fake data
- no backend intelligence hidden in UI

---

## Phase 11 â€” Architecture Intelligence

### Purpose

Add architecture-level understanding, including group-level and cross-project analysis.

### Scope

- community detection
- module boundary detection
- dependency cluster map
- circular dependency reports
- architecture layer detection
- service map
- multi-service flow analysis
- project group overview
- cross-project dependency flow
- frontend -> backend -> worker flow
- API -> message broker -> consumer flow
- app service -> infrastructure flow where safe

### Algorithms

- connected components
- strongly connected components
- PageRank / centrality
- label propagation or Louvain if feasible
- dependency clustering

---

## Phase 12 â€” Git Intelligence

### Purpose

Add repository history signals as local ranking/risk inputs.

### Scope

- recent churn score
- last modified commit
- commit frequency
- hotspot files
- author count
- ranking/risk integration

Rules:

- local git data only
- no remote calls required
- no telemetry

---

## Phase 13 â€” Duplicate / Similarity Detection

### Purpose

Detect duplicate and similar code locally.

### Scope

- AST fingerprinting
- normalized AST hash
- MinHash
- SimHash
- duplicate function detection
- similar code search

---

## Phase 14 â€” Real Plugin System

### Purpose

Add a real plugin model while keeping the default core offline/free.

### Scope

- plugin registry runtime
- plugin lifecycle
- capability discovery
- language plugin loading
- ranking plugin loading
- embedding plugin loading
- compactor plugin loading

Rules:

- external providers optional only
- disabled by default
- not required for install/tests/benchmarks/core features

---

## Phase 15 â€” Packaging + Installers

### Purpose

Prepare B3 for easier distribution.

### Scope

- release packaging
- binary install workflow
- optional Tauri app if still useful
- versioned artifacts
- platform notes
- install/doctor/uninstall polish

Note:

Basic local agent install helpers already exist from Phase 8.7. This phase is about release-grade packaging and distribution polish.

---

## When Can We Use It?

| Use case | Status |
|---|---|
| Test MCP runtime with Codex/Cursor | Usable now |
| Rust repositories | Usable now |
| JavaScript / TypeScript / JSX / TSX basic indexing | Usable now |
| Command output compaction | Usable now |
| Project init/index workflow | Usable now |
| MCP tool profiles | Usable now |
| Codex/Cursor install helper | Usable now |
| Multi-project local workflow | Usable now, metadata only |
| Project groups | Usable now, metadata only |
| Language backend contracts | Usable now |
| C# Web API / backend services | Phase 9.2.5 |
| Basic JavaScript / TypeScript / JSX / TSX indexing | Usable now |
| Basic React / TSX component intelligence | Usable now, basic/static |
| Next.js intelligence | Phase 9.2.3 |
| Angular deep graph intelligence | Phase 9.2.4 |
| Node.js REST API | Usable now, basic/static |
| Kafka / ksqlDB | Phase 9.2.8 |
| RabbitMQ | Phase 9.2.8 |
| Docker / docker-compose | Phase 9.2.9 |
| SignalR | Phase 9.2.7 |
| C# WPF | Deferred |
| Three.js / WebGL | Deferred |
| Agent install helper for Codex/Cursor | Usable now |
| Multi-project CLI registry | Usable now |
| Project groups metadata | Usable now |
| Registry Web UI | Deferred |
| Control registry APIs | Deferred |
| C# Web API / ASP.NET Core | Phase 9.2.5 |
| Node.js REST API / Express / NestJS / Fastify | Usable now, basic/static |
| React / TSX component intelligence | Usable now, basic/static |
| Next.js intelligence | Phase 9.2.3 |
| Angular intelligence | Phase 9.2.4 |
| ORM / database access intelligence | Phase 9.2.6 |
| WebSocket / Socket.IO / SignalR / RSocket | Phase 9.2.7 |
| AMQP / RabbitMQ / Kafka / Google Pub/Sub | Phase 9.2.8 |
| Docker / Kubernetes / GCP / GKE / Terraform | Phase 9.2.9 |
| Go language support | Phase 9.2.10 |
| Refactor assistant | Phase 9.3 / 9.4 |
| Local embeddings / vector search | Phase 10 |
| Full memory/context platform | Phase 10.2+ |
| Architecture intelligence | Phase 11 |
| Release-grade packaging | Phase 15 |

B3 can run today as a local MCP/runtime/control/UI platform with Rust, basic JS/TS/JSX/TSX indexing, basic static Node.js REST route intelligence, and basic static React/TSX component intelligence. Broader real-world app-stack intelligence depends on Phase 9.2.3 and later.

---

## Refactor Rules

- Refactor only after verified feature milestones.
- Prefer small targeted refactors.
- Do not rewrite architecture unnecessarily.
- Do not genericize too early.
- Keep MCP runtime thin.
- Preserve offline-first architecture.
- Preserve public DTO compatibility where possible.
- Preserve nested scope compatibility where possible:
  - `scope.project_id`
  - `scope.branch_id`
- Do not move persistence internals out of storage.
- Do not move graph traversal/ranking into MCP runtime or control server.
- Do not move parser/indexing logic into MCP runtime or control server.

---

## Optimization Rules

- Benchmark first.
- Optimize measured bottlenecks only.
- Preserve regression benchmarks.
- Avoid speculative optimization.
- No performance work without before/after measurement.
- Benchmark output must remain local.
- No benchmark telemetry.
- Do not upload benchmark data.

---

## Benchmark Strategy

Benchmarks should establish a baseline before optimization work starts.

Track:

- cold startup time
- MCP initialize latency
- MCP tools/list latency
- MCP tools/call latency
- common query latencies
- graph traversal latency
- context pack latency
- impact analysis latency
- route extraction latency when implemented
- technology detection latency when implemented
- indexing speed
- changed-file reindex latency
- watcher debounce overhead
- SQLite query latency
- parser worker latency
- LSP request latency when enabled and tested locally
- command compaction latency
- approximate memory use when feasible

Benchmark fixtures must remain local, deterministic, small enough to commit, and free from network/cloud/API calls.

---

## Multi-language Strategy

Target: around 20 practical languages/file types.

Do not implement all languages in one phase.

Use support levels.

### Basic

- file detection
- top-level symbols
- imports/includes where applicable
- FTS support

### Good

- methods
- classes
- tests
- basic calls
- references
- route hints
- framework/library hints where safe

### Advanced

- framework routes
- DI relationships
- cross-file references
- inheritance/interface edges
- component relationships
- message flows
- infrastructure relationships
- LSP references/definitions
- safe editing support

### Priority

First priority is based on common real-world application stacks:

- Rust
- JavaScript
- TypeScript
- JSX / TSX
- C#
- React
- Angular
- Node.js
- Express
- NestJS
- Fastify

Then:

- ASP.NET Core
- Entity Framework Core
- Dapper
- Prisma
- TypeORM
- Sequelize
- WebSocket
- Socket.IO
- SignalR
- RSocket
- AMQP
- RabbitMQ
- Kafka
- Google Pub/Sub
- generic Pub/Sub messaging
- ksqlDB
- Docker
- Kubernetes
- GCP
- GKE
- Terraform
- Go
- WPF
- Three.js / WebGL

Then:

- Python
- Java
- PHP
- Ruby
- C
- C++
- Swift
- Kotlin
- SQL
- YAML
- JSON
- HTML
- CSS / SCSS
- TOML
- XML

---

## Documentation Sync Rule

After every phase, update relevant markdown files before marking the phase complete.

Review:

- `README.md`
- `PLAN.md`
- `REQUIREMENTS.md`
- `ALGORITHM_ANALYSIS.md`
- `DEVELOPMENT.md`
- `MCP_TOOLS.md`
- `CONTROL_SERVER.md`
- `WEB_UI.md`
- `AGENTS.md`
- `.skills/*/SKILL.md`

Only update affected files.

Every phase completion report must include:

A. files changed  
B. behavior implemented  
C. boundaries preserved  
D. docs updated  
E. verification results  
F. offline/free compliance result  
G. remaining risks  
H. deferred work  
I. READY / NOT READY for next phase

For larger feature phases, include phase-specific sections such as API behavior, storage behavior, benchmark status, MCP/profile compatibility, and UI impact.

---

## Reference Models and Borrowed Ideas

These projects are inspirations only, not dependencies.

B3 must remain Rust-native where appropriate, local-first, offline-first, and free-by-default.

### codebase-memory-mcp

B3 learns tree-sitter code graph, persistent local code memory, MCP code intelligence, and graph visualization.

Covered by Phase 3 to Phase 7 and future Phase 9.x for multi-language graph expansion.

B3 intentionally differs through stronger offline/free governance, Rust-native architecture, explicit control server/UI/debugging boundaries, query trace, and benchmark-first roadmap.

### TokenSave

B3 learns token-saving retrieval, indexed context instead of repeated grep/read, context pack strategy, and savings ledger concept.

Covered by Phase 5, Phase 5.1, Phase 5.2, and Phase 8.5 as an adjacent command compaction layer.

### RTK / Rust Token Killer

B3 learns command output compaction, git/test/build/lint output summarization, and local proxy/token-saving ideas.

Covered by Phase 8.5.

B3 intentionally differs because B3 also owns code graph, UI, MCP tools, query trace, and storage, and does not execute commands in the compaction tool.

### Context Mode

B3 learns session continuity, context virtualization, local session event log, and compact/resume workflow.

Covered by future Phase 10.2.

### Token Savior

B3 learns structural MCP navigation, MCP tool profiles, manifest slimming, command output compaction, benchmark-driven development, transcript discovery, and install helper ideas.

Covered by Phase 8.2, Phase 8.5, Phase 8.6, Phase 8.7, and Phase 10.3.

### CodeGraph

B3 learns broad multi-language graph MCP, framework-aware route detection, with/without MCP benchmarks, and agent steering.

Covered by Phase 8.2, Phase 9.0+, Phase 9.2+, and Phase 9.2.1+.

### GitNexus

B3 learns multi-repo registry, setup/install UX, bridge UI/product UX, repo groups, and multi-service analysis.

Covered by Phase 8.7, Phase 8.8, and Phase 11.

### Serena

B3 learns LSP backend, IDE-grade semantic operations, find definition/references/implementations, symbolic editing, rename/refactor, and mode/profile system.

Covered by Phase 8.6, Phase 9.0, Phase 9.1, Phase 9.3, and Phase 9.4.

B3 intentionally differs because there is no required paid JetBrains plugin, LSP must be local/free by default, tree-sitter graph remains core, and LSP complements graph rather than replacing it.

### Neo4j Browser-style UX

B3 learns graph explorer UX, node/edge inspector, and path/cycle visualization.

Covered by Phase 7.2 and Phase 7.2.1.

B3 intentionally avoids a Neo4j dependency.

### Sourcegraph / Cursor-style Systems

B3 learns code intelligence workflow, impact analysis, context retrieval, and developer-oriented code navigation.

Covered by Phase 5+, Phase 6+, and Phase 7+.

B3 intentionally stays local-first, offline-first, free-by-default, MCP-compatible, and SQLite-backed.

---

## Notes

- Do not implement all domains at once.
- Each domain must have tests and benchmark fixtures.
- Framework/library detection should be conservative and explainable.
- Language support, framework detection, and framework intelligence are separate layers.
- No cloud APIs are required.
- Offline-first and free-by-default remain hard requirements.
- External/cloud/paid integrations are optional plugins only and disabled by default.
