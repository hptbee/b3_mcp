# Project Plan

B3 is a local-first, offline-first, free-by-default AI-native code intelligence platform for coding agents and local developer workflows.

This document is the source of truth for the detailed roadmap. README.md should remain concise and traditional.

Current roadmap status:

- Completed: Phase 8.5 - Command Output Compaction
- Completed: Phase 8.5.1 - Project Init + Manual Index Command
- Completed: Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup
- Next: Phase 8.6 - MCP Tool Profiles + Manifest Slimming

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
- localhost control server
- web UI
- graph explorer
- query trace UI
- file watcher
- parser isolation
- benchmark baseline
- command output compaction
- project init/index workflow
- future MCP tool profiles
- future agent install helper
- future multi-repo registry
- future project groups
- future language backend architecture
- future LSP backend
- future symbolic editing
- future messaging/runtime infrastructure intelligence
- future session memory
- future local embeddings

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
- local LSP servers when implemented later
- local embeddings only when implemented later
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

1. **Local-first and offline-first** — B3 must work without internet access.
2. **Free-by-default** — core functionality must be free to run locally.
3. **MCP runtime stays thin** — protocol/tool adapter only.
4. **Storage owns persistence** — SQLite schema, migrations, transactions, repositories, and adapters live in `b3-storage`.
5. **Indexer owns indexing** — parsing, discovery, hashing, incremental indexing, watcher, parser worker, and parse failures live in `b3-indexer`.
6. **Query owns intelligence** — graph traversal, ranking, context packing, impact analysis, dependency tracing, cycle detection, and query trace live in `b3-query`.
7. **Control server is an adapter** — localhost HTTP/SSE only; no storage internals or query intelligence.
8. **Web UI talks only to local control server** — no cloud calls.
9. **Graph first, semantic second** — AST, graph, and FTS are core; embeddings are optional future secondary signals.
10. **Benchmark before optimization** — measure first, optimize measured bottlenecks only.
11. **Refactor only after verified milestones** — prefer small targeted refactors.
12. **Language support must be layered** — do not implement all languages in one phase.
13. **LSP complements tree-sitter** — tree-sitter for fast indexing; local LSP for semantic operations later.
14. **Project model supports standalone and grouped projects** — `1 project = 1 .b3/b3.db` by default.

---

## Current Status

Completed:

```text
Phase 8.5 - Command Output Compaction
Phase 8.5.1 - Project Init + Manual Index Command
Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup
```

Next:

```text
Phase 8.6 - MCP Tool Profiles + Manifest Slimming
```
---

## Completed Phases

- Phase 1 — Workspace / Scaffold
- Phase 1.5 — Contracts / Boundaries
- Phase 2 — SQLite Storage / Schema Foundation
- Phase 3 — Incremental Indexer Skeleton
- Phase 3.1 — Indexer Audit / Cleanup
- Pre-Phase-4 — Plugin Contracts / Docs / CI
- Phase 4 — Real Rust Parsing + Storage Adapter
- Phase 4.1 — Project/Branch Auto Ensure + Deleted File Cleanup
- Phase 5 — Query Engine + Graph Traversal + Context Pack
- Phase 5.1 — Query Hardening + Retrieval Explainability
- Phase 5.2 — Ranking Algorithms Upgrade
- Phase 6 — MCP Tools over Query Engine
- Phase 6.0.1 — Live MCP Runtime Wiring
- Phase 6.1 — Impact Intelligence
- Phase 6.2 — PageRank / Centrality
- Phase 6.3 — MCP Runtime Hardening + Real-world Smoke Test
- Phase 7 — Control Server + Localhost API
- Phase 7.1 — Web UI Foundation
- Phase 7.2 — Graph Explorer UI
- Phase 7.2.1 — Real Graph API Wiring
- Phase 7.3 — Query Trace UI
- Phase 8 — File Watcher + Daemon Mode
- Phase 8.1 — Parser Isolation
- Phase 8.2 — Benchmark Harness + Performance Baseline
- Phase 8.3 — Refactor Checkpoint A
- Phase 8.4 — Performance Optimization Pass A
- Phase 8.5 — Command Output Compaction
- Phase 8.5.1 - Project Init + Manual Index Command
- Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup

---

## Current Tool State

B3 currently exposes 11 MCP tools:

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

---

## Project Model

B3 must support both standalone projects and project groups.

### Standalone Project

A standalone project is an independent repository with its own local `.b3` database.

