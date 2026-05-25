# B3 Project Plan

B3 is a local-first, offline-first, free-by-default AI-native code intelligence platform for coding agents and local developer workflows.

This document is the source of truth for the detailed roadmap. `README.md` should remain concise and traditional.

---

## Current Roadmap Status

```text
Completed:
- Phase 1 — Workspace / Scaffold
- Phase 1.5 — Contracts / Boundaries
- Phase 2 — SQLite Storage / Schema Foundation
- Phase 3 — Incremental Indexer Skeleton
- Phase 3.1 — Indexer Audit / Cleanup
- Pre-Phase-4 — Plugin Contracts / Docs / CI
- Phase 4 — Real Rust Parsing + SqliteStorage ↔ IndexStore Adapter
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
- Phase 8.5.1 — Project Init + Manual Index Command
- Phase 8.5.1.1 — Repository Structure Audit + Folder/File Cleanup + Web UI Port 8888
- Phase 8.6 — MCP Tool Profiles + Manifest Slimming
- Phase 8.7 — Agent Install Helper + Hook Integration Foundation
- Phase 8.8 — Multi-repo Registry + Project Groups
- Phase 9.0 — Language Backend Architecture
- Phase 9.1 — LSP Backend MVP
- Phase 9.2 — Web Application Priority Support A
- Phase 9.2.1 — Node.js / REST API Intelligence
- Phase 9.2.2 — React / TSX Component Intelligence
- Phase 9.2.3 — Next.js Intelligence
- Phase 9.2.3.1 — Indexer Module Split / Refactor Checkpoint B
- Phase 9.2.4 — Angular Intelligence
- Phase 9.2.4.1 — Web Module Split / Refactor Checkpoint C
- Phase 9.2.5 — ASP.NET Core / C# Web API Intelligence
- Phase 9.2.6 — ORM / Database Access Intelligence
- Phase 9.2.7 — Realtime / Socket Intelligence
- Phase 9.2.8 — Messaging / Event-driven Intelligence
- Phase 9.2.9 - Cloud / Infrastructure Intelligence
- Phase 9.2.10 - Go Language Support
- Phase 9.2.11 - Scoped Indexing + Intelligence Targets

Next:
- Phase 9.2.12 - .NET Desktop / WPF Intelligence

Later:
- Phase 9.3 — Symbolic Editing MVP
- Phase 9.4 — Rename / Refactor MVP
- Phase 9.5 — Additional Backend Language Support
- Phase 9.6 — Systems / Mobile Language Support
- Phase 9.7 — Config / Data / Web File Support
- Phase 9.8 — Language and Technology Quality Audit
- Phase 9.9 — Refactor Checkpoint D
- Phase 9.10 — Performance Optimization Pass B
- Phase 10 — Local Embeddings + Vector Search
- Phase 10.1 — Semantic Context Upgrade
- Phase 10.2 — Session Memory + Context Virtualization
- Phase 10.3 — Transcript Discovery / Token Opportunity Report
- Phase 10.4 — Web UI Developer Console Refresh
- Phase 11 — Cross-Project Architecture Intelligence
- Phase 12 — Git Intelligence
- Phase 13 — Duplicate / Similarity Detection
- Phase 14 — Real Plugin System
- Phase 15 — Packaging + Installers
```

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
- local CLI install/doctor/uninstall helper
- local multi-repo registry
- project group metadata
- language backend architecture
- local LSP backend foundation
- basic JavaScript/TypeScript/JSX/TSX indexing
- basic static framework intelligence for implemented web stacks
- basic static ORM/data-access intelligence
- basic static realtime/socket intelligence
- basic static messaging/event-driven intelligence
- future cloud/infrastructure intelligence
- scoped indexing targets
- future symbolic editing
- future local embeddings and vector search
- future local session memory
- future cross-project architecture intelligence

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
- JetBrains paid plugins
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
5. **Indexer owns indexing** — parsing, discovery, hashing, incremental indexing, watcher, parser worker, parse failures, and language/technology extraction live in `b3-indexer`.
6. **Query owns intelligence** — graph traversal, ranking, context packing, impact analysis, dependency tracing, cycle detection, and query trace live in `b3-query`.
7. **Control server is an adapter** — localhost HTTP/SSE only; no storage internals or query intelligence.
8. **Web UI talks only to local control server** — no cloud calls.
9. **Graph first, semantic second** — AST, graph, and FTS are core; embeddings are optional future secondary signals.
10. **Benchmark before optimization** — measure first, optimize measured bottlenecks only.
11. **Refactor only after verified milestones** — prefer small targeted refactors.
12. **Language support must be layered** — language syntax support, framework/library detection, and framework intelligence are separate layers.
13. **LSP complements tree-sitter** — tree-sitter for fast indexing; local LSP for semantic operations.
14. **Project model supports standalone and grouped projects** — `1 project = 1 .b3/b3.db` by default.
15. **Framework detection must be conservative** — do not invent routes, handlers, components, topics, queues, infrastructure resources, or relationships with low confidence.
16. **UI follows data maturity** — major UI refresh should happen after enough backend intelligence exists to display useful information.

