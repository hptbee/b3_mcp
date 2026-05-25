# Requirements

## Product Vision

Build a high-performance local-first MCP code intelligence platform for Cursor and Codex.

The system combines:
- TokenSave-style token-saving semantic graph queries
- Codebase Memory MCP-style persistent tree-sitter knowledge graphs

Primary goal:
Agents should query indexed structure instead of repeatedly scanning files.

## Non-Goals

Do not build:
- cloud-first architecture
- microservice architecture
- Electron app
- Neo4j-dependent graph engine
- Python hot-path runtime
- Docker-first local UX
- semantic-only RAG system

## Stack

Keep:
- Rust backend/core
- Tokio
- Axum
- rmcp
- tree-sitter
- SQLite/libSQL local DB with WAL
- local Qdrant only
- DashMap
- tracing
- Next.js
- TypeScript
- React Flow
- WebSocket
- Tauri optional

## Offline-First Requirement

The default installation must work fully offline.

Core functionality must not require:
- external APIs
- cloud authentication
- hosted vector databases
- remote telemetry
- SaaS dependencies
- internet connectivity

External services may be supported only as optional plugins, disabled by default.

Preferred local providers:
- SQLite/libSQL local mode
- local Qdrant
- Ollama
- GGUF embedding models
- sentence-transformers
- Candle
- fastembed

## Plugin Readiness Contracts

Pre-Phase-4 plugin work is limited to contracts:
- stable plugin identifiers
- plugin metadata
- capability discovery
- lifecycle boundaries
- timeout and cancellation expectations

Plugins must not make external services required. Any cloud or hosted provider
must remain optional, plugin-based, and disabled by default.

## Core Capabilities

Must provide, over the implementation phases:
- MCP server for Cursor/Codex
- local Codex/Cursor MCP config helper
- optional local multi-repo registry
- metadata-only project groups/workspaces
- language backend contracts and capability discovery
- persistent code graph
- semantic search with local embeddings
- FTS/BM25 keyword search
- graph traversal
- impact analysis
- smart context packs
- token savings ledger
- incremental indexing
- daemon/file watcher mode
- branch-aware indexing
- cross-session memory
- localhost UI
- graph explorer
- config editor
- tool playground
- diagnostics dashboard

## Required MCP Tools

Indexing:
- `index_project`
- `sync_project`
- `project_status`
- `watch_project`
- `doctor`

Search:
- `find_symbol`
- `search_code`
- `semantic_search`
- `search_code_graph`

Graph:
- `find_references`
- `find_callers`
- `find_callees`
- `trace_dependency`
- `impact_analysis`
- `detect_cycles`
- `community_map`

Context:
- `get_context_pack`
- `explain_symbol`
- `summarize_module`
- `list_related_files`

Memory:
- `record_decision`
- `record_code_area`
- `session_recall`

Token Saving:
- `estimate_tokens_saved`
- `savings_report`

Edit:
- `anchored_replace`
- `atomic_multi_replace`
- `insert_at_anchor`

## MCP Tool Profiles

The MCP runtime must support static local tool profiles to reduce `tools/list`
manifest noise and token overhead. The default profile is `optimized`, not
`full`.

Supported profiles:

- `tiny`
- `optimized`
- `full`
- `debug`
- `readonly`
- `editing`
- `web-app`
- `enterprise`

Hidden tools must not execute. They should return a structured profile-aware
error when practical. Future mutation tools must be hidden from `readonly` and
reserved for `editing`, `full`, or `debug` only when explicitly allowed.

## Architecture

Use a hybrid monolith:
- lightweight MCP runtime
- shared core engine
- async/background worker pipelines
- optional control server
- optional localhost UI

MCP runtime responsibilities:
- stdio transport
- JSON-RPC
- tool routing
- local tool profile filtering
- concise tools/list manifest generation
- streaming
- cancellation
- session lifecycle

Installer/helper responsibilities:
- local MCP config snippet generation
- dry-run install plans
- safe local config updates when explicitly applied
- backups before config writes
- local doctor diagnostics
- hook placeholders disabled by default

Registry responsibilities:
- local JSON project registry
- project registration/list/status
- metadata-only project groups
- repo-local database paths per project
- no automatic filesystem scanning