Examples:

```text
D:\Project\b3_mcp\.b3\b3.db
D:\Project\ThreeDemo\.b3\b3.db
D:\Project\DesktopTool\.b3\b3.db
```

### Project Group

A project group is a set of related projects that form one system.

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

## Phase 8.5.1 — Project Init + Manual Index Command

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

## Phase 8.6 — MCP Tool Profiles + Manifest Slimming

### Purpose

Reduce MCP tool manifest/token overhead and allow B3 to expose different tool sets depending on workflow.

Inspired by:

- Token Savior tool profiles
- Serena mode/profile system

### Profiles

- `tiny`
- `optimized`
- `full`
- `debug`
- `readonly`
- `editing`
- `web-app`
- `enterprise`

### Default

Default profile:

```text
optimized
```

### Profile Behavior

#### tiny

Expose only the smallest high-value set:

- `search_code`
- `find_symbol`
- `get_context_pack`
- `compact_command_output`
- `savings_report`

#### optimized

Default normal agent profile:

- `find_symbol`
- `search_code`
- `related_symbols`
- `impact_analysis`
- `get_context_pack`
- `compact_command_output`
- `savings_report`

#### full

Expose all current tools.

#### debug

Expose all current tools, especially:

- `trace_dependency`
- `detect_cycles`
- `savings_report`

#### readonly

Expose only non-mutating tools.

Currently all tools are readonly. Future mutation tools must be hidden in this profile.

#### editing

Reserved for future symbolic editing tools.

For now, same as optimized or full.

#### web-app

Optimized for common web application workflows.

For now:

- `find_symbol`
- `search_code`
- `related_symbols`
- `impact_analysis`
- `get_context_pack`
- `compact_command_output`
- `savings_report`

Future web-app tools may prioritize C#, TypeScript, JavaScript, React, Angular, Node.js, REST APIs, and route/component workflows.

#### enterprise

Optimized for future graph, impact, messaging, Docker, SignalR, and API route workflows.

For now:

- `find_symbol`
- `search_code`
- `related_symbols`
- `impact_analysis`
- `get_context_pack`
- `trace_dependency`
- `detect_cycles`
- `compact_command_output`
- `savings_report`

### CLI

Add a profile flag:

```bash
b3-mcp-runtime serve --profile optimized
```

or:

```bash
b3-mcp-runtime serve --tool-profile optimized
```

### Rules

- Existing tools stay compatible.
- Hidden tools should return a structured profile-aware error.
- `tools/list` should return only enabled tools.
- Manifest descriptions should be concise.
- Do not remove required validation details.
- Preserve nested scope compatibility:
  - `scope.project_id`
  - `scope.branch_id`

---

## Phase 8.7 — Agent Install Helper + Hook Integration

### Purpose

Make setup easier for local agents.

Inspired by:

- GitNexus setup UX
- Token Savior init
- Context Mode hooks

### Commands

- `b3 install --agent cursor`
- `b3 install --agent codex`
- `b3 install --agent claude`
- `b3 install --dry-run`
- `b3 install --backup`
- `b3 doctor`
- `b3 uninstall`

### Rules

- Idempotent.
- Backup existing configs.
- Show diff before writing when possible.
- No cloud.
- No telemetry.
- No required internet.
- Hooks must be optional.
- Any auto-interception must be disabled by default.

### Future Hook Integration

Optional only:

- suggest MCP tools instead of repeated grep/read
- optional command output compaction
- optional session event capture

---

## Phase 8.8 — Multi-repo Registry + Project Groups

### Purpose

Support many local repos cleanly.

Some projects are standalone.

Some projects belong to a group/system.

Inspired by:

- GitNexus multi-repo registry
- GitNexus project groups
- multi-service developer workflows

### Registry

Add:

```text
~/.b3/registry.json
```

### Project Commands

- `b3 register <project-path>`
- `b3 unregister <project-id>`
- `b3 list`
- `b3 status <project-id>`
- `b3 clean <project-id>`

### Group Commands

- `b3 group create <group-name>`
- `b3 group add <group-id> <project-id>`
- `b3 group remove <group-id> <project-id>`
- `b3 group list`
- `b3 group status <group-id>`

### Registry Example

