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
- normal SQLite vector tables first; local Qdrant may only be optional later
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
- deterministic test embedding provider for tests only
- future explicitly configured local embedding providers such as Ollama, GGUF,
  sentence-transformers, Candle, or fastembed
- optional local vector components only when disabled by default

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
- semantic search with local embeddings in later Phase 10.x work
- Phase 10.0 local embedding/vector contracts, chunk model, and vector store
  architecture
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
- `semantic_search` (local/offline hybrid search from Phase 10.4)
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
- messaging/event-driven, cloud/infrastructure, Go, WPF/XAML, symbolic editing,
  rename/refactor, embeddings, semantic search, and cross-project architecture
  intelligence

## Realtime / Socket Intelligence

Phase 9.2.7 adds basic static realtime/socket intelligence.

Must support:
- local package/project detection for WebSocket, Socket.IO, SignalR, and RSocket
  package hints
- browser/native `new WebSocket(...)`, message listener, and `send()` metadata
  where WebSocket context is present
- Socket.IO literal `on(...)` listener and `emit(...)` emitter metadata where
  Socket.IO context is present
- SignalR C# `Hub` classes, hub methods, `Clients.*.SendAsync(...)`, JS/TS
  `HubConnectionBuilder`, `connection.on(...)`, and `connection.invoke(...)`
- minimal RSocket package/import and request method metadata where obvious
- read-only realtime listing through `GET /api/realtime`
- metadata cleanup through normal deleted-file index cleanup

Must not require:
- network connections
- socket server or client startup
- protocol decoding
- payload schema inference
- `npm install`, `node`, package-manager scripts, package registries, `dotnet`,
  restore/build/run/test commands, app code, external APIs, telemetry, or paid
  dependencies

Must defer:
- runtime event discovery and packet capture
- payload schema inference
- backpressure/protocol semantics
- room/group membership semantics beyond obvious literal metadata
- cross-project event producer/consumer matching
- cloud/infrastructure, Go, WPF/XAML, symbolic editing, rename/refactor,
  embeddings, semantic search, and cross-project architecture intelligence

## Messaging / Event-driven Intelligence

Phase 9.2.8 adds basic static messaging/event-driven intelligence.

Must support:
- local package/project detection for AMQP, RabbitMQ, Kafka, Google Pub/Sub,
  generic Pub/Sub hints, and NestJS messaging packages
- RabbitMQ/AMQP literal publish, send-to-queue, consume, exchange, queue, bind,
  routing key, and queue metadata
- Kafka literal producer send, consumer subscribe/run, topic metadata, and
  simple consumer-group hints where visible
- Google Pub/Sub package/import/using hints, topic publisher and subscription
  handler metadata, and basic C# publisher/subscriber callsites
- NestJS `@MessagePattern`, `@EventPattern`, and low-risk ClientProxy
  `emit`/`send` literal pattern metadata
- read-only messaging listing through `GET /api/messaging`
- metadata cleanup through normal deleted-file index cleanup

Must not require:
- Kafka, RabbitMQ, AMQP, Google Pub/Sub, or other broker connections
- broker startup
- Google Cloud API calls or credentials
- runtime topic/queue discovery
- payload schema inference
- `npm install`, `node`, package-manager scripts, package registries, `dotnet`,
  restore/build/run/test commands, app code, external APIs, telemetry, or paid
  dependencies

Must defer:
- runtime broker state discovery
- payload schema and contract intelligence
- schema registry calls
- RabbitMQ exchange binding runtime model
- Kafka partition/runtime semantics
- Google Pub/Sub IAM/project discovery
- cross-project producer/consumer matching
- cloud/infrastructure, Go, scoped indexing targets, WPF/XAML, symbolic editing,
  rename/refactor, embeddings, semantic search, and cross-project architecture
  intelligence

## Cloud / Infrastructure Intelligence

Phase 9.2.9 adds basic static cloud/infrastructure intelligence.