---

## Current Capabilities

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

The `optimized` profile exposes 7 high-value tools by default:

- `find_symbol`
- `search_code`
- `related_symbols`
- `impact_analysis`
- `get_context_pack`
- `compact_command_output`
- `savings_report`

Profile tool counts:

```text
tiny        5
optimized   7
full       11
debug      11
enterprise  9
```

### Control Server

Control server:

```text
http://127.0.0.1:7777
```

Current local APIs include:

- `GET /health`
- `GET /api/status`
- `GET /api/project`
- `POST /api/index/run`
- `POST /api/index/reindex`
- `GET /api/index/status`
- `GET /api/capabilities`
- `GET /api/languages`
- `GET /api/lsp/status`
- `GET /api/lsp/servers`
- `GET /api/routes`
- `GET /api/components`
- `GET /api/data-access`
- `GET /api/realtime`
- `GET /api/messaging`
- `GET /api/infrastructure`

Registry control APIs are deferred. Registry management is currently CLI-only.

### Web UI

Web UI:

```text
http://127.0.0.1:8888
```

The current UI is a local single-project developer console. Dedicated views for routes, components, data access, realtime, messaging, infrastructure, registry, and project groups are deferred until the data model is mature enough.

---

## Current Language Support

Support levels are intentionally honest.

### Rust — Good

Backend:

- `tree-sitter-rust`

Capabilities:

- file detection
- parsing
- symbol extraction
- import extraction
- basic relationship extraction

### JavaScript — Basic

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

### TypeScript — Basic

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

### JSX / TSX — Basic

Backend:

- JavaScript/TypeScript tree-sitter grammars

Capabilities:

- `.jsx`, `.tsx` detection
- parsing
- component-like declarations where structurally obvious
- JSX component usages where safe
- imports / exports
- conservative relationships

### C# — Basic Static

Backend:

- conservative static/text extraction

Capabilities:

- `.cs` and `.csproj` detection
- namespace/class/method/constructor extraction
- using/package reference hints
- ASP.NET Core Web API controller/action route extraction
- constructor DI type-name metadata
- EF Core, Dapper, SignalR, RabbitMQ, Kafka, and Google Pub/Sub static hints where implemented

Limitations:

- no Roslyn requirement
- no full C# semantic analysis
- no `dotnet` execution
- no package restore
- no runtime behavior

### Other Planned Languages

Other planned languages are detect-only or unsupported unless explicitly implemented.

---

## Project Model

B3 supports standalone projects and project groups.

Default storage model:

```text
1 project = 1 repo-local .b3/b3.db
```

Registry:

```text
~/.b3/registry.json
```

Registry status:

- completed as local JSON registry support in `b3-cli`
- metadata only
- each project keeps its repo-local `.b3/b3.db`
- no graph merging
- no cross-project query execution
- no cloud sync
- no telemetry

Project group example:

```text
Business Application
├── Backend API
├── Frontend App
├── Worker Service
├── Desktop Client
└── Runtime Infrastructure
```

Deferred:

- control server registry write APIs
- Web UI registry view
- installer `--project-id`
- cross-project graph merging
- cross-project architecture intelligence

---

## Technology Intelligence Model

B3 separates language support from technology intelligence.

Technology intelligence teaches B3 how to understand application architecture, including:

- routes
- controllers
- services
- components
- modules
- middleware
- dependency injection
- data access callsites
- realtime events
- message handlers
- queues
- topics
- exchanges
- routing keys
- cloud resources
- infrastructure manifests
- deployment relationships

### Implemented Technology Intelligence

Web backend:

- Express
- NestJS REST
- Fastify
- ASP.NET Core Web API

Web frontend:

- React
- Next.js
- Angular

Data / ORM:

- Entity Framework Core
- Dapper
- Prisma
- TypeORM
- Sequelize

Realtime:

- WebSocket
- Socket.IO
- SignalR
- RSocket

Messaging / Event-driven:

- AMQP
- RabbitMQ
- Kafka
- Google Pub/Sub
- NestJS messaging

### Planned Technology Intelligence

Cloud / Infrastructure:

- Docker
- Docker Compose
- Kubernetes
- GCP
- GKE
- Terraform

Language / backend:

- Go

Desktop / Graphics:

- WPF
- Avalonia if useful
- Electron if useful
- Three.js
- WebGL

---

## Roadmap Details

## Phase 9.2.1 — Node.js / REST API Intelligence

Status: Completed with basic static/local REST route intelligence.

### Scope

- Express route extraction
- NestJS controller/method decorator route extraction
- Fastify route extraction
- route metadata encoded on `Route` symbols
- read-only `GET /api/routes`

### Out of Scope

- runtime request tracing
- dynamic runtime route generation
- deep middleware execution order
- Nest module graph, guards, interceptors, pipes, and deep DI

---

## Phase 9.2.2 — React / TSX Component Intelligence

Status: Completed with basic static/local React component intelligence.

### Scope

- function components
- arrow function components
- class components
- default and named exports
- props interfaces/types
- React package/import detection
- JSX component usages and parent-child component references where safe
- basic hook detection
- read-only `GET /api/components`

### Out of Scope

- full runtime rendering behavior
- state machine inference
- deep hook semantics
- full JSX tree graph

---

## Phase 9.2.3 — Next.js Intelligence

Status: Completed with basic static/local Next.js intelligence.

### Scope

- Next.js package/config detection
- App Router detection
- Pages Router detection
- dynamic route detection
- `app/api/**/route.*` route handler detection
- HTTP method export detection
- `"use client"` / `"use server"` boundary detection
- route metadata through `GET /api/routes?framework=nextjs`

### Out of Scope

- running `next dev`
- running `next build`
- full RSC semantics
- middleware execution order
- Vercel/deployment intelligence

---

## Phase 9.2.3.1 — Indexer Module Split / Refactor Checkpoint B

Status: Completed as a behavior-preserving refactor checkpoint.

The indexer keeps orchestration and shared contracts in `crates/b3-indexer/src/lib.rs`.

Web-language extraction moved under `crates/b3-indexer/src/web/`, and the large inline indexer test module moved to `crates/b3-indexer/src/tests.rs`.

No runtime behavior, storage schema, control API response, MCP tool/profile, or dependency changes are part of this checkpoint.

---

## Phase 9.2.4 — Angular Intelligence

Status: Completed with basic static/local Angular intelligence.

### Scope

- Angular package/config detection
- `@Component`
- `@Injectable`
- `@NgModule`
- `@Directive`
- `@Pipe`
- selector/template/style metadata where safe
- service `providedIn`
- basic constructor dependency type names
- module declarations/imports/providers/exports/bootstrap names
- route config metadata
- Angular component metadata via `GET /api/components?framework=angular`
- Angular route metadata via `GET /api/routes?framework=angular`

### Out of Scope

- full template type checking
- Angular compiler integration
- runtime behavior
- advanced RxJS flow inference
- deep DI/module graph resolution

---

## Phase 9.2.4.1 — Web Module Split / Refactor Checkpoint C

Status: Completed as a behavior-preserving refactor checkpoint.

`crates/b3-indexer/src/web/mod.rs` is now a small orchestration and re-export layer.

Existing JS/TS symbol extraction, Node REST routes, React component metadata, Next.js routes/config detection, shared route/component metadata, and tree-sitter helpers were split into focused web modules.

No runtime behavior, storage schema, control API response, MCP tool/profile, dependency, or Web UI behavior changed.