```json
{
  "version": 1,
  "projects": [
    {
      "id": "backend-api",
      "name": "Backend API",
      "path": "D:\\Project\\BackendApi",
      "database": "D:\\Project\\BackendApi\\.b3\\b3.db",
      "tags": ["api", "backend"]
    },
    {
      "id": "frontend-app",
      "name": "Frontend App",
      "path": "D:\\Project\\FrontendApp",
      "database": "D:\\Project\\FrontendApp\\.b3\\b3.db",
      "tags": ["frontend", "react", "angular"]
    }
  ],
  "groups": [
    {
      "id": "business-app",
      "name": "Business Application",
      "description": "Backend, frontend, workers, desktop clients, and runtime infrastructure.",
      "project_ids": [
        "backend-api",
        "frontend-app",
        "worker-service",
        "desktop-client"
      ]
    }
  ]
}
```

### UI

Add:

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

---

## Phase 9.0 — Language Backend Architecture

### Purpose

Replace the old “Language Pack Architecture” concept with a broader language backend architecture.

Support both:

- tree-sitter
- LSP

### Add

- `LanguageBackend` trait
- `TreeSitterBackend`
- `LspBackend`
- `LanguageRegistry`
- capability discovery
- extension-to-language resolution
- backend selection policy
- unsupported language fallback
- support quality levels:
  - Basic
  - Good
  - Advanced

### Tree-sitter Responsibilities

- fast local indexing
- symbols
- imports
- basic relationships
- FTS
- graph nodes/edges

### LSP Responsibilities

- go to definition
- find references
- find implementations
- diagnostics
- rename support
- type-aware operations
- safe symbolic editing support

### Offline/Free Rules

- LSP backends must use local language servers.
- No paid/proprietary backend required.
- Paid/proprietary integrations are optional plugins only, disabled by default.

---

## Phase 9.1 — LSP Backend MVP

Priority languages:

- C#
- TypeScript
- JavaScript
- Angular where possible

Implement local LSP process management, workspace initialization, document sync, definition lookup, references lookup, implementations lookup, diagnostics, capability detection, timeout/retry/error handling.

Rules:

- Do not implement symbolic editing yet.
- Use local/free LSP servers only.
- No paid JetBrains plugin required.
- No cloud language service.

---

## Phase 9.2 — Web Application Priority Support

Support:

- C#
- TypeScript
- JavaScript
- React / TSX
- Angular

This phase targets common business/web application stacks, not one specific product domain.

Detect common C# classes, interfaces, methods, controllers, services, repositories, DTOs, tests, and ASP.NET routes where possible.

Detect React/Angular components, hooks, services, modules, routes, imports/exports, and tests.

Graph nodes:

- Controller
- Service
- Repository
- DTO
- Component
- Hook
- Module
- Route

Edges:

- `ROUTES_TO`
- `HANDLES_ROUTE`
- `COMPONENT_USES`
- `SERVICE_INJECTS`
- `CALLS_SERVICE`

---

## Phase 9.2.1 — Node.js / REST API Intelligence

Support Node.js, Express, NestJS, Fastify basic, REST handlers, API routes, middleware, controllers, and services.

Graph nodes:

- ApiRoute
- Controller
- Middleware
- Service
- Handler

Edges:

- `ROUTES_TO`
- `HANDLES_ROUTE`
- `USES_MIDDLEWARE`
- `CALLS_SERVICE`

---

## Phase 9.2.2 — Messaging / Event-driven Intelligence

Support:

- Kafka
- ksqlDB
- RabbitMQ

Detect topics, producers, consumers, consumer groups, ksql streams/tables, joins, windows, queues, exchanges, bindings, routing keys, publishers, consumers, and dead-letter queues.

Graph nodes:

- Topic
- Queue
- Exchange
- RoutingKey
- ConsumerGroup
- Stream
- Table
- MessageContract
- Producer
- Consumer

Edges:

- `PRODUCES_TO`
- `CONSUMES_FROM`
- `PUBLISHES_TO`
- `BINDS_TO`
- `ROUTES_TO`
- `JOINS_STREAM`
- `READS_TOPIC`
- `WRITES_TOPIC`
- `USES_CONTRACT`

---

## Phase 9.2.3 — Docker / Runtime Infrastructure Intelligence

Support Dockerfile, docker-compose.yml, `.env`, and deployment configs.

Detect services, images, ports, volumes, env vars, networks, depends_on, healthchecks, and build contexts.

Graph nodes:

- DockerService
- DockerImage
- Port
- Volume
- Network
- EnvironmentVariable
- Container

Edges:

- `DEPENDS_ON`
- `EXPOSES_PORT`
- `USES_VOLUME`
- `USES_NETWORK`
- `BUILDS_FROM`
- `USES_ENV`
- `RUNS_SERVICE`

---

## Phase 9.2.4 — SignalR / Real-time Communication Intelligence

Support ASP.NET SignalR hubs, hub methods, client events, groups, and frontend SignalR client handlers.

Detect Hub classes, Hub methods, `Clients.All`, `Clients.Caller`, `Clients.Group`, `Groups.AddToGroupAsync`, `SendAsync` event names, frontend `.on` handlers, and frontend `.invoke` calls.

Graph nodes:

- SignalRHub
- HubMethod
- ClientEvent
- Group
- RealtimeConnection

Edges:

- `EMITS_EVENT`
- `HANDLES_EVENT`
- `JOINS_GROUP`
- `SENDS_TO_CLIENT`
- `CALLS_HUB`
- `USES_CONNECTION`

---

## Phase 9.2.5 — C# WPF Desktop App Intelligence

Support WPF XAML, Windows, Pages, UserControls, ViewModels, Commands, Bindings, Resources, and Event handlers.

Detect Window, Page, UserControl, ViewModel, `ICommand`, `RelayCommand`, Binding paths, ResourceDictionary, XAML event handlers, and navigation targets.

Graph nodes:

- WpfWindow
- WpfPage
- WpfUserControl
- ViewModel
- Command
- Binding
- Resource
- EventHandler

Edges:

- `BINDS_TO`
- `USES_VIEWMODEL`
- `HANDLES_EVENT`
- `USES_RESOURCE`
- `NAVIGATES_TO`
- `COMMAND_EXECUTES`

---

## Phase 9.2.6 — Three.js / WebGL Graphics Intelligence

Support Three.js scenes, meshes, cameras, lights, render loops, shaders, and React integration where applicable.

Detect canvas elements, Three.js imports, scene construction, camera setup, mesh creation, material usage, animation loops, and shader usage.

Graph nodes:

- ThreeScene
- Mesh
- Camera
- Light
- Material
- Shader
- ReactComponent

Edges:

- `RENDERS_IN`
- `CONTAINS_MESH`
- `ATTACHES_CAMERA`
- `USES_MATERIAL`
- `USES_SHADER`
- `EMBEDDED_IN_COMPONENT`

---

## Phase 9.3 — Symbolic Editing MVP

Tools:

- `replace_symbol_body`
- `insert_before_symbol`
- `insert_after_symbol`
- `preview_edit`
- `apply_edit`
- `reindex_after_edit`

Rules:

- dry-run first
- show affected files
- require explicit apply
- reindex changed files
- no blind text replacement
- no full-file rewrite unless needed
- readonly profile must hide mutation tools

---

## Phase 9.4 — Rename / Refactor MVP

Tools:

- `rename_symbol`
- `preview_rename`
- `find_declaration`
- `find_implementations`
- `find_references_lsp`

Rules:

- use LSP when available
- preview affected edits
- apply atomically where possible
- reindex after apply
- fallback to readonly when LSP capability is missing

---

## Phase 9.5 — Backend Language Packs

Support Python, Java, Go, PHP, and Ruby.

Target:

- Basic for all
- Good for Python / Java / Go where feasible

---

## Phase 9.6 — Systems/Mobile Language Packs

Support C, C++, Swift, Kotlin, and Dart if useful.

Target:

- Basic first
- Good where feasible

---

## Phase 9.7 — Config/Data/Web Language Packs

Support SQL, YAML, JSON, HTML, CSS / SCSS, TOML if useful, and XML if useful.

Support level:

- file detection
- top-level symbols/keys
- imports/includes where applicable
- FTS
- config key extraction

---

## Phase 9.8 — Language Pack Benchmark + Quality Audit

Measure indexing correctness, symbol extraction quality, relationship extraction quality, route extraction quality, messaging extraction quality, Docker extraction quality, SignalR extraction quality, WPF extraction quality, false positives, false negatives, parser crash/error rate, and LSP timeout/failure rate.

---

## Phase 9.9 — Refactor Checkpoint B

Clean language backend abstractions after real-world usage.

---

## Phase 9.10 — Performance Optimization Pass B

Optimize measured multi-language bottlenecks.

---

## Phase 10 — Local Embeddings + Vector Search

Add semantic retrieval as a secondary signal.