Must support:
- local file detection for Dockerfile, Docker Compose, Kubernetes YAML, and
  Terraform files
- Dockerfile base image, exposed port, environment key, command, and entrypoint
  metadata
- Docker Compose service, image/build context, port, environment key, and
  `depends_on` metadata
- Kubernetes kind, name, namespace, labels, selectors, container names, images,
  ports, Service/Ingress backend hints, and GKE-oriented annotation metadata
- Terraform provider, resource, module, variable, output, resource type, and
  simple literal field metadata
- GCP/GKE classification from visible Terraform `google_*` resource types and
  Kubernetes GKE annotations
- read-only infrastructure listing through `GET /api/infrastructure`
- metadata cleanup through normal deleted-file index cleanup

Must not require:
- Docker daemon, Docker Compose, Kubernetes cluster, Terraform binary, `gcloud`,
  cloud credentials, registries, cloud APIs, network, telemetry, or paid
  dependencies
- `docker`, `kubectl`, `terraform`, `gcloud`, provider/module downloads, app
  code, package managers, or infrastructure command execution

Must defer:
- runtime cloud inventory
- cluster discovery
- Terraform plan/apply behavior
- Helm/Kustomize rendering and CRD semantics
- security scanning
- cost estimation
- policy enforcement
- cross-project deployment/service matching
- WPF/XAML, symbolic editing, rename/refactor, embeddings, semantic search,
  and cross-project architecture intelligence

## Go Language Support

Phase 9.2.10 adds basic static Go language support.

Must support:
- `.go` file detection
- `go.mod` detection for module, require, and replace metadata
- package declaration extraction
- imports, including aliases, blank imports, dot imports, and simple stdlib
  classification
- functions, receiver methods, structs, interfaces, type aliases/basic type
  declarations, and const/var declarations
- conservative same-file local call relationships where names can be matched
  safely
- basic HTTP route hints for `net/http` and simple Gin/Echo/Fiber/Chi router
  calls when visible local router construction makes the framework clear
- read-only route listing through existing `GET /api/routes` when route hints
  are indexed
- metadata cleanup through normal deleted-file index cleanup

Must not require:
- Go toolchain, `go build`, `go test`, `go run`, `go list`,
  `go mod download`, module registry access, package restore, network,
  telemetry, external APIs, paid dependencies, or app execution

Must defer:
- Go compiler/type checking
- dependency resolution from registries
- full interface implementation graphs
- full module workspace analysis
- deep Gin/Echo/Fiber/Chi/gRPC intelligence
- WPF/XAML, symbolic editing, rename/refactor, embeddings, semantic search,
  and cross-project architecture intelligence

## Scoped Indexing + Intelligence Targets

Phase 9.2.11 adds targeted indexing plans for a single local project.

Must support:
- shared `IndexScope` model and structured preview responses
- deterministic parsing for project, path, file, glob, language, framework,
  route, component, module, data access, realtime, messaging, and
  infrastructure scopes
- path/file/glob/language/framework filtering without shell expansion or
  command execution
- target scopes from existing indexed metadata only
- dry-run preview with matched files, samples, languages, frameworks,
  warnings, skipped reasons, and metadata target labels
- scoped index/reindex through the local control CLI and control API
- zero-match scopes as non-fatal results
- full-project indexing as the default when no scope is provided
- scoped reindex preserving unrelated indexed files

Must not require:
- package manager execution, app execution, runtime discovery, external APIs,
  cloud services, database connections, broker connections, cluster access,
  telemetry, paid services, internet access, embeddings, or semantic search

Must defer:
- cross-project scoped indexing
- WPF/XAML/.NET Desktop intelligence
- symbolic editing and rename/refactor
- local embeddings and vector search
- cross-project architecture intelligence

## .NET Desktop / WPF Intelligence

Phase 9.2.12 adds basic static/local .NET Desktop and WPF intelligence.

Must support:
- modern SDK-style WPF project detection from local `.csproj` text
- older .NET Framework WPF project detection from local references and item
  metadata
