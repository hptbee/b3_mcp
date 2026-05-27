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

## Additional Backend Language Boundaries

Phase 14 Additional Backend Language Support is completed as basic local static
analysis only. It may inspect Python, Java, Kotlin, PHP, and Ruby source and
project metadata files, extract conservative symbols/imports, route/API hints,
data-access hints, and messaging hints where literal evidence is visible, and
emit existing B3 symbol/route/data-access/messaging metadata.

It must not run pip, poetry, uv, Maven, Gradle, composer, bundle, compilers,
formatters, runtimes, language servers, Docker/Kubernetes/Terraform, external
APIs, cloud services, telemetry, package registries, or internet access. It
does not implement compiler-grade semantics, deep framework intelligence,
systems/mobile language support, config/data/web file support, quality audit,
architecture graph UI, full Git Intelligence, or broad refactor behavior.

## Systems / Mobile / Config / Web File Boundaries

Phase 15 Systems / Mobile / Config / Web File Support A is completed as basic
local static analysis only. It may inspect C, C++, Swift, Objective-C,
Dart/Flutter, YAML, JSON, TOML, XML, HTML, CSS/SCSS, XAML, JavaScript/TypeScript
Three.js/WebGL usage, and ksqlDB files for conservative symbols,
imports/includes, config key paths, safe package/dependency names,
template/style/asset references, route/client hints, XAML metadata hints, and
ksqlDB Kafka topic/dependency hints where literal evidence is visible.

It must not run clang, gcc, CMake, make, xcodebuild, swift, dart, flutter, npm,
package managers, compilers, preprocessors, formatters, runtimes, browsers,
WebGL, Docker/Kubernetes/Terraform, Kafka, ksqlDB, brokers, databases, language
servers, external APIs, cloud services, telemetry, or internet access.
Secret-like config values must be redacted or skipped; names and keys are
acceptable. It does not implement compiler-grade systems/mobile semantics,
browser/runtime analysis, SQL validation, schema compatibility, architecture
graph UI, Phase 17 quality audit, full Git Intelligence, or broad refactor
behavior.

Phase 16 Config / Data / Web File Support B / Hardening is completed as basic
local static hardening only. It may improve secret redaction, safe env-example
parsing, key-only/redacted real env file handling, static config reference
hints, YAML/JSON/TOML/XML metadata, HTML/template route/asset hints, CSS/SCSS
media/import/asset hints, XAML resource/binding quality, ksqlDB topic and
dependency hints, SQL table-reference metadata, and Three.js/WebGL asset hints.

It must not read OS environment variables, run SQL, connect to databases,
Kafka, ksqlDB, RabbitMQ, brokers, browsers, WebGL runtimes,
Docker/Kubernetes/Terraform, package managers, compilers, formatters, language
servers, external APIs, cloud services, telemetry, or internet access. It does
not implement architecture graph UI, full Git Intelligence, broad refactor
behavior, runtime validation, schema compatibility validation, or advanced
messaging intelligence.

Phase 17 Language and Technology Quality Audit is completed as a local/static
truthfulness and regression pass. It may audit and align support levels,
`/api/languages`, `/api/capabilities`, fixture/test coverage, metadata key
consistency, redaction guarantees, false-positive guardrails, cross-surface
integration, benchmark claims, and docs. Small hardening is allowed only when
directly tied to an audit finding. It must not implement Phase 18 refactor
checkpoint, Phase 19 performance work, architecture graph UI, full Git
Intelligence, broad refactor behavior, RabbitMQ advanced messaging
implementation, cloud services, hosted vector/graph databases, telemetry,
external APIs, paid dependencies, package-manager/compiler/runtime execution,
browser/WebGL execution, Docker/Kubernetes/Terraform execution,
Kafka/ksqlDB/RabbitMQ/broker/database connections, or mandatory language
servers.

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

Phase 11.0 is completed as cross-project architecture model/contracts only. It
may define local project/group identity, service identity, architecture
node/edge contracts, future match candidate contracts, confidence, evidence,
source/provenance metadata, deterministic normalization helpers, and read-only
architecture capability/status reporting.

It must not implement group query federation, cross-repo route/API matching,
cross-repo messaging matching, package/contract/infra matching, group-level
impact analysis, architecture graph UI, service map APIs, global database
merges, cloud graph databases, hosted vector databases, external APIs,
telemetry, symbolic editing, rename/refactor, session memory, or cross-project
architecture intelligence beyond contracts.

Phase 11.1 is completed as local group query federation only. It may resolve
registry-defined project groups, open existing project DBs read-only, return
per-project statuses and warnings, aggregate metadata summaries, and expose
read-only local control endpoints for group list/status/summary.

