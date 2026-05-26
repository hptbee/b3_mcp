# Agent Instructions

## Identity

This repository builds a high-performance local-first MCP code intelligence platform for Cursor and Codex.

It combines:
- TokenSave-style token-saving semantic graph queries
- Codebase Memory MCP-style persistent tree-sitter knowledge graphs

## Offline-First Requirement

This platform must remain fully functional offline.

Never introduce required dependencies on:
- OpenAI APIs
- Anthropic APIs
- hosted vector databases
- cloud auth providers
- remote telemetry systems

External services must always be:
- optional
- plugin-based
- disabled by default

The default installation must work entirely offline.

## Keep Stack

Backend:
- Rust
- Tokio
- Axum
- rmcp
- tree-sitter
- SQLite/libSQL local DB with WAL
- Qdrant local mode
- DashMap
- tracing

Frontend:
- Next.js
- TypeScript
- React Flow
- WebSocket

Packaging:
- Tauri optional

## Architecture

Use hybrid monolith:
- lightweight MCP runtime
- shared core engine
- async worker pipelines
- optional control server
- optional localhost UI

MCP runtime must only handle:
- stdio transport
- JSON-RPC
- tool routing
- static local tool profile filtering
- concise tools/list manifest generation
- streaming
- cancellation
- session lifecycle

Never put these in MCP hot path:
- full indexing
- embedding generation
- unbounded graph traversal
- blocking IO
- large filesystem scans

## Product Priorities

1. reduce agent tokens
2. reduce grep/glob/read-file calls
3. provide instant graph queries
4. preserve memory across sessions
5. support Cursor and Codex
6. expose localhost UI for control/config/graph
7. make local agent setup explicit, dry-run-first, and reversible

## MCP Tool Profiles

Default MCP runtime profile is `optimized`.

Use:
- `tiny` for the smallest high-value manifest
- `optimized` for normal agent work
- `full` or `debug` when trace-heavy tools are needed
- `readonly` when future mutation tools must be hidden
- `editing` only for future symbolic editing tools
- `web-app` for common web application workflows
- `enterprise` for future graph/impact/multi-service workflows

Profiles are static local configuration only. Do not add installer hooks,
command execution, registry behavior, cloud services, telemetry, embeddings,
LSP, language packs, session memory, or symbolic editing as part of profile
filtering.

## Agent Install Helper

The `b3` helper may generate or update local Codex/Cursor MCP config files.
Install behavior must be dry-run-first, preserve unrelated config, avoid
duplicate server entries, and create backups before writes.

It also owns local registry commands for project and group metadata. Registry
behavior must stay metadata-only:

- default path `~/.b3/registry.json`
- `B3_HOME` or `--registry` can redirect tests/smoke runs
- one project keeps one repo-local `.b3/b3.db`
- no automatic filesystem scans
- no cross-project query execution
- no graph merging
- no architecture intelligence

## Language Backend Architecture

Language backend contracts live in `b3-core`. Indexer implementations or
adapters live in `b3-indexer`; control may report capabilities but must not own
parser logic.

Phase 9.0 support must stay honest:

- Rust is available through tree-sitter metadata and the existing parser path.
- Planned languages can be detected locally but must not claim parser, LSP,
  framework, route, or semantic capabilities.
- LSP remains disabled until a later local/free LSP backend is implemented.
- Detection must not shell out, call external APIs, or require internet access.

Hook integration is foundation only. Hooks must stay disabled by default:

- no automatic command interception
- no shell proxy
- no terminal capture
- no telemetry
- no shell profile modification
- no background daemon watching user commands

## Apply These Algorithms

Use:
- incremental hash indexing
- tree-sitter AST extraction
- FTS5/BM25 lexical search
- vector semantic search
- graph expansion with depth limits
- PageRank/centrality for importance
- community detection for module maps
- impact analysis through callers/callees/references/tests
- token savings ledger
- crash-isolated parser workers
- branch-aware indexing

## Avoid

- semantic-only search
- full-file dumps
- unbounded result lists
- Neo4j dependency
- Electron
- Python hot paths
- microservices
- global mutable state
- sync mutexes in hot paths

## LSP Backend Boundaries

LSP complements the tree-sitter graph; it does not replace indexing, SQLite graph storage, FTS, query ranking, context pack generation, or Rust parser behavior.