Language backend architecture responsibilities:
- local file/language detection
- backend metadata and capability discovery
- support level reporting
- tree-sitter backend contract for indexing
- future LSP backend contract for semantic operations
- unsupported language fallback without fake symbols

Never put these in the MCP hot path:
- full indexing
- embedding generation
- unbounded graph traversal
- blocking IO
- large filesystem scans

Installer/helper must not:
- execute user commands
- intercept shells
- modify shell profiles
- start telemetry
- call external APIs
- require agent installation to generate config

Registry must not:
- require the registry for single-project mode
- merge project graphs
- run cross-project queries
- own architecture intelligence
- use cloud sync, telemetry, hosted DBs, or external APIs

Language backend architecture must not:
- claim unimplemented parser/LSP capabilities
- require LSP until the local LSP backend phase implements it
- call external tools or network services for detection
- add cloud or paid language services

## Indexing Pipeline

Discovery
-> Ignore Filtering
-> Language Detection
-> File Hashing
-> tree-sitter Parsing
-> Symbol Extraction
-> Relationship Extraction
-> Graph Update
-> FTS Update
-> Embedding Queue
-> Cache Update

Must support:
- incremental indexing
- changed-file-only reindexing
- per-symbol hashes
- git-aware change detection
- branch-aware database namespace
- subprocess-isolated parser workers
- parser timeout/retry policy
- branch-aware parse failure registry
- parallel worker pools

## LSP Backend

Phase 9.1 adds a local LSP backend foundation.

Must support:
- disabled-by-default LSP config
- explicitly configured local stdio language-server processes
- workspace initialization
- full-document open/sync messages
- capability discovery
- diagnostics collection
- basic definition, references, and implementation requests when the server supports them
- startup/request timeouts and bounded stderr capture
- clear disabled, missing-server, crash, timeout, invalid-response, unsupported-capability, file, and URI errors

Must not:
- install or download language servers
- require internet access
- require cloud APIs or paid/proprietary backends
- replace tree-sitter indexing, SQLite graph storage, FTS, query engine, or Rust parser behavior
- add symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.1
- debounced filesystem watcher
- generated/vendor file skipping
- crash recovery

## Web Language Indexing

Phase 9.2 adds basic local indexing for JavaScript, TypeScript, JSX, and TSX.

Must support:
- file detection for `.js`, `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`, `.jsx`, `.tsx`, and `.cs`
- local tree-sitter parsing for JS/TS/JSX/TSX
- basic symbol extraction for functions, arrow/function variables, classes, methods, TypeScript interfaces, type aliases, enums, exports, and obvious component-like JSX/TSX declarations
- import extraction for ESM imports and CommonJS `require(...)`
- CommonJS export assignment detection
- safe `CONTAINS` and `IMPORTS` relationships without fake external package file nodes
- no `npm`, `node`, `tsc`, `eslint`, cloud parser, external API, telemetry, or runtime install requirement

Must defer:
- deep React runtime behavior, state-machine inference, full JSX tree graph,
  and framework-specific router intelligence
- Angular module/route/template intelligence
- ASP.NET Core / C# Web API intelligence beyond the Phase 9.2.5 static/basic
  controller and route extractor
- JS/TS call graph extraction unless it can be made low-noise
- symbolic editing, rename/refactor, embeddings, semantic search, and cross-project architecture intelligence

## Node.js REST API Intelligence

Phase 9.2.1 adds basic local REST route intelligence for Node.js projects.

Must support:
- package.json detection for Express, NestJS, Fastify, TypeScript, and detect-only frontend packages where obvious
- Express route calls and router calls
- NestJS controller and method decorator routes with class/method path composition
- Fastify shorthand calls and basic route object calls
- route metadata represented locally without runtime execution
- read-only route listing through the control API when route metadata is indexed
- route cleanup through normal deleted-file index cleanup

Must not require:
- `npm install`
- `node`
- `tsc`
- `eslint`
- Nest CLI
- package registry access
- cloud parsers or external APIs

Must defer:
- deep middleware execution order
- Nest guards/interceptors/pipes/modules/deep dependency injection
- request lifecycle inference
- advanced React component graph/runtime intelligence
- deep Angular compiler/template/DI/module graph intelligence
- ASP.NET Core/C# intelligence beyond basic static Web API route extraction
- realtime, messaging, cloud, infrastructure, Go, symbolic editing, embeddings, semantic search, and cross-project intelligence