It must not implement cross-repo route/API matching, cross-repo messaging
matching, package/contract/infra matching, group-level impact analysis,
architecture graph UI, service map APIs, global database merges, cloud graph
databases, hosted vector databases, external APIs, telemetry, symbolic editing,
rename/refactor, session memory, or cross-project relationship inference.

Phase 11.1.1 is completed as context efficiency and tool-call reduction
benchmarking only. It may define deterministic local file-by-file and
B3-assisted workflow baselines, context profiles, chars/4 token estimates,
modeled tool-call counts, fixture answer-quality scoring, target comparisons,
benchmark JSON output, tests, and docs.

It must not implement cross-repo route/API matching, cross-repo messaging
matching, package/contract/infra matching, group-level impact/context pack,
architecture graph UI, service map APIs, symbolic editing, rename/refactor,
neural embedding providers, hosted vector databases, cloud embeddings, external
APIs, telemetry, paid dependencies, model downloads, internet requirements, or
cross-project relationship inference.

Phase 11.2 is completed as cross-repo route/API matching only. It may match
conservative static HTTP client call literals to existing server route metadata
inside a registry project group, produce `CallsHttpRoute` match candidates,
architecture nodes/edges, confidence, evidence, warnings, and expose a
read-only local Control API endpoint.

It must not implement cross-repo messaging matching, package/contract/infra
matching, group-level impact/context pack, architecture graph UI, service map
APIs, symbolic editing, rename/refactor, runtime HTTP calls, remote OpenAPI
fetching, DNS/service discovery, global database merges, cloud graph databases,
hosted vector databases, external APIs, telemetry, paid dependencies, model
downloads, or internet requirements.

Phase 11.3 is completed as cross-repo messaging matching only. It may match
existing local producer/consumer messaging metadata inside a registry project
group, produce `PublishesMessage` match candidates, architecture nodes/edges,
confidence, evidence, warnings, and expose a read-only local Control API
endpoint.

It must not implement package/contract/infra matching, group-level
impact/context pack, architecture graph UI, service map APIs, symbolic editing,
rename/refactor, broker connections, runtime publish/consume operations, cloud
Pub/Sub API calls, global database merges, cloud graph databases, hosted vector
databases, external APIs, telemetry, paid dependencies, model downloads, or
internet requirements.

Phase 11.4 is completed as cross-repo package/contract/infra matching only. It
may match deterministic local package/dependency, contract/schema, and
infrastructure keys inside a registry project group, produce match candidates,
architecture nodes/edges, confidence, evidence, warnings, and expose a read-only
local Control API endpoint.

It must not implement group-level impact/context pack, architecture graph UI,
service map APIs, symbolic editing, rename/refactor, package manager execution,
dependency restore, Docker/Kubernetes/Terraform/cloud CLI execution, remote
schema fetching, schema compatibility validation, global database merges, cloud
graph databases, hosted vector databases, external APIs, telemetry, paid
dependencies, model downloads, or internet requirements.

Phase 11.5 is completed as group-level impact and cross-repo context pack only.
It may resolve local seeds, traverse existing route/message/dependency match
candidates inside a registry project group, produce impact nodes/edges/paths,
confidence, evidence, warnings, bounded context packs, and expose a read-only
local Control API endpoint.

It must not implement architecture graph UI, service map APIs, Phase 11.7
benchmark/docs expansion, symbolic editing, rename/refactor, package manager
execution, Docker/Kubernetes/Terraform/cloud CLI execution, runtime HTTP calls,
broker connections, global database merges, cloud graph databases, hosted vector
databases, external APIs, telemetry, paid dependencies, model downloads, or
internet requirements.

Phase 11.6 is completed as architecture graph / service map API only. It may
build bounded local graph and service-map responses on demand from existing
route/message/dependency match candidates, group impact/context metadata where
useful, and group federation summaries. It may expose read-only local Control
API endpoints for graph and service-map responses.

It must not implement architecture graph UI, Phase 11.7 benchmark/docs
expansion, symbolic editing, rename/refactor, package manager execution,
Docker/Kubernetes/Terraform/cloud CLI execution, runtime HTTP calls, broker
connections, global database merges, persisted global architecture graphs, cloud
graph databases, hosted vector databases, external APIs, telemetry, paid
dependencies, model downloads, or internet requirements.

Phase 11.7 is completed as cross-project benchmark + docs only. It may add
local benchmark coverage for Phase 11 federation, matching, group impact,
context packs, architecture graph API, and service map API; parse
`benchmarks/b3.benchmark.toml`; report optional local project warnings; write
benchmark JSON; and document current capability/limitations.
It is local fixture/local-repo benchmark coverage only, not a 31 public
real-world repository claim. After switching branches, users should reindex
before comparing results until branch-aware indexing is implemented.