LSP must remain:
- local-only
- disabled by default
- free-by-default
- configured explicitly
- non-fatal when a language server is missing

Do not install or download language servers. Do not add cloud, telemetry, paid/proprietary, symbolic editing, rename/refactor, semantic search, embeddings, or cross-project architecture intelligence while working in Phase 9.1.

## Web Language Boundaries

Phase 9.2 web language support is basic indexing only. JavaScript, TypeScript, JSX, and TSX may use bundled local tree-sitter parsers for symbols/imports, but indexing must not run `npm`, `node`, `tsc`, `eslint`, cloud parsers, or framework CLIs.

Do not add Node.js REST route intelligence, React hook/component graph intelligence, Angular route/template/module intelligence, C# semantic intelligence, JS/TS symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.2.

## Node.js REST Boundaries

Phase 9.2.1 Node.js REST intelligence is completed as basic local static analysis only. It may detect package.json dependencies and high-confidence Express, NestJS, and Fastify route declarations, but it must not execute `node`, `npm`, `tsc`, `eslint`, Nest CLI, app code, package-manager scripts, package registries, or cloud parsers.

Do not add React graph intelligence, Angular intelligence, ASP.NET Core/C# intelligence, Go support, realtime/socket intelligence, messaging intelligence, cloud/infrastructure intelligence, symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.2.1.

## React / TSX Boundaries

Phase 9.2.2 React / TSX component intelligence is completed as basic local static analysis only. It may detect common React components, props type names, JSX component usages, and hook names, but it must not execute `node`, `npm`, `tsc`, `eslint`, React dev servers, app code, package-manager scripts, package registries, or cloud parsers.

Do not add Angular intelligence, Vue/Svelte intelligence, ASP.NET Core/C# intelligence, Go support, realtime/socket intelligence, messaging intelligence, cloud/infrastructure intelligence, symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.2.2.

## Next.js Intelligence Boundaries

Phase 9.2.3 Next.js intelligence is completed as basic local static analysis
only on top of the completed React / TSX support. It may inspect local
`package.json`, `next.config.*`, `app/`, and `pages/` files, detect common
file-system routes and app route-handler method exports, and mark basic
`"use client"` / `"use server"` boundaries, but it must not run `next dev`,
`next build`, `node`, `npm`, `tsc`, `eslint`, package scripts, deployment
tooling, package registries, cloud parsers, or app code. It does not implement
full RSC semantics, middleware execution order, Vercel/deployment intelligence,
auth intelligence, or deep data fetching semantics.

## Angular Intelligence Boundaries

Phase 9.2.4 Angular intelligence is completed as basic local static analysis
only on top of TypeScript support. It may inspect local `package.json`,
`angular.json`, TypeScript decorators, route config object literals, and
literal template/style references, but it must not run `ng`, Angular compiler,
`node`, `npm`, `tsc`, `eslint`, package scripts, package registries, cloud
parsers, or app code. It does not implement full template type checking,
runtime lifecycle semantics, deep DI/module graph resolution, RxJS/NgRx flow,
or Angular Material intelligence.

## ASP.NET Core / C# Web API Boundaries

Phase 9.2.5 ASP.NET Core / C# Web API intelligence is completed as basic local
static analysis only. It may inspect local `.cs` and `.csproj` files, detect
ASP.NET Core project references, controller classes, `[ApiController]`,
`[Route]`, common HTTP method attributes, action methods, composed
controller/action routes, and constructor DI type names, but it must not run
`dotnet`, restore packages, call NuGet, require Roslyn, launch Visual Studio,
Rider, OmniSharp, or C# language servers, run app code, or use cloud parsers.
It does not implement full semantic C# analysis, full DI container graphs,
middleware pipeline analysis, minimal API intelligence, WPF/XAML, or .NET
desktop intelligence.

## ORM / Database Access Boundaries

Phase 9.2.6 ORM / Database Access intelligence is completed as basic local
static analysis only. It may detect EF Core, Dapper, Prisma, TypeORM, and
Sequelize packages/imports/usings, EF Core DbContext/DbSet declarations,
obvious query/execute callsites, model/entity/context names, operations, and
direct literal SQL snippets where visible, but it must not connect to databases,
execute SQL, run migrations, run `dotnet`, `node`, `npm`, Prisma generate,
TypeORM CLI, Sequelize CLI, package registries, app code, or cloud parsers. It
does not implement full SQL parsing, full LINQ semantics, full TypeScript/C#
type checking, runtime DB behavior, schema introspection, or cross-project data
lineage.