## React / TSX Component Intelligence

Phase 9.2.2 adds basic local React component intelligence for JavaScript,
TypeScript, JSX, and TSX projects.

Must support:
- React package detection from package.json for `react`, `react-dom`, and `@types/react`
- React import/package detection through existing import extraction
- function, arrow-function, class, memo, and forwardRef component metadata where statically obvious
- default and named export metadata where statically obvious
- props type/interface name extraction from local annotations and React.FC generics
- basic JSX component usage names for PascalCase tags
- basic hook name detection for built-in hooks and custom `use[A-Z]*` hooks
- read-only component listing through the control API when component metadata is indexed
- no duplicate components on reindex and cleanup through normal deleted-file cleanup

Must not require:
- `npm install`
- `node`
- `tsc`
- `eslint`
- React dev server or build tooling
- package registry access
- cloud parsers or external APIs

Must defer:
- deep Angular compiler/template/DI/module graph intelligence
- Vue and Svelte intelligence
- React runtime rendering behavior
- state machine inference
- deep hook semantics and dependency-array analysis
- CSS/layout intelligence
- full JSX tree graph
- symbolic editing, rename/refactor, embeddings, semantic search, and cross-project intelligence

## Next.js Intelligence

Phase 9.2.3 adds basic static Next.js intelligence on top of the completed
React / TSX component support.

Must support:
- Next.js package detection from `package.json`
- detection of `next.config.js`, `next.config.mjs`, and `next.config.ts`
- App Router structure under `app/`
- Pages Router structure under `pages/`
- safe mapping of page, layout, loading, error, and not-found files to routes
- dynamic route segment detection for `[id]`, `[...slug]`, and `[[...slug]]`
- `app/api/**/route.ts` and `route.js` API handler detection
- route handler method detection for `GET`, `POST`, `PUT`, `PATCH`, `DELETE`,
  `OPTIONS`, and `HEAD`
- `"use client"` boundary detection
- basic static server/client component classification
- preservation of React component intelligence from Phase 9.2.2
- read-only route listing through `GET /api/routes` using `framework=nextjs`

Must not require:
- `next dev`
- `next build`
- `tsc`
- `eslint`
- `npm install`
- `node`
- package-manager scripts
- package registry access
- cloud parsers or external APIs

Must defer:
- runtime rendering
- full React Server Components semantics
- middleware execution order
- Vercel/deployment intelligence
- NextAuth/auth intelligence
- deep data fetching semantics
- deep Angular compiler/template/DI/module graph behavior, ASP.NET Core/C#
  beyond basic static Web API extraction, ORM/database, realtime, messaging,
  cloud/infrastructure, Go, symbolic editing, rename/refactor, embeddings,
  semantic search, and cross-project intelligence

## Angular Intelligence

Phase 9.2.4 adds basic static Angular intelligence on top of TypeScript support.

Must support:
- Angular package detection from `package.json`
- detection of `angular.json` and `tsconfig.app.json`
- static decorator detection for `@Component`, `@Injectable`, `@NgModule`,
  `@Directive`, and `@Pipe`
- component selector, template URL, style URL, standalone, imports, and
  provider metadata where represented as safe literals
- service `providedIn` metadata and constructor dependency type names
- module declarations/imports/providers/exports/bootstrap names
- basic Angular route config extraction from static object literals
- read-only route listing through `GET /api/routes` using `framework=angular`
- Angular component listing through `GET /api/components` using
  `framework=angular`

Must not require:
- `ng serve`
- `ng build`
- Angular compiler
- `tsc`
- `eslint`
- `npm install`
- `node`
- package-manager scripts
- package registry access
- cloud parsers or external APIs

Must defer:
- runtime template checking
- full template type checking
- lifecycle execution semantics
- full DI container/module graph resolution
- RxJS/NgRx deep flow analysis
- Angular Material intelligence
- ASP.NET Core/C# beyond basic static Web API extraction, ORM/database,
  realtime, messaging, cloud/infrastructure, Go,
  symbolic editing, rename/refactor, embeddings, semantic search, and
  cross-project intelligence