Allowed local/free providers:

- Ollama
- fastembed
- Candle if feasible
- local Qdrant only as an optional local component

Rules:

- no cloud embeddings required
- no OpenAI / Anthropic / Gemini required
- cloud providers optional plugin only, disabled by default
- semantic signal remains secondary to AST/graph/FTS

---

## Phase 10.1 — Semantic Context Upgrade

Add semantic search, semantic + graph reranking, semantic fallback, semantic duplicate dedup, and embedding freshness tracking.

---

## Phase 10.2 — Session Memory + Context Virtualization

Inspired by Context Mode and Token Savior.

Add SessionEvent, SessionSnapshot, DecisionMemory, TaskMemory, ErrorFixMemory, ResumeContext, compact/resume support, local SQLite memory, and no telemetry.

---

## Phase 10.3 — Transcript Discovery / Token Opportunity Report

Scan agent transcripts/logs locally, detect repeated grep/read chains, detect missed context-pack opportunities, detect long command output opportunities, produce adoption report, and suggest better B3 tool usage.

---

## Phase 11 — Architecture Intelligence

Add architecture-level understanding, including group-level and cross-project analysis.

Add community detection, module boundary detection, dependency cluster map, circular dependency reports, architecture layer detection, service map, multi-service flow analysis, project group overview, cross-project dependency flow, frontend -> backend -> worker flow, and API -> message broker -> consumer flow.

Algorithms:

- connected components
- SCC
- PageRank / centrality
- label propagation or Louvain if feasible
- dependency clustering

---

## Phase 12 — Git Intelligence

Add recent churn score, last modified commit, commit frequency, hotspot files, author count, and ranking/risk integration.

---

## Phase 13 — Duplicate / Similarity Detection

Add AST fingerprinting, normalized AST hash, MinHash, SimHash, duplicate function detection, and similar code search.

---

## Phase 14 — Real Plugin System

Add plugin registry runtime, plugin lifecycle, capability discovery, language plugin loading, ranking plugin loading, embedding plugin loading, compactor plugin loading, and external providers optional and disabled by default.

---

## Phase 15 — Packaging + Installers

Add MCP install command, Cursor config helper, Codex config helper, Claude config helper if applicable, doctor command, uninstall command, optional Tauri app, and release packaging.

---

## When Can We Use It?

| Use case | Status |
|---|---|
| Test MCP runtime with Codex/Cursor | Usable now |
| Rust repositories | Usable now |
| Command output compaction | Usable now |
| Project init/index workflow | Usable now |
| MCP tool profiles | Phase 8.6 |
| Multi-project local workflow | Phase 8.8 |
| Project groups | Phase 8.8 |
| C# Web API / backend services | Phase 9.1 / 9.2 |
| React / Angular / TypeScript / JavaScript | Phase 9.2 |
| Node.js REST API | Phase 9.2.1 |
| Kafka / ksqlDB | Phase 9.2.2 |
| RabbitMQ | Phase 9.2.2 |
| Docker / docker-compose | Phase 9.2.3 |
| SignalR | Phase 9.2.4 |
| C# WPF | Phase 9.2.5 |
| Three.js / WebGL | Phase 9.2.6 |
| Refactor assistant | Phase 9.3 / 9.4 |
| Full memory/context platform | Phase 10.2+ |

B3 can run today as a local MCP/runtime/control/UI platform, but broad real-world app-stack intelligence depends on Phase 9.x.

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

Track cold startup time, MCP initialize latency, MCP tools/list latency, MCP tools/call latency, common query latencies, graph traversal latency, context pack latency, impact analysis latency, indexing speed, changed-file reindex latency, watcher debounce overhead, SQLite query latency, parser worker latency, command compaction latency, and approximate memory use when feasible.

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

### Advanced

- framework routes
- DI relationships
- cross-file references
- inheritance/interface edges
- component relationships
- LSP references/definitions
- safe editing support

### Priority

First priority is based on common real-world application stacks:

- C#
- TypeScript
- JavaScript
- React / TSX
- Angular
- Node.js

Then:

- Docker
- Kafka
- ksqlDB
- RabbitMQ
- SignalR
- WPF
- Three.js / WebGL

Then:

- Python
- Java
- Go
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
- Framework detection should be conservative and explainable.
- No cloud APIs are required.
- Offline-first and free-by-default remain hard requirements.
- External/cloud/paid integrations are optional plugins only and disabled by default.