Do not start Realtime / Socket Intelligence until Phase 9.2.7.

## Realtime / Socket Boundaries

Phase 9.2.7 Realtime / Socket intelligence is completed as basic local static
analysis only. It may detect common WebSocket, Socket.IO, SignalR, and minimal
RSocket package/import/project hints, constructors, listeners, emitters, hub
classes, hub methods, client invocations, and obvious literal event/channel/hub
metadata, but it must not open network connections, start socket servers, run
`node`, `npm`, `dotnet`, framework CLIs, package scripts, package registries, or
app code. It does not implement runtime event flow, protocol decoding, payload
schema inference, auth negotiation analysis, broker/messaging intelligence,
room/group semantics beyond obvious metadata, or cross-project event matching.

## Messaging / Event-driven Boundaries

Phase 9.2.8 Messaging / Event-driven intelligence is completed as basic local
static analysis only. It may detect common AMQP, RabbitMQ, Kafka, Google
Pub/Sub, generic Pub/Sub, and NestJS messaging package/import/project hints,
producer/consumer callsites, literal topics, queues, exchanges, routing keys,
consumer groups, and message/event patterns, but it must not connect to brokers,
start brokers, call cloud APIs, use cloud credentials, run `node`, `npm`,
`dotnet`, framework CLIs, package scripts, package registries, or app code. It
does not implement runtime broker state discovery, payload schema inference,
message contract intelligence, schema registry calls, RabbitMQ binding runtime
models, Kafka partition/runtime semantics, Google Pub/Sub IAM/project discovery,
or cross-project producer/consumer matching.

## Cloud / Infrastructure Boundaries

Phase 9.2.9 Cloud / Infrastructure intelligence is completed as basic local
static analysis only. It may detect Dockerfile, Docker Compose, Kubernetes
YAML, Terraform, and visible GCP/GKE hints, including obvious images, services,
workloads, ports, environment keys, providers, resources, modules, variables,
and outputs, but it must not run Docker, Docker Compose, `kubectl`, Terraform,
`gcloud`, provider/module downloads, registry calls, cloud APIs, cloud
credentials, package managers, or app code. It does not implement runtime cloud
inventory, cluster discovery, Terraform plan/apply behavior, Helm/Kustomize
rendering, CRD semantic expansion, security scanning, cost estimation, policy
enforcement, or cross-project deployment matching.

## Go Language Boundaries

Phase 9.2.10 Go language support is completed as basic local static analysis
only. It may inspect local `.go`, `go.mod`, `go.sum`, and `go.work` files,
detect packages, imports, functions, receiver methods, structs, interfaces,
type declarations, const/var declarations, conservative same-file call edges,
and basic HTTP route hints for `net/http` plus simple Gin/Echo/Fiber/Chi router
calls when local router construction is visible, but it must not run `go`,
`go build`, `go test`, `go run`, `go list`, `go mod download`, module registry
access, package restore, app code, compiler/type checking, or external parsers.
It does not implement full semantic Go analysis, dependency resolution, full
interface implementation graphs, deep framework intelligence, gRPC
intelligence, symbolic editing, rename/refactor, embeddings, semantic search,
or cross-project architecture intelligence.

## Scoped Indexing Boundaries

Phase 9.2.11 scoped indexing is completed as local static target planning only.
It may parse explicit scope strings, validate local paths under the project
root, preview matched files, filter path/file/glob/language/framework scopes,
and use existing indexed route/component/data-access/realtime/messaging/
infrastructure metadata to select source files. Full-project indexing remains
the default when no scope is provided.

It must not broaden invalid scopes to the whole project, execute commands,
expand shell globs, run package managers or app code, connect to databases,
brokers, clusters, cloud APIs, or external services, add embeddings or semantic
search, perform symbolic editing or rename/refactor, or do cross-project scoped
indexing/architecture matching.

## .NET Desktop / WPF Boundaries