- XAML Application, Window, UserControl, Page, ResourceDictionary, and
  NavigationWindow detection
- `x:Class`, obvious code-behind path hints, static DataContext hints, and
  ViewModel naming hints
- literal Binding paths, command bindings, CommandParameter paths,
  StaticResource/DynamicResource keys, resource definitions, and
  ResourceDictionary source references
- code-behind partial class and ViewModel hints from local C# source where
  obvious
- read-only metadata through existing symbol storage and `GET /api/wpf`
- scoped indexing compatibility for `language:xaml`, `framework:wpf`, and
  `framework:dotnet_desktop`

Must not require:
- Visual Studio
- MSBuild
- `dotnet restore`, `dotnet build`, `dotnet run`, or `dotnet test`
- Windows runtime
- XAML compiler or designer
- app execution
- external APIs, telemetry, paid services, or internet access

Must defer:
- full WPF binding type checking
- runtime DataContext inference
- designer integration
- deep MVVM framework analysis
- symbolic editing and rename/refactor
- local embeddings, vector search, semantic search, and cross-project
  architecture intelligence

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
- Phase 9.2.7: Realtime / Socket Intelligence
- Phase 9.2.8: Messaging / Event-driven Intelligence
- Phase 9.2.9: Cloud / Infrastructure Intelligence
- Phase 9.2.10: Go Language Support
- Phase 9.2.11: Scoped Indexing + Intelligence Targets
- Phase 9.2.12: .NET Desktop / WPF Intelligence
- Phase 10.0: Local Embeddings + Vector Search Architecture
- Phase 10.1: Local Embedding Provider MVP
- Phase 10.2: SQLite Vector Storage / Search Index
- Phase 10.3: Hybrid Search Ranking
- Phase 10.4: MCP / Control API Integration
- Phase 10.5: Benchmark + Quality Evaluation
- Phase 11.0: Cross-Project Architecture Model + Contracts
- Phase 11.1: Group Query Federation
- Phase 11.1.1: Context Efficiency + Tool Call Reduction Benchmark
- Phase 11.2: Cross-Repo Route / API Matching
- Phase 11.3: Cross-Repo Messaging Matching

Phase 11.1.1 is benchmark/measurement only. It must remain local,
deterministic, offline-first, and free-by-default. It measures chars/4 token
estimates, deterministic workflow tool-call counts, and fixture answer-quality
coverage for file-by-file exploration versus B3-assisted `search_code`,
`semantic_search`, context-pack profiles, and group-summary workflows. It must
not add MCP tools, cross-repo matching, group-level impact/context pack,
service-map APIs, symbolic editing, rename/refactor, telemetry, cloud APIs,
hosted vector databases, paid dependencies, or internet requirements.

Phase 11.2 adds static/read-only cross-repo route/API matching inside local
registry project groups. It may match deterministic HTTP client call keys to
server route keys and produce architecture match candidates, nodes, edges,
confidence, evidence, and warnings. It must preserve one repo-local
`.b3/b3.db` per project and must not merge project DBs globally, execute HTTP
requests, fetch OpenAPI remotely, call external APIs, use cloud graph
databases, require hosted vector databases, infer messaging/package/infra
relationships, add telemetry, or add MCP architecture tools.

Phase 11.3 adds static/read-only cross-repo messaging matching inside local
registry project groups. It may match deterministic producer/consumer messaging
keys for topics, queues, patterns, routing keys, and broker hints, and produce
architecture match candidates, nodes, edges, confidence, evidence, and warnings.
It must preserve one repo-local `.b3/b3.db` per project and must not merge
project DBs globally, connect to brokers, publish or consume messages, call
cloud Pub/Sub APIs, infer package/contract/infra relationships, add telemetry,
or add MCP architecture tools.