---

## Phase 9.2.5 — ASP.NET Core / C# Web API Intelligence

Status: Completed with basic static/local ASP.NET Core route intelligence.

### Scope

- `.cs` and `.csproj` detection
- ASP.NET Core project/package reference detection from `.csproj`
- conservative C# symbol extraction
- controller detection
- `[ApiController]`
- `[Route]`
- common HTTP method attributes
- controller/action route composition
- constructor DI type names
- `GET /api/routes?framework=aspnetcore`

### Out of Scope

- full Roslyn replacement
- full C# semantic analysis
- full DI graph resolution
- middleware pipeline analysis
- WPF/XAML
- ORM/database intelligence

### Completion Notes

Implemented as `crates/b3-indexer/src/csharp.rs`, outside the JS/TS `web/` module.

---

## Phase 9.2.6 — ORM / Database Access Intelligence

Status: Completed with basic static/local ORM/data-access metadata.

### Scope

- Entity Framework Core
- Dapper
- Prisma
- TypeORM
- Sequelize
- EF Core DbContext and DbSet detection
- Dapper Query/Execute callsite detection
- PrismaClient and model operation detection
- TypeORM entity/repository call detection
- Sequelize model/query call detection
- read-only `GET /api/data-access`

### Out of Scope

- DB connections
- SQL execution
- migrations
- full SQL parser
- full LINQ semantics
- runtime ORM behavior
- cross-project data lineage

### Completion Notes

Implemented as focused `crates/b3-indexer/src/data_access/` static extraction.

---

## Phase 9.2.7 — Realtime / Socket Intelligence

Status: Completed with basic static/local realtime metadata.

### Scope

- WebSocket
- Socket.IO
- SignalR
- RSocket
- event handlers
- event/channel names
- hub methods
- client/server flow metadata
- read-only `GET /api/realtime`

### Out of Scope

- live socket tracing
- runtime packet capture
- network connections
- server startup
- payload schema inference
- runtime flow inference
- cross-project event matching

### Completion Notes

Implemented as focused `crates/b3-indexer/src/realtime/` static extraction.

---

## Phase 9.2.8 — Messaging / Event-driven Intelligence

Status: Completed with basic static/local messaging metadata.

### Scope

- AMQP
- RabbitMQ
- Kafka
- Google Pub/Sub
- NestJS messaging
- generic Pub/Sub hints
- producers
- consumers
- topics
- queues
- exchanges
- routing keys
- consumer groups where literal and safe
- read-only `GET /api/messaging`

### Out of Scope

- broker connections
- cloud service calls
- runtime topic/queue discovery
- message consumption
- payload schema inference
- contract intelligence
- cross-project producer/consumer matching

### Completion Notes

Implemented as focused `crates/b3-indexer/src/messaging/` static extraction.

---

## Phase 9.2.9 — Cloud / Infrastructure Intelligence

Status: Next.

### Purpose

Understand infrastructure and deployment metadata through basic static/local extraction.

### Scope

- Dockerfile detection
- Docker Compose detection
- Kubernetes manifest detection
- Terraform `.tf` detection
- GCP/GKE resource hints
- images
- services
- containers
- ports
- environment keys
- labels/selectors
- Terraform providers/resources/modules/variables/outputs
- read-only `GET /api/infrastructure` if implemented

### Out of Scope

- `docker` execution
- `kubectl` execution
- `terraform` execution
- `gcloud` execution
- cloud API calls
- registry calls
- credential loading
- live cluster inspection
- cost estimation
- security scanning
- cross-project deployment matching

### Completion Notes

Implemented as focused `crates/b3-indexer/src/infrastructure/` static
extraction. Infrastructure records are encoded on existing symbols with
`infrastructure.*` metadata and exposed through storage/control adapters without
a schema migration.

Support is basic, static, local, and conservative:

- Dockerfile `FROM`, `EXPOSE`, `ENV`, `CMD`, and `ENTRYPOINT` metadata
- Docker Compose service names, images/build contexts, ports, environment
  keys, and `depends_on` service names
- Kubernetes YAML kind/name/namespace, labels, selectors, container names,
  images, ports, ingress/service backend hints, and GKE-oriented annotations