Phase 9.2.12 .NET Desktop / WPF intelligence is completed as basic local static
analysis only. It may inspect local `.csproj`, `.xaml`, and `.xaml.cs`/`.cs`
files, detect modern SDK-style WPF and older .NET Framework WPF project hints,
classify obvious XAML Application, Window, UserControl, Page,
ResourceDictionary, and NavigationWindow roots, extract `x:Class`, code-behind
path hints, static DataContext hints, ViewModel naming hints, Binding paths,
Command bindings, StaticResource/DynamicResource references, and
ResourceDictionary sources.

It must not run Visual Studio, MSBuild, `dotnet`, WPF applications, XAML
compilers, designers, package restore, app code, external APIs, telemetry, or
internet access. It does not implement full WPF binding type checking, runtime
DataContext inference, deep MVVM framework analysis, symbolic editing,
rename/refactor, embeddings, semantic search, or cross-project architecture
intelligence.

## Local Embeddings / Vector Architecture Boundaries

Phase 10.0 is completed as local architecture foundation only. It may define
embedding provider contracts, deterministic test-provider behavior for tests,
embedding/vector configuration defaults, vector document/chunk/source models,
deterministic chunk planning, vector store traits, normal SQLite vector tables,
read-only control status/stats endpoints, docs, and tests.

It must not implement real model downloads, OpenAI/cloud embedding providers,
hosted vector databases, required Qdrant/Pinecone/Weaviate integrations,
telemetry, SaaS auth, API-key requirements, full semantic search, hybrid
ranking, MCP semantic search tools, symbolic editing, rename/refactor, session
memory, or cross-project architecture intelligence.

Phase 10.1 is completed as a local embedding provider MVP only. It may provide
the deterministic `local_hash` lexical/hash embedding provider, provider
registry/config integration, batch chunk embedding, vector normalization and
similarity helpers, and read-only control provider/status metadata.

It must not implement OpenAI/cloud embedding providers, hosted vector database
integrations, model downloads, telemetry, API-key requirements, full semantic
search, hybrid ranking, MCP semantic search tools, cross-project semantic
search, symbolic editing, rename/refactor, session memory, or cross-project
architecture intelligence.

Phase 10.2 is completed as SQLite vector storage/search only. It may provide
durable vector document and embedding vector tables, deterministic upsert/
dedupe, cleanup by file/project/branch, little-endian `Vec<f32>` BLOB encoding,
metadata-filtered local cosine search in Rust, vector stats, and read-only
control status/stats updates.

It must not implement hybrid ranking, final semantic search ranking, MCP
semantic search tools, hosted vector database integrations, native SQLite vector
extensions as required dependencies, OpenAI/cloud embedding providers, model
downloads, telemetry, API-key requirements, cross-project semantic search,
symbolic editing, rename/refactor, session memory, or cross-project
architecture intelligence.

Phase 10.3 is completed as hybrid ranking only. It may provide reusable
`b3-query` ranking that combines lexical overlap, local SQLite vector cosine
scores, metadata boosts, deterministic score normalization/tie-breaking, and
compact explanations.

It must not implement MCP semantic search tools, final semantic search UX,
benchmark/quality datasets, hosted vector database integrations, OpenAI/cloud
embedding providers, model downloads, telemetry, API-key requirements,
cross-project semantic search, symbolic editing, rename/refactor, session
memory, or cross-project architecture intelligence.

Phase 10.4 is completed as MCP / Control API integration only. It may expose
local hybrid search through `POST /api/search/hybrid` and the MCP
`semantic_search` tool, update read-only capability/status reporting, validate
requests, and return compact explanations and lexical/metadata fallback
warnings.

It must not implement benchmark/quality datasets, neural embedding providers,
hosted vector database integrations, OpenAI/cloud embedding providers, model
downloads, telemetry, API-key requirements, cross-project semantic search,
symbolic editing, rename/refactor, session memory, or cross-project
architecture intelligence.

Phase 10.5 is completed as benchmark + quality evaluation only. It may add
local deterministic benchmark fixtures, query expectations, lexical/vector/
hybrid comparison metrics, latency measurements, token/context estimates,
structured benchmark JSON, and regression guardrail tests.

It must not implement cross-project architecture intelligence, symbolic editing,
rename/refactor, neural embedding providers, hosted vector database
integrations, OpenAI/cloud embedding providers, model downloads, telemetry,
API-key requirements, benchmark upload, session memory, or UI redesign.