Phase 11.4 adds static/read-only cross-repo package/contract/infra matching
inside local registry project groups. It may match deterministic package,
contract/schema, and infrastructure keys from existing local project DBs and
indexed file contents, and produce architecture match candidates, nodes, edges,
confidence, evidence, and warnings. It must preserve one repo-local
`.b3/b3.db` per project and must not merge project DBs globally, run package
managers, restore dependencies, run Docker/Kubernetes/Terraform/cloud CLIs,
fetch remote schemas, validate schema compatibility, call external APIs, add
telemetry, or add MCP architecture tools. Group impact/context packs and service
map APIs remain deferred.

Phase 11.5 adds static/read-only group impact and cross-repo context pack
generation inside local registry project groups. It may resolve local seeds,
traverse existing route/message/dependency match candidates with bounded
depth/limit settings, produce impact nodes, edges, paths, confidence, evidence,
warnings, project summaries, and bounded context packs. It must preserve one
repo-local `.b3/b3.db` per project and must not merge project DBs globally,
execute HTTP requests, connect to brokers, run package managers, run
Docker/Kubernetes/Terraform/cloud CLIs, fetch remote schemas, validate schema
compatibility, call external APIs, add telemetry, or add MCP architecture tools.
Architecture graph/service map APIs and graph UI remain deferred.

Phase 11.6 adds static/read-only architecture graph and service map APIs inside
local registry project groups. It may build bounded graph/service-map responses
on demand from existing route, messaging, package/contract/infra, impact, and
federation metadata; produce project/service/resource nodes, relationship
edges, summaries, confidence, evidence, warnings, unresolved relationship
reports, and service-level dependency summaries. It must preserve one repo-local
`.b3/b3.db` per project, must not persist a global graph by default, and must
not merge project DBs globally, execute HTTP requests, connect to brokers, run
package managers, run Docker/Kubernetes/Terraform/cloud CLIs, fetch remote
schemas, validate schema compatibility, call external APIs, add telemetry, add
MCP architecture tools, or implement graph UI. Cross-project benchmark/docs
expansion remains Phase 11.7.

Phase 11.7 adds local cross-project benchmark coverage and documentation for
Phase 11 architecture intelligence. It is local fixture/local-repo benchmark
coverage only, not a 31 public real-world repository claim. It may benchmark group federation,
route/API matching, messaging matching, package/contract/infra matching, group
impact, cross-repo context packs, architecture graph API, and service map API
through local fixture data and optional local repository candidates. It must
preserve one repo-local `.b3/b3.db` per project, must not persist or require a
global graph, and must not merge project DBs globally, execute HTTP requests,
connect to brokers, run package managers, run Docker/Kubernetes/Terraform/cloud
CLIs, call external APIs, add telemetry, add MCP tools, implement graph UI,
implement symbolic editing, implement rename/refactor, or implement full Git
Intelligence. After switching branches, users should reindex before comparing
results until branch-aware indexing is implemented. Configured optional local
benchmark repositories are optional; missing paths or DBs must warn and skip,
not fail normal builds/tests/benchmarks.

`benchmarks/b3.benchmark.toml` is the default local benchmark configuration
path for broader Phase 11.7 real-local benchmarks and the source of truth for
local benchmark project names/paths. Missing optional projects must produce
warnings, not failures. Normal cargo build/test runs must not require these
paths to exist, must not access the internet or external APIs, must not use
telemetry, hosted vector databases, cloud embeddings, brokers, database servers,
or paid dependencies, and must preserve one repo-local `.b3/b3.db` per project
without merging benchmark databases into a global DB.

Phase 12 adds the Symbolic Editing MVP. It may define editing contracts,
resolve explicit file ranges and indexed symbols, produce dry-run edit previews,
validate bounded single-file edits, apply only with explicit `mode=apply` and
`dry_run=false`, create local backups by default, and expose local Control API
endpoints for preview/apply. It must not implement rename/refactor workflows,
update-all-references, move/extract refactors, broad automatic refactoring, MCP
editing tools by default, UI editing, compiler/formatter execution, package
manager execution, generated code execution, Git Intelligence, cloud services,
external APIs, telemetry, paid dependencies, or internet requirements. After
apply, it should warn that reindex is recommended rather than silently updating
the index.