- Terraform provider/resource/module/variable/output blocks plus simple literal
  name/location/region/project hints
- GCP/GKE classification for visible `google_*` Terraform resources, including
  GKE cluster and node-pool resource types

The implementation does not run Docker, Docker Compose, `kubectl`, Terraform,
`gcloud`, cloud APIs, registries, module/provider downloads, cloud credential
loading, runtime discovery, security scanning, cost estimation, or cross-project
deployment matching.

---

## Phase 9.2.10 — Go Language Support

Status: Completed.

### Scope

- `.go` detection
- `go.mod` detection for module, require, and replace metadata
- packages
- imports
- functions
- structs
- interfaces
- methods
- type aliases and basic type declarations
- const/var declarations
- basic local call relationships where same-file names can be matched safely
- conservative HTTP route hints for `net/http`, Gin, Echo, Fiber, and Chi when
  visible local router construction makes the framework clear

This is basic static analysis in `b3-indexer`, not Go compiler/type checking.
It does not run `go build`, `go test`, `go run`, `go list`,
`go mod download`, module registry access, package restore, app code, gRPC
analysis, deep framework intelligence, symbolic editing, rename/refactor,
embeddings, semantic search, or cross-project architecture intelligence.

---

## Phase 9.2.11 — Scoped Indexing + Intelligence Targets

Status: Completed.

### Scope

- shared `IndexScope` and `ScopePreview` contracts in `b3-core`
- deterministic parser/validator for `project`, `path`, `file`, `glob`,
  `language`, `framework`, `route`, `component`, `module`, `data_access`,
  `realtime`, `messaging.*`, and `infrastructure` scopes
- dry-run preview with matched file counts, sample files, languages,
  frameworks, existing metadata targets, warnings, and skipped reasons
- scoped manual indexing and reindexing through `b3-control-server`
  `--scope`, `--dry-run`, and `--force`
- local API support through `POST /api/index/preview`,
  `POST /api/index/run`, and `POST /api/index/reindex`
- path/file/glob/language/framework filtering through local file discovery
- target scopes using existing indexed metadata for routes, components,
  data access, realtime, messaging, and infrastructure

### Boundaries

- Full-project indexing remains the default when no scope is provided.
- Target scopes do not invent metadata; zero matches are explicit and
  non-fatal, with a warning that a broader first index may be needed.
- Scoped reindex only touches matched files and preserves unrelated indexed
  files. Full reindex cleanup semantics are unchanged.
- No MCP tools or MCP profile counts changed.
- No command execution, package-manager execution, database/broker/cloud
  connection, embeddings, semantic search, symbolic editing, rename/refactor,
  runtime discovery, cross-project matching, or WPF/XAML intelligence was added.

---

## Phase 9.2.12 — .NET Desktop / WPF Intelligence

Status: Planned.

### Scope

- WPF/XAML file detection
- basic XAML view and code-behind relationships where safe
- MVVM naming/convention hints where conservative

### Out of Scope

- Visual Studio automation
- XAML runtime execution
- full WPF binding semantics

---

## Phase 9.3 — Symbolic Editing MVP

Status: Planned.

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

---

## Phase 9.4 — Rename / Refactor MVP

Status: Planned.

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

## Phase 9.5 — Additional Backend Language Support

Status: Planned.

Targets:

- Python
- Java
- PHP
- Ruby

Go is handled earlier in Phase 9.2.10 because it is part of the planned web/backend priority stack.

---

## Phase 9.6 — Systems / Mobile Language Support

Status: Planned.

Targets:

- C
- C++
- Swift
- Kotlin
- Dart if useful

---

## Phase 9.7 — Config / Data / Web File Support

Status: Planned.

Targets:

- SQL
- YAML
- JSON
- HTML
- CSS / SCSS
- TOML
- XML
- XAML if not already handled by WPF intelligence

---

## Phase 9.8 — Language and Technology Quality Audit

Status: Planned.

Measure:

- symbol extraction quality
- relationship extraction quality
- route extraction quality
- component extraction quality
- data-access extraction quality
- realtime extraction quality
- messaging extraction quality
- infrastructure extraction quality
- false positives
- false negatives
- parser crash/error rate
- LSP timeout/failure rate

---

## Phase 9.9 — Refactor Checkpoint D