## ASP.NET Core / C# Web API Intelligence

Phase 9.2.5 adds basic static ASP.NET Core / C# Web API intelligence.

Must support:
- `.cs` file detection and basic static symbol extraction for namespaces,
  classes, methods, constructors, and using/package references
- `.csproj` detection for `Microsoft.NET.Sdk.Web`,
  `Microsoft.AspNetCore.App`, `Microsoft.AspNetCore.Mvc`,
  `Microsoft.AspNetCore.Mvc.Core`, ASP.NET Core package references, framework
  references, and target framework metadata
- controller detection from `Controller` suffix, visible `ControllerBase` /
  `Controller` inheritance text, `[ApiController]`, and `[Route]`
- common route and HTTP method attributes: `[Route]`, `[HttpGet]`,
  `[HttpPost]`, `[HttpPut]`, `[HttpPatch]`, `[HttpDelete]`, `[HttpHead]`,
  and `[HttpOptions]`
- route composition with controller/action route templates, `[controller]`,
  `[action]`, empty method routes, and preserved parameter tokens such as `{id}`
- action method metadata and route handler links where the method is locally
  visible
- constructor dependency type names as metadata only
- ASP.NET Core routes exposed through `GET /api/routes` with
  `framework=aspnetcore`

Must not require:
- `dotnet restore`, `dotnet build`, `dotnet run`, or `dotnet test`
- NuGet or package registry access
- Roslyn, Visual Studio, Rider, OmniSharp, or C# language servers
- runtime execution, app startup, external APIs, cloud parsers, telemetry, or
  paid/proprietary dependencies

Must defer:
- full semantic C# analysis and type checking
- full Microsoft DI container graph resolution
- middleware pipeline analysis
- minimal API analysis beyond future low-risk static work
- ORM/database behavior beyond basic static data access callsite metadata
- WPF/XAML and .NET desktop intelligence
- realtime/socket, messaging/event-driven, cloud/infrastructure, Go,
  symbolic editing, rename/refactor, embeddings, semantic search, and
  cross-project intelligence

## ORM / Database Access Intelligence

Phase 9.2.6 adds basic static ORM/database access intelligence.

Must support:
- local package/project detection for Entity Framework Core, Dapper, Prisma,
  TypeORM, Sequelize, and selected SQL driver hints
- EF Core `DbContext` and `DbSet<T>` detection
- EF Core obvious query/change callsite detection with coarse operations
- Dapper `Query*` and `Execute*` detection with direct literal SQL capture
  where visible
- Prisma `PrismaClient` construction and `prisma.<model>.<operation>()`
  detection
- TypeORM `@Entity` and obvious repository/manager calls
- Sequelize model declarations and model query calls
- read-only data access listing through `GET /api/data-access`
- metadata cleanup through normal deleted-file index cleanup

Must not require:
- database connections
- SQL execution
- migration execution
- `dotnet restore`, `dotnet build`, `dotnet run`, or `dotnet test`
- `npm install`, `node`, Prisma generate, TypeORM CLI, or Sequelize CLI
- package registry access, cloud databases, external APIs, telemetry, or
  paid/proprietary dependencies

Must defer:
- full SQL parsing and query optimization
- full LINQ expression semantics
- full C# or TypeScript type checking
- schema introspection from live databases
- runtime DB/ORM behavior
- cross-project data lineage
- realtime/socket, messaging/event-driven, cloud/infrastructure, Go,
  WPF/XAML, symbolic editing, rename/refactor, embeddings, semantic search, and
  cross-project architecture intelligence

## Graph Requirements

Node types:
- Project
- File
- Module
- Namespace
- Class
- Struct
- Interface
- Enum
- Function
- Method
- Variable
- Route
- Endpoint
- ConfigKey
- Test
- Package
- Decision
- CodeArea

Edge types:
- CONTAINS
- IMPORTS
- CALLS
- REFERENCES
- IMPLEMENTS
- INHERITS
- DEPENDS_ON
- TESTS
- ROUTES_TO
- READS_CONFIG
- WRITES_CONFIG
- SIMILAR_TO
- TOUCHES
- DECIDES

Every edge should support:
- confidence
- source/provenance
- created_at
- updated_at