Phase 13 adds the Rename / Refactor MVP. It may define local rename/refactor
contracts, resolve indexed symbol targets, discover conservative identifier
occurrences from indexed evidence, graph/FTS candidate files, and bounded
same-file or bounded multi-file scans, produce preview-first rename plans,
apply only with explicit `mode=apply` and `dry_run=false`, create backups for
all changed files by default, and expose local Control API preview/apply
endpoints. It must not implement broad automatic refactoring, extract method,
move symbol/module broadly, IDE-grade semantic rename guarantees, compiler or
formatter validation, package manager execution, LSP-required edits, MCP
rename/refactor tools by default, UI editing, Git Intelligence, cloud services,
external APIs, telemetry, paid dependencies, or internet requirements.

Phase 14 adds Additional Backend Language Support. It may statically and locally
detect and index Python, Java, Kotlin, PHP, and Ruby backend/application code,
including basic project metadata, symbols/imports, conservative route/API hints,
data-access hints, and messaging hints where literal evidence is visible. It
must not run package managers, compilers, formatters, runtimes, language
servers, Docker/Kubernetes/Terraform, external APIs, cloud services, telemetry,
or internet access. Support is Basic/static only; compiler-grade semantics and
deep framework analysis remain deferred.

Phase 15 adds Systems / Mobile / Config / Web File Support A. It may statically
and locally detect and index C, C++, Swift, Objective-C, Dart/Flutter, YAML,
JSON, TOML, XML, HTML, CSS/SCSS, XAML hardening, Three.js/WebGL hints, and
ksqlDB hints. It may extract conservative symbols, imports/includes, config key
paths, safe package/dependency names, template/style/asset references,
route/client hints, infrastructure-compatible YAML metadata where existing
parsers apply, and ksqlDB Kafka topic/dependency hints where literal evidence is
visible. It must not run compilers, preprocessors, package managers, formatters,
runtimes, browsers, WebGL, Docker/Kubernetes/Terraform, Kafka, ksqlDB, brokers,
databases, language servers, external APIs, cloud services, telemetry, or
internet access. Secret-like config values must be redacted or skipped; names
and keys are acceptable.

Phase 16 adds Config / Data / Web File Support B / Hardening. It may deepen the
Phase 15 config/data/web extraction with shared secret redaction rules, safe
env-example parsing, key-only/redacted handling for real env files, static env
reference hints, hardened YAML/JSON/TOML/XML metadata, HTML/template local route
and asset hints, CSS/SCSS asset/media/module hints, XAML resource/binding
quality improvements, ksqlDB topic/dependency hints, and basic SQL table/view/
procedure/function/table-reference metadata. It must not read OS environment,
run SQL, connect to databases, Kafka, ksqlDB, RabbitMQ, brokers, browsers,
WebGL, package managers, compilers, formatters, Docker/Kubernetes/Terraform,
external APIs, cloud services, telemetry, or internet access.

Phase 17 completes the Language and Technology Quality Audit. It audits support
level truthfulness, `/api/languages`, `/api/capabilities`, fixture coverage,
metadata consistency, redaction guarantees, false-positive/false-negative
guardrails, cross-surface integration, and benchmark claims. It may add small
local/static hardening fixes only when directly tied to audit findings. It must
not add compiler-grade parsing, runtime validation, architecture graph UI, full
Git Intelligence, broad refactor behavior, cloud/external APIs, telemetry,
package-manager/compiler/runtime execution, mandatory LSP, browser/WebGL
execution, broker/database/Kafka/ksqlDB/RabbitMQ connections, or internet
requirements.

### Roadmap

Completed:

- Phase 11.4: Cross-Repo Package / Contract / Infra Matching
- Phase 11.5: Group-Level Impact + Context Pack
- Phase 11.6: Architecture Graph / Service Map API
- Phase 11.7: Cross-Project Benchmark + Docs
- Phase 12: Symbolic Editing MVP
- Phase 13: Rename / Refactor MVP
- Phase 14: Additional Backend Language Support
- Phase 15: Systems / Mobile / Config / Web File Support A
- Phase 16: Config / Data / Web File Support B / Hardening
- Phase 17: Language and Technology Quality Audit