Status: Planned.

This checkpoint is named **D** because:

- Phase 8.3 was Refactor Checkpoint A
- Phase 9.2.3.1 was Refactor Checkpoint B
- Phase 9.2.4.1 was Refactor Checkpoint C

### Rules

- no speculative rewrite
- preserve DTO compatibility where possible
- keep MCP runtime thin
- keep parser/indexing logic in indexer
- keep storage persistence in storage
- preserve existing APIs unless a migration is explicitly documented

---

## Phase 9.10 — Performance Optimization Pass B

Status: Planned.

Rules:

- benchmark first
- optimize measured bottlenecks only
- no telemetry
- no unrelated refactor

---

## Phase 10 — Local Embeddings + Vector Search

Status: Planned.

### Purpose

Add semantic retrieval as a secondary signal.

### Allowed Local/Free Providers

- local embedding model abstraction
- fastembed if feasible
- Candle if feasible
- Ollama if used as an optional local provider
- local vector index/storage

### Rules

- no cloud embeddings required
- no OpenAI / Anthropic / Gemini required
- no hosted vector database required
- no internet required at runtime
- cloud providers are optional plugins only, disabled by default
- semantic signal remains secondary to AST/graph/FTS

---

## Phase 10.1 — Semantic Context Upgrade

Status: Planned.

Scope:

- semantic search
- hybrid search: FTS + graph + vector
- graph-aware reranking
- semantic fallback
- duplicate deduplication
- embedding freshness tracking
- benchmark context quality impact

---

## Phase 10.2 — Session Memory + Context Virtualization

Status: Planned.

Scope:

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

## Phase 10.3 — Transcript Discovery / Token Opportunity Report

Status: Planned.

Scope:

- detect repeated grep/read chains
- detect missed context-pack opportunities
- detect long command output opportunities
- produce local adoption report
- suggest better B3 tool usage
- local logs only
- no uploads
- no telemetry

---

## Phase 10.4 — Web UI Developer Console Refresh

Status: Planned.

Purpose:

Upgrade the B3 Web UI into a polished local developer console after core language, route, framework, messaging, infrastructure, semantic search, and project-group capabilities are mature enough to display useful data.

Scope:

- app shell
- sidebar navigation
- dashboard cards
- project status
- indexing status
- language support page
- technology intelligence page
- route view
- component view
- data access view
- realtime view
- messaging view
- infrastructure view
- service view
- registry/project group visibility
- diagnostics/events improvements
- better empty/loading/error states

Rules:

- no paid UI kit
- no telemetry
- no cloud dependency
- no runtime internet requirement
- no fake data
- no backend intelligence hidden in UI

---

## Phase 11 — Cross-Project Architecture Intelligence

Status: Planned.

Scope:

- community detection
- module boundary detection
- dependency cluster map
- circular dependency reports
- architecture layer detection
- service map
- multi-service flow analysis
- project group overview
- cross-project dependency flow
- frontend -> backend route matching
- producer -> topic/queue/routing-key -> consumer matching
- app service -> infrastructure flow where safe
- group-level impact analysis
- group-level context packs

Algorithms:

- connected components
- strongly connected components
- PageRank / centrality
- label propagation or Louvain if feasible
- dependency clustering

---

## Phase 12 — Git Intelligence

Status: Planned.

Scope:

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

## Phase 13 — Duplicate / Similarity Detection

Status: Planned.

Scope:

- AST fingerprinting
- normalized AST hash
- MinHash
- SimHash
- duplicate function detection
- similar code search

---

## Phase 14 — Real Plugin System

Status: Planned.

Scope:

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

## Phase 15 — Packaging + Installers

Status: Planned.

Scope:

- release packaging
- binary install workflow
- optional Tauri app if still useful
- versioned artifacts
- platform notes
- install/doctor/uninstall polish