It must not implement architecture graph UI, symbolic editing, rename/refactor,
full Git Intelligence, package manager execution, Docker/Kubernetes/Terraform/
cloud CLI execution, runtime HTTP calls, broker connections, global database
merges, persisted global architecture graphs, cloud graph databases, hosted
vector databases, cloud embeddings, external APIs, telemetry, paid
dependencies, model downloads, or internet requirements. Optional local
benchmark repositories configured in `benchmarks/b3.benchmark.toml` must remain
optional.

Phase 12 is completed as Symbolic Editing MVP only. It may define local edit
contracts, resolve explicit file ranges and indexed symbols, preview bounded
single-file edits, validate safety constraints, apply only with explicit
`mode=apply` and `dry_run=false`, create local backups by default, emit
unified-diff-style patches, and expose local Control API preview/apply
endpoints. It must not implement rename/refactor, update-all-references, broad
automatic refactoring, multi-file rename workflows, MCP editing tools, UI
editing, architecture graph UI, full Git Intelligence, formatter/compiler
execution, package manager execution, generated code execution, cloud services,
external APIs, telemetry, paid dependencies, model downloads, or internet
requirements. Reindex after apply is recommended, not hidden or automatic.

Phase 13 is completed as Rename / Refactor MVP only. It may define local
rename/refactor contracts, resolve indexed symbol targets, discover conservative
identifier occurrences from indexed evidence, graph/FTS candidate files, and
bounded scans, preview bounded rename plans, apply only with explicit
`mode=apply` and `dry_run=false`, create backups for all changed files by
default, emit patch output, and expose local Control API rename preview/apply
endpoints. It must not implement broad automatic refactoring, extract method,
move symbol/module broadly, IDE-grade semantic rename guarantees, mandatory LSP
edits, MCP rename/refactor tools, UI editing, compiler/formatter execution,
package manager execution, generated code execution, architecture graph UI,
full Git Intelligence, cloud services, external APIs, telemetry, paid
dependencies, model downloads, or internet requirements. Reindex after apply is
recommended, not hidden or automatic.

## Local Benchmark Config Boundaries

`benchmarks/b3.benchmark.toml` is the default local benchmark config path and
the source of truth for local benchmark project names/paths. General docs must
not hardcode private/local benchmark project names. Missing configured local
benchmark repositories or DBs must warn, not fail, and `cargo test --workspace`
must pass on machines without those optional local repositories.

Use `scripts/setup-local-benchmark.ps1` only as an explicit local helper for
available optional benchmark candidates. It reads enabled `local_repo` projects
from `benchmarks/b3.benchmark.toml`, may create repo-local `.b3` directories,
and may index available projects with missing databases into their configured
`.b3\b3.db` files. Those projects remain optional and must never be required for
normal build, test, or CI success.

Benchmark config handling must remain offline-first and free-by-default: no
internet access, external APIs, telemetry, hosted vector databases, cloud
embeddings, brokers, database servers, paid dependencies, or global database
merge. Each benchmark project keeps its own repo-local `.b3/b3.db`.

## Git Intelligence Boundaries

Phase 21.3 Stale Index Detection is completed as local read-only freshness and
policy evaluation. Git Intelligence must remain local-only,
read-only by default, deterministic where possible, and safe for no-git
projects, dirty worktrees, detached HEAD, subdirectories inside a Git repo,
submodules, worktrees, and incomplete repositories.

Future implementation may read `.git` metadata or run bounded read-only local
Git commands when available. It must not run checkout, switch, commit, merge,
rebase, reset, clean, push, pull, fetch, branch/tag/ref mutation, auto-stash,
auto-reindex on branch switch, write `.git`, modify working-tree files, call
GitHub/GitLab/Bitbucket or other remote APIs, add telemetry, require internet,
or require paid/cloud dependencies.

Phase 21.3 adds `GitIndexFreshness` and conservative `AutoIndexPolicy`
evaluation. It does not add Control API endpoints, MCP tools, MCP profile
changes, Web UI panels, schema migrations, changed-file diff summaries,
diff-aware impact, branch comparison, auto-reindex execution, or Git mutation.
Phase 21.4 is next for changed files and diff summary.

Manual reindex actions remain future work. Any future auto-index toggle must be
off or conservative by default and must never run on branch change, commit
change, detached HEAD, conflicts, unknown Git state, no-git projects, excessive
changed files, unsafe delete/rename batches, indexed branch mismatch, or
indexed commit mismatch.