Current / Next:

- Phase 18: Refactor Checkpoint D

Upcoming:

- Phase 19: Performance Optimization Pass B
- Phase 20: Web UI Developer Console Refresh
- Phase 21: Git Intelligence

## Phase 21.0 Git Intelligence Safety Requirements

Phase 21.0 is completed as design and safety planning only. Git Intelligence
must remain local-only, read-only by default, offline/free, and safe for no-git
projects, dirty worktrees, detached HEAD, subdirectories inside repositories,
submodules, worktrees, and incomplete repositories.

Future Git Intelligence may read local `.git` metadata or run bounded
read-only local Git commands, but it must not run mutating or remote commands:
no checkout, switch, commit, merge, rebase, reset, clean, push, pull, fetch,
branch/tag/ref mutation, auto-stash, auto-reindex on branch switch, working-tree
edits, GitHub/GitLab/Bitbucket APIs, telemetry, cloud services, package
managers, Docker/Kubernetes/Terraform, brokers/databases, mandatory LSP, paid
dependencies, or internet access.

Phase 21.1 is completed as local Git status detection only. It adds internal
contracts and a dedicated local reader for repository root, `.git` directory,
current branch or detached HEAD, HEAD commit, and basic porcelain status counts
for staged, unstaged, untracked, conflicted, dirty, and total changed paths.
The reader uses bounded read-only local Git commands, returns warnings when Git
is unavailable or status cannot be read, and remains safe for no-git projects.

Phase 21.2 is completed as branch-aware index metadata only. B3 records a
read-only Git index snapshot at index time when possible, including no-git
state, repository root, `.git` directory, indexed branch/commit, short commit,
detached HEAD, dirty state, staged/unstaged/untracked/conflicted/total changed
counts, timestamp, and warnings. A minimal local SQLite migration stores this
metadata by project and branch.

Phase 21.3 is completed as stale index detection and conservative auto-index
policy evaluation only. It compares current read-only Git status with the latest
indexed Git snapshot and classifies freshness as Fresh, Dirty, Stale, Unsafe,
or Unknown. It produces reindex recommendations and manual-action requirements.

Auto full reindex must never run on branch or commit changes. The default
auto-index policy is disabled, and conservative incremental changed-file mode
is only allowed by policy when branch and commit match, Git state is known, no
conflicts exist, changed count is bounded, and changed-file details are
available. Phase 21.3 does not execute auto-index because changed-file detail is
deferred to Phase 21.4.

Phase 21.4 is completed as local read-only changed-file and diff-summary
support only. It parses bounded `git status --porcelain=v1 -z --branch` output
for changed file paths and classifications, and bounded `git diff --numstat`
plus `git diff --cached --numstat` output for line-count summaries. It does not
read file contents or full patches.

Changed-file details now inform conservative auto-index policy evaluation, but
auto-index execution remains disabled/deferred. Truncated output, conflicts,
deleted/renamed/copied/type-changed/unknown statuses unless explicitly allowed,
branch changes, commit changes, detached HEAD, no-git state, and unknown Git
state block auto-index.

Phase 21.4 does not add schema migrations, Control API endpoints, MCP tools,
MCP profile changes, Web UI panels, diff-aware impact, branch comparison,
auto-reindex execution, Git mutation, or remote APIs. Phase 21.5 is next for
diff-aware impact analysis.

Manual reindex actions are planned later: preview reindex, reindex current
branch, and reindex changed files only. Any future auto-index toggle must be
off or conservative by default and must never run on branch change, commit
change, detached HEAD, conflicts, unknown Git state, no-git projects, excessive
changed files, unsafe delete/rename batches, indexed branch mismatch, or
indexed commit mismatch.