Note: Basic local agent install helpers already exist from Phase 8.7. This phase is about release-grade packaging and distribution polish.

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
| Local LSP backend foundation | Usable now, disabled by default |
| Node.js REST API / Express / NestJS / Fastify | Usable now, basic/static |
| React / TSX component intelligence | Usable now, basic/static |
| Next.js intelligence | Usable now, basic/static |
| Angular intelligence | Usable now, basic/static |
| ASP.NET Core / C# Web API | Usable now, basic/static |
| ORM / database access intelligence | Usable now, basic/static |
| WebSocket / Socket.IO / SignalR / RSocket | Usable now, basic/static |
| AMQP / RabbitMQ / Kafka / Google Pub/Sub / NestJS messaging | Usable now, basic/static |
| Docker / Docker Compose / Kubernetes / GCP / GKE / Terraform | Phase 9.2.9 |
| Go language support | Usable now, basic/static |
| Scoped indexing targets | Usable now |
| C# WPF / XAML | Phase 9.2.12 |
| Three.js / WebGL | Deferred |
| Registry Web UI | Deferred |
| Control registry APIs | Deferred |
| Refactor assistant | Phase 9.3 / 9.4 |
| Local embeddings / vector search | Phase 10 |
| Full memory/context platform | Phase 10.2+ |
| Cross-project architecture intelligence | Phase 11 |
| Release-grade packaging | Phase 15 |

B3 can run today as a local MCP/runtime/control/UI platform with Rust, basic JS/TS/JSX/TSX indexing, basic static Node.js REST route intelligence, basic static React/TSX component intelligence, basic static Next.js route/boundary intelligence, basic static Angular metadata, basic static ASP.NET Core / C# Web API route intelligence, basic static ORM/database access metadata, basic static realtime/socket metadata, basic static messaging/event-driven metadata, basic static cloud/infrastructure metadata, and basic static Go language support.

WPF/XAML, local embeddings, and cross-project architecture intelligence depend on Phase 9.2.12 and later.

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
- route extraction latency
- technology detection latency
- indexing speed
- changed-file reindex latency
- watcher debounce overhead
- SQLite query latency
- parser worker latency
- LSP request latency when enabled and tested locally
- command compaction latency
- data-access extraction latency
- realtime extraction latency
- messaging extraction latency
- infrastructure extraction latency when implemented
- approximate memory use when feasible

Benchmark fixtures must remain local, deterministic, small enough to commit, and free from network/cloud/API calls.

---

## Multi-language Strategy

Target: around 20 practical languages/file types.

Do not implement all languages in one phase. Use support levels.

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

First priority:

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
- Docker
- Kubernetes
- GCP
- GKE
- Terraform
- Go
- WPF

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
- XAML
- Three.js / WebGL
- ksqlDB

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

Only update affected files.

Every phase completion report must include:

A. Files changed  
B. Behavior implemented  
C. Boundaries preserved  
D. Docs updated  
E. Verification results  
F. Offline/free compliance result  
G. Remaining risks  
H. Deferred work  
I. READY / NOT READY for next phase

For larger feature phases, include phase-specific sections such as API behavior, storage behavior, benchmark status, MCP/profile compatibility, and UI impact.

---

## Reference Models and Borrowed Ideas

These projects are inspirations only, not dependencies. B3 must remain Rust-native where appropriate, local-first, offline-first, and free-by-default.

### codebase-memory-mcp

B3 learns tree-sitter code graph, persistent local code memory, MCP code intelligence, and graph visualization.

Covered by Phase 3 to Phase 7 and future Phase 9.x for multi-language graph expansion.

### TokenSave

B3 learns token-saving retrieval, indexed context instead of repeated grep/read, context pack strategy, and savings ledger concept.

Covered by Phase 5, Phase 5.1, Phase 5.2, and Phase 8.5 as an adjacent command compaction layer.

### RTK / Rust Token Killer

B3 learns command output compaction, git/test/build/lint output summarization, and local proxy/token-saving ideas.

Covered by Phase 8.5.

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

### Neo4j Browser-style UX

B3 learns graph explorer UX, node/edge inspector, and path/cycle visualization.

Covered by Phase 7.2 and Phase 7.2.1. B3 intentionally avoids a Neo4j dependency.

---

## Notes

- Do not implement all domains at once.
- Each domain must have tests and benchmark fixtures.
- Framework/library detection should be conservative and explainable.
- Language support, framework detection, and framework intelligence are separate layers.
- No cloud APIs are required.
- Offline-first and free-by-default remain hard requirements.
- External/cloud/paid integrations are optional plugins only and disabled by default.