## Retrieval Requirements

Use hybrid retrieval. Never rely on semantic search alone.

Ranking should include:
- exact symbol match
- FTS/BM25 lexical score
- semantic similarity
- graph distance
- active session relevance
- recency
- centrality
- test relevance

Context packs must:
- be token-budget aware
- deduplicate snippets
- include why each item was included
- include expansion handles
- avoid full-file dumps by default

## UI Requirements

The localhost UI must remain separate from the MCP hot path.

Features:
- dashboard
- project list
- indexing monitor
- graph explorer
- dependency path view
- call graph view
- community map
- token savings dashboard
- session memory viewer
- tool playground
- config editor
- logs viewer
- cache inspector
- diagnostics

## Performance Requirements

Optimize for:
1. low MCP query latency
2. token reduction
3. fast startup
4. low RAM
5. incremental indexing speed

Use:
- bounded graph expansion
- hot cache
- WAL mode
- prepared statements
- batched writes
- cancellation tokens
- bounded worker pools

Avoid:
- semantic-only search
- full-file dumps
- unbounded result lists
- global mutable state
- sync mutexes in hot paths
- blocking IO in MCP hot paths

## Project Roadmap

### Completed Phases

- Phase 1: Workspace / Scaffold
- Phase 1.5: Contracts / Boundaries
- Phase 2: SQLite Storage / Schema Foundation
- Phase 3: Incremental Indexer Skeleton
- Phase 3.1: Indexer Audit / Cleanup
- Pre-Phase-4: Plugin Contracts / Docs / CI
- Phase 4: Real Rust Parsing + Storage Adapter
- Phase 4.1: Project/Branch Auto Ensure + Deleted File Cleanup
- Phase 5: Query Engine + Graph Traversal + Context Pack
- Phase 5.1: Query Hardening + Explainability
- Phase 5.2: Ranking Algorithms Upgrade
- Phase 6: MCP Tools over Query Engine
- Phase 6.1: Impact Intelligence
- Phase 6.2: PageRank / Centrality
- Phase 7: Control Server + Localhost API
- Phase 7.1: Web UI Foundation
- Phase 7.2: Graph Explorer UI
- Phase 7.3: Query Trace UI
- Phase 8: File Watcher + Daemon Mode
- Phase 8.1: Parser Isolation
- Phase 8.2: Benchmark Harness + Performance Baseline
- Phase 8.3: Refactor Checkpoint A
- Phase 8.4: Performance Optimization Pass A
- Phase 8.5: Command Output Compaction
- Phase 8.5.1: Project Init + Manual Index Command
- Phase 8.5.1.1: Repository Structure Audit + Folder/File Cleanup
- Phase 8.6: MCP Tool Profiles + Manifest Slimming
- Phase 8.7: Agent Install Helper + Hook Integration Foundation
- Phase 8.8: Multi-repo Registry + Project Groups
- Phase 9.0: Language Backend Architecture
- Phase 9.1: LSP Backend MVP
- Phase 9.2: Web Application Priority Support A
- Phase 9.2.1: Node.js / REST API Intelligence
- Phase 9.2.2: React / TSX Component Intelligence
- Phase 9.2.3: Next.js Intelligence
- Phase 9.2.4: Angular Intelligence
- Phase 9.2.5: ASP.NET Core / C# Web API Intelligence
- Phase 9.2.6: ORM / Database Access Intelligence

### Planned Phases

- Phase 9.2.7: Realtime / Socket Intelligence
- Phase 9.2.8: Messaging / Event-driven Intelligence
- Phase 9.2.9: Cloud / Infrastructure Intelligence
- Phase 9.2.10: Go Language Support
- Phase 9.2.11: Scoped Indexing + Intelligence Targets
- Phase 9.2.12: .NET Desktop / WPF Intelligence
- Phase 9.3/9.4: Symbolic editing and rename/refactor support
- Phase 10: Local Embeddings + Vector Search
- Phase 10.1: Semantic Context Upgrade
- Phase 10.2+: Local session memory and context platform
- Phase 11: Cross-Project Architecture Intelligence
- Phase 12: Git Intelligence
- Phase 13: Duplicate / Similarity Detection
- Phase 14: Real Plugin System
- Phase 15: Packaging + Installers
