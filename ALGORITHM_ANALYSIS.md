# Algorithm Analysis and Improvements

## Ideas Taken From TokenSave

TokenSave focuses on replacing agent exploration with compact local graph queries.

Strategies adopted:
- pre-indexed semantic knowledge graph
- FTS5/local database lookup
- one-call smart context building
- semantic search by meaning
- impact analysis through callers/callees
- file watcher / daemon mode
- branch-aware indexing
- cross-session memory
- token savings ledger
- guardrails to reduce wasteful grep/read-file exploration
- atomic edit primitives with re-index after writes

## Ideas Taken From Codebase Memory MCP

Codebase Memory MCP focuses on persistent tree-sitter knowledge graphs for MCP clients.

Strategies adopted:
- tree-sitter based parsing
- persistent knowledge graph
- multi-phase indexing pipeline
- parallel worker pools
- call-graph traversal
- impact analysis
- community discovery / graph clustering
- graph-native query support

## Algorithms To Apply

### Incremental File Hash Indexing

Use content hashes to skip unchanged files.

Algorithm:
1. walk repo with ignore rules
2. compute file hash
3. compare against stored hash
4. only parse changed/deleted/new files
5. update affected graph nodes, edges, FTS entries, and caches

Improvement:
- store per-symbol hash
- avoid rewriting unchanged symbol nodes

### Tree-Sitter AST Symbol Extraction

Use tree-sitter queries per language.

Extract:
- functions
- methods
- classes
- structs
- interfaces
- enums
- modules
- imports
- routes
- tests
- config keys

Improvement:
- add language packs as plugins
- support fallback text extraction for unsupported languages

### Local LSP Backend

Use local language servers as a semantic capability layer alongside the tree-sitter graph.

Phase 9.1 scope:
1. launch only explicitly configured local stdio server binaries
2. initialize a workspace with bounded startup/request timeouts
3. sync documents with full-text `didOpen`/`didChange`
4. parse capabilities and diagnostics
5. request definition, references, and implementations when supported
6. keep missing servers non-fatal

LSP does not replace tree-sitter indexing, graph persistence, FTS/BM25, context packing, or Rust parser behavior. Symbolic editing and rename/refactor remain deferred.

### Basic Web Language Indexing

Phase 9.2 adds tree-sitter parsing for JavaScript, TypeScript, JSX, and TSX.

Extraction is intentionally conservative:
1. create a file-level module symbol for each parsed web file
2. extract high-confidence declarations such as functions, classes, methods, exported constants, interfaces, type aliases, and enums
3. extract ESM and CommonJS import specifiers as package symbols
4. emit `CONTAINS` and `IMPORTS` relationships only
5. skip JS/TS call graph extraction until name resolution is reliable enough

External packages remain package symbols, not fake file nodes. Relative import resolution supports common JS/TS extensions and `index.*`, but tsconfig aliases and framework route/template graphs are deferred.

### Node.js REST Route Extraction

Phase 9.2.1 adds conservative static route extraction for Node.js backends.

Implemented extraction rules:
1. detect REST-relevant packages from local `package.json` dependency sections
2. extract direct Express route calls and router calls from tree-sitter call expressions
3. extract NestJS controller routes from decorator text and compose controller/method paths
4. extract basic Fastify shorthand and route-object calls
5. store each confident route as a `Route` symbol with encoded method/path/framework metadata
6. emit a route-to-handler `REFERENCES` edge only when the handler or controller method symbol is resolvable

The extractor intentionally avoids runtime middleware order, Nest module graphs,
guards/interceptors/pipes, deep DI resolution, request lifecycle inference, and
low-confidence inferred routes. It does not execute `npm`, `node`, `tsc`,
`eslint`, framework CLIs, package registries, or app code.

### React / TSX Component Extraction

Phase 9.2.2 adds conservative static React component extraction for
JavaScript, TypeScript, JSX, and TSX files.

Implemented extraction rules:
1. detect React package metadata from local package.json dependency sections
2. annotate high-confidence PascalCase function, arrow-function, class,
   memo, and forwardRef component symbols when JSX is present
3. encode component metadata on existing symbols, including export kind,
   component kind, props type name, hooks, JSX component usages, source kind,
   line range, and confidence
4. emit safe `REFERENCES` edges from components to same-file props
   interfaces/type aliases and same-file component usages where resolvable
5. expose read-only component metadata through the control API

The extractor intentionally avoids React runtime rendering, state-machine
inference, deep hook semantics, dependency-array analysis, full JSX tree graph
construction, CSS/layout intelligence, framework-specific router intelligence,
and low-confidence component guesses. It does not execute `npm`, `node`, `tsc`,
`eslint`, React dev servers, package registries, or app code.

### Next.js Route Extraction

Phase 9.2.3 adds conservative static Next.js intelligence. The algorithm is
file-system based:

1. detect Next.js package metadata from local `package.json` dependency
   sections and `next.config.js`, `next.config.mjs`, or `next.config.ts`
   filenames
2. inspect `app/` routes for `page`, `layout`, `loading`, `error`,
   `not-found`, and `template` files
3. inspect `pages/` routes while ignoring Pages Router special files such as
   `_app`, `_document`, and `_error`
4. normalize route groups out of URL paths and map `[id]`, `[...slug]`, and
   `[[...slug]]` to safe route metadata
5. detect `app/api/**/route.*` handlers and exported HTTP methods
6. encode Next.js routes as existing `Route` symbols with `framework=nextjs`
   and a route kind
7. classify App Router components as basic static server/client components
   from top-of-file `"use client"` and `"use server"` directives

The extractor intentionally avoids running `next dev`, `next build`, `node`,
`npm`, `tsc`, `eslint`, package scripts, package registries, deployment
tooling, or app code. It does not implement full React Server Components
semantics, middleware execution order, Vercel/deployment intelligence,
auth-specific intelligence, or deep data fetching semantics.

### Scoped Indexing Planning

Phase 9.2.11 adds a deterministic scope-planning layer before indexing:

1. parse explicit scope strings without fuzzy interpretation
2. validate path and glob scopes under the project root
3. discover local files with the existing ignore rules and stable ordering
4. filter by path, file, glob, language, or conservative framework hints
5. resolve target scopes only from existing indexed metadata
6. return dry-run previews without SQLite mutation
7. run scoped index/reindex only against matched files

The algorithm is intentionally local and static. It does not execute shells,
package managers, app code, brokers, databases, cloud APIs, clusters, semantic
search, embeddings, or cross-project matching.

### Local Embeddings / Vector Architecture

Phase 10.0 adds architecture only. The core model defines embedding provider
capabilities, vector documents, embedding vectors, source kinds, metadata, and
a vector store contract. Default configuration keeps embeddings disabled,
provider `none`, external plugins disabled, and semantic retrieval off.

Chunk planning is deterministic:

1. prefer symbol-level chunks when symbol ranges are available
2. fall back to file chunks when no symbol chunk can be produced
3. preserve project, branch, file, optional symbol, language, framework, source
   kind, path, content hash, chunk hash, chunk index, text, and line ranges
4. split by local character limits without requiring a tokenizer
5. skip empty chunks and keep ordering stable

SQLite vector architecture uses normal tables for `vector_documents` and
`embedding_vectors`. Vectors are stored as local BLOBs with provider id,
dimension, vector hash, and indexed timestamp. Phase 10.0 includes only simple
contract/search plumbing for tests and status reporting; real local providers,
optimized SQLite search, hybrid ranking, MCP semantic tools, hosted vector DBs,
OpenAI/cloud embeddings, telemetry, and cross-project semantic search are
deferred.

### Angular Static Extraction

Phase 9.2.4 adds conservative static Angular intelligence on top of TypeScript
indexing:

1. detect Angular package metadata from local `package.json` dependency
   sections and `angular.json` / `tsconfig.app.json` filenames
2. inspect TypeScript class decorators for `@Component`, `@Injectable`,
   `@NgModule`, `@Directive`, and `@Pipe`
3. extract only safe literal object metadata such as selectors, template/style
   references, `providedIn`, standalone flags, imports, providers,
   declarations, exports, and bootstrap names
4. detect constructor dependency type names without resolving the Angular DI
   container
5. inspect Angular route config object literals for path, component,
   loadChildren, loadComponent, redirectTo, and children presence
6. encode Angular routes as existing `Route` symbols with `framework=angular`
   and Angular components as existing component metadata with
   `framework=angular`

The extractor intentionally avoids running `ng`, the Angular compiler, `node`,
`npm`, `tsc`, `eslint`, package scripts, package registries, or app code. It
does not implement template type checking, full template parsing, runtime
lifecycle semantics, deep DI/module graph resolution, RxJS/NgRx flow, or
Angular Material intelligence.

### ASP.NET Core / C# Web API Static Extraction

Phase 9.2.5 adds conservative static C# Web API extraction outside the JS/TS
`web/` module:

1. detect `.cs` and `.csproj` files locally
2. detect ASP.NET Core project references from `.csproj` text, including
   `Microsoft.NET.Sdk.Web`, `Microsoft.AspNetCore.App`, and ASP.NET Core MVC
   package/framework references
3. scan C# text for namespaces, classes, methods, constructors, and `using`
   references without requiring Roslyn or a language server
4. detect controller classes from `Controller` suffix, visible
   `ControllerBase` / `Controller` inheritance text, `[ApiController]`, and
   `[Route]`
5. extract literal `[Route]` and common HTTP method attributes
6. compose controller/action routes with `[controller]` and `[action]` token
   replacement and preserve parameter tokens such as `{id}`
7. encode ASP.NET Core routes as existing `Route` symbols with
   `framework=aspnetcore`
8. record constructor dependency type names as basic metadata only

The extractor intentionally avoids full C# semantic analysis, full DI container
resolution, middleware pipeline analysis, minimal API expansion, WPF/XAML
intelligence, runtime execution, package restore,
NuGet access, Roslyn, Visual Studio automation, OmniSharp, language servers,
cloud parsers, and telemetry.

### ORM / Database Access Static Extraction

Phase 9.2.6 adds conservative static data access extraction:

1. detect EF Core and Dapper from local `.csproj` text and C# `using`
   references
2. detect Prisma, TypeORM, Sequelize, and selected SQL drivers from local
   `package.json` dependency sections and imports
3. inspect C# text for EF Core `DbContext`, `DbSet<T>`, obvious LINQ/DbSet
   method calls, `SaveChanges`, and Dapper `Query*` / `Execute*` calls
4. inspect JS/TS text for `new PrismaClient()`, `prisma.<model>.<operation>()`,
   TypeORM `@Entity` and repository calls, and Sequelize model/query calls
5. classify coarse operations as read, insert, update, delete, execute, or
   raw_sql
6. record literal SQL snippets only when directly visible in Dapper calls
7. encode records as existing symbols with `data_access.*` metadata and expose
   them through `GET /api/data-access`

The extractor intentionally avoids database connections, SQL execution,
migration execution, Prisma generate, TypeORM/Sequelize CLIs, package managers,
full SQL parsing, full LINQ semantics, full TypeScript/C# type checking,
runtime DB behavior, schema introspection, cross-project data lineage, cloud
APIs, and telemetry.

### Basic Realtime / Socket Intelligence

Phase 9.2.7 adds conservative static realtime/socket extraction:

1. detect WebSocket, Socket.IO, SignalR, and RSocket package/project hints from
   local `package.json`, `.csproj`, imports, and usings
2. inspect JS/TS text for browser/native WebSocket constructors, message
   listeners, and sends where WebSocket context is present
3. inspect JS/TS text for Socket.IO literal `on(...)` and `emit(...)` event
   names where Socket.IO context is present
4. inspect C# text for SignalR `Hub` classes, hub methods, and
   `Clients.*.SendAsync(...)`; inspect JS/TS text for SignalR
   `HubConnectionBuilder`, `on(...)`, and `invoke(...)`
5. record minimal RSocket package/request metadata for obvious request methods
6. encode records as existing symbols with `realtime.*` metadata and expose
   them through `GET /api/realtime`

The extractor intentionally avoids network connections, socket startup,
protocol execution, payload schema inference, runtime event discovery,
cross-project event matching, package managers, package registries, external
APIs, and telemetry.

### Basic Messaging / Event-driven Intelligence

Phase 9.2.8 adds conservative static messaging/event-driven extraction:

1. detect AMQP, RabbitMQ, Kafka, Google Pub/Sub, NestJS messaging, and generic
   messaging package/project hints from local `package.json`, `.csproj`,
   imports, and usings
2. inspect JS/TS and C# text for RabbitMQ/AMQP publish, send-to-queue, consume,
   exchange, queue, bind, and routing-key callsites with literal metadata
3. inspect JS/TS and C# text for Kafka producer send, consumer subscribe/run,
   topic metadata, and simple literal consumer-group hints
4. inspect JS/TS and C# text for Google Pub/Sub topic publishers and
   subscription handlers without cloud API calls
5. inspect NestJS `@MessagePattern`, `@EventPattern`, and low-risk ClientProxy
   `emit`/`send` patterns
6. encode records as existing symbols with `messaging.*` metadata and expose
   them through `GET /api/messaging`

The extractor intentionally avoids broker connections, broker startup, cloud
API calls, runtime topic/queue discovery, payload schema inference, schema
registry calls, package managers, package registries, external APIs, and
telemetry.

### Basic Cloud / Infrastructure Intelligence

Phase 9.2.9 adds conservative static cloud/infrastructure extraction:

1. detect Dockerfile, Docker Compose, Kubernetes YAML, and Terraform files by
   local path/content heuristics
2. scan Dockerfiles for `FROM`, `EXPOSE`, `ENV`, `CMD`, and `ENTRYPOINT`
3. scan Compose YAML for services, images/build contexts, ports, environment
   keys, and `depends_on` names
4. scan Kubernetes YAML for common kinds, metadata, labels, selectors,
   containers, images, ports, ingress/service backend hints, and GKE annotations
5. scan Terraform text for provider/resource/module/variable/output blocks and
   classify visible `google_*` resources as GCP/GKE where safe
6. encode records as existing symbols with `infrastructure.*` metadata and
   expose them through `GET /api/infrastructure`

The extractor intentionally avoids Docker, Docker Compose, `kubectl`,
Terraform, `gcloud`, registry calls, provider/module downloads, cloud API
calls, credential loading, runtime discovery, security scanning, cost
estimation, cross-project deployment matching, external APIs, and telemetry.

### Basic Go Language Extraction

Phase 9.2.10 adds conservative static Go extraction:

1. detect `.go`, `go.mod`, `go.sum`, and `go.work` locally by filename or
   extension
2. strip Go comments while preserving line positions, then scan declarations
   without invoking the Go toolchain
3. extract package declarations, imports, functions, receiver methods, structs,
   interfaces, type aliases/basic type declarations, and const/var declarations
4. parse `go.mod` for module, require, and replace metadata without resolving
   modules
5. add same-file local call edges only when a callable name can be matched
   safely
6. encode basic `net/http` and simple Gin/Echo/Fiber/Chi route hints as
   existing `Route` symbols when the framework is visible from local router
   construction

The extractor intentionally avoids `go build`, `go test`, `go run`, `go list`,
`go mod download`, module registry access, package restore, compiler/type
checking, app execution, gRPC intelligence, deep framework intelligence,
external APIs, and telemetry.

### Basic .NET Desktop / WPF Extraction

Phase 9.2.12 adds conservative static WPF/XAML extraction:

1. detect modern and older WPF project hints from local `.csproj` text without
   MSBuild evaluation
2. detect `.xaml` files and classify obvious Application, Window, UserControl,
   Page, ResourceDictionary, and NavigationWindow roots
3. extract `x:Class`, obvious `.xaml.cs` code-behind paths, literal static
   DataContext hints, and ViewModel naming hints
4. extract simple `{Binding ...}` paths, command bindings, CommandParameter
   paths, StaticResource/DynamicResource keys, resource definitions, and
   ResourceDictionary source references
5. scan local C# code-behind/ViewModel files for partial classes,
   DataContext assignments, INotifyPropertyChanged, and ICommand properties
6. encode records as existing symbols with `wpf.*` metadata and expose them
   through `GET /api/wpf`

The extractor intentionally avoids Visual Studio automation, MSBuild, `dotnet`,
WPF app execution, XAML compilation, designer integration, full binding type
checking, runtime DataContext inference, deep MVVM framework analysis,
cross-project matching, external APIs, and telemetry.

### Relationship Extraction

Build graph edges from AST and import analysis.

Edges:
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

Improvement:
- confidence scoring per edge
- edge provenance: AST, import analysis, text heuristic, semantic, user recorded

### Hybrid Retrieval

Do not rely on embeddings alone.

Retrieval score should combine:
- exact symbol match
- lexical FTS/BM25 score
- semantic similarity
- graph proximity
- active session relevance
- recency
- centrality
- test relevance

Improvement:
- adaptive weights by query intent
- exact symbol queries favor graph/AST
- conceptual queries favor semantic + graph

### Context Pack Compression

Return compact context instead of raw files.

Pipeline:
1. classify query intent
2. retrieve candidates
3. graph expand with depth limits
4. rerank
5. deduplicate
6. summarize
7. pack under token budget
8. include expansion handles

Context packs must include why each item was included.

### Impact Analysis

Given a symbol:
1. find direct callers
2. find callees
3. expand references by bounded depth
4. include tests that cover affected area
5. include config/routes impacted
6. rank by risk

Risk score factors:
- fan-in
- fan-out
- centrality
- dependency depth
- test gap
- recent churn
- public API exposure

### Community Discovery

Use graph clustering to detect modules/domains.

Possible algorithms:
- Louvain community detection
- Label Propagation
- strongly connected components
- weakly connected components

Use cases:
- summarize module boundaries
- detect architecture layers
- detect circular dependencies
- improve retrieval ranking

### Centrality and Hotspot Ranking

Compute:
- PageRank
- betweenness centrality
- in-degree / out-degree
- dependency fan-in/fan-out

Use cases:
- identify critical files
- rank context
- improve impact analysis
- detect risky changes

### Token Savings Ledger

Track every MCP call.

Store:
- query type
- estimated raw exploration tokens
- returned tokens
- avoided tool calls
- latency
- files avoided
- cache hit/miss

Expose:
- per-project savings
- per-session savings
- compression ratio
- avoided grep/read-file counts

### Crash-Isolated Parser Workers

Tree-sitter grammars may fail or panic.

Use subprocess workers:
- parent process queues file jobs
- worker receives JSONL parser jobs over stdin and returns structured JSON over stdout
- if worker crashes or times out, parent records the failure and continues
- sync continues

Improvement:
- mark file as parse_failed with reason
- retry retryable worker failures up to a bounded retry count
- store branch-aware parse failure diagnostics for later UI and control-server inspection

### Branch-Aware Indexing

Store separate graph snapshots per branch or worktree.

Use:
- branch name
- git commit hash
- working tree dirty state
- per-branch database namespace

Use cases:
- compare branches
- avoid polluted graph from branch switching
- cross-branch search

## Improvements Beyond Both Projects

Added requirements:
- strict offline-first default
- no required cloud APIs or hosted databases
- local embeddings by default
- local Qdrant only
- plugin-only cloud integrations
- disabled-by-default external services
- edge confidence and provenance
- adaptive ranking weights
- token-budget context packing
- expansion handles instead of large blobs
- UI-visible diagnostics
- token savings dashboard
- dependency path visualization
- test-gap risk scoring
- architecture boundary detection
- duplicate code cluster detection
- generated/vendor classifiers
- profile-based language/indexing depth

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

### Completed Through Current Phase

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

### Completed Through

- Phase 10.4: MCP / Control API Integration
- Phase 10.5: Benchmark + Quality Evaluation
- Phase 11.0: Cross-Project Architecture Model + Contracts

### Planned Phases

- Phase 11.1: Group Query Federation
- Phase 11.2: Cross-Repo Route / API Matching
- Phase 11.3: Cross-Repo Messaging Matching
- Phase 11.4: Cross-Repo Package / Contract / Infra Matching
- Phase 11.5: Group-Level Impact + Context Pack
- Phase 11.6: Architecture Graph / Service Map API
- Phase 11.7: Cross-Project Benchmark + Docs
- Phase 12: Symbolic Editing MVP
- Phase 13: Rename / Refactor MVP
- Phase 14: Additional Backend Language Support
- Phase 15: Systems / Mobile Language Support

### Phase 10.1 Local Hash Embeddings

Phase 10.1 adds `local_hash`, a deterministic lexical embedding provider. The
algorithm lowercases text, tokenizes on identifier/word boundaries, splits
simple camelCase and snake_case forms, adds token bigrams, hashes features into
a fixed-size vector with signed hashing, and optionally L2-normalizes the
result. Empty input deterministically returns a zero vector. Long input is
truncated at a configured character boundary before tokenization.

The provider requires no model file, API key, network access, hosted vector
database, telemetry endpoint, or paid dependency. It is suitable for offline
chunk/vector generation, but it is not a neural semantic model. MCP/control
semantic integration is available from Phase 10.4, and fixture-based quality
benchmarking is available from Phase 10.5.

### Phase 10.2 SQLite Vector Search

Phase 10.2 stores vector documents and embedding vectors in normal SQLite
tables. Vectors are encoded as validated little-endian `f32` BLOBs, keyed by
document, provider, and dimension, and rejected when dimensions mismatch or
values are NaN/infinite. The storage layer uses deterministic upserts for
documents and vectors, deletes vector documents by file or project/branch, and
relies on SQLite foreign-key cascade behavior to avoid orphan vectors.

Search is exact brute-force cosine over SQLite-filtered candidates. SQLite
first narrows candidates by project, branch, provider, dimension, language,
framework, source kind, file, symbol, and optional path prefix. Rust then
decodes vectors, validates dimensions, computes cosine similarity, applies
`min_score`, and sorts deterministically by score descending, path ascending,
chunk index ascending, and document id ascending. This phase requires no native
SQLite vector extension, hosted vector database, approximate nearest neighbor
index, API key, model download, cloud API, or telemetry. It is raw vector search,
not final hybrid semantic ranking.

### Phase 10.3 Hybrid Ranking

Phase 10.3 adds deterministic ranking in `b3-query`. The ranking layer combines
local lexical token overlap, SQLite vector cosine scores, and conservative
metadata boosts for language, framework, source kind, path terms, symbol chunks,
and compact chunks. Default weights are lexical `0.4`, vector `0.5`, and
metadata `0.1`; custom weights are validated and normalized before scoring.

Results are sorted deterministically by final score, vector score, lexical
score, path, line, and document id. Optional explanations include component
scores, matched terms, boosts, vector provider/dimension, filters, and fallback
warnings.

### Phase 10.4 MCP / Control Integration

Phase 10.4 exposes the Phase 10.3 ranking layer through thin local adapters:
`POST /api/search/hybrid` in `b3-control` and the MCP `semantic_search` tool in
`b3-mcp-runtime`. The adapters validate query text, limits, weights, min score,
source kind, and relative path prefixes, then delegate to `b3-query`; they do
not implement ranking, embedding generation, or SQLite search directly.

The integration remains local/offline. It uses `local_hash` plus SQLite vector
data when available and returns lexical/metadata fallback warnings when vector
data is missing. It does not add hosted vector databases, cloud providers,
model downloads, telemetry, cross-project semantic search, or benchmark quality
claims.

### Phase 10.5 Search Quality Benchmark

Phase 10.5 adds fixture-based quality evaluation in `b3-bench`. The benchmark
seeds a small local semantic-search fixture into SQLite, creates local_hash
vectors, and evaluates the same query set in lexical-only, vector-only, and
hybrid modes. Metrics include hit@1/3/5/10, MRR, average score, average/p50/p95
latency, result counts, fallback counts, source-kind matches, file matches, and
symbol matches.

The baseline JSON contains a `semantic_quality` section and the terminal output
prints a compact comparison table. Token/context savings are estimated with a
simple chars/4 approximation. The results are deterministic fixture baselines,
not production quality guarantees, and they do not imply neural embedding
quality.

### Phase 11.0 Cross-Project Architecture Contracts

Phase 11.0 adds model contracts rather than matching algorithms. Projects,
groups, services, architecture nodes, architecture edges, future match
candidates, confidence, evidence, warnings, and source provenance are
serializable DTOs in `b3-core`. Deterministic IDs are derived from stable local
keys, and every future relationship is expected to carry explicit confidence
and provenance.

Normalization helpers are deliberately small and deterministic: HTTP methods
are uppercased, route paths are slash-normalized and lowercased, messaging
topics/queues/routing keys become stable local keys, package names are
case-normalized, and service/resource names are normalized without fuzzy
matching. No project database federation, cross-repo matching, global database
merge, cloud graph database, hosted vector database, or remote lookup is part
of this phase.

### Phase 11.1 Group Query Federation

Phase 11.1 federates read-only queries across registry-defined local project
groups. The registry supplies group membership and per-project database paths;
each existing project database is opened independently in read-only mode and
queried through existing storage readers. Missing registry projects, missing
DBs, unreadable DBs, and unindexed DBs become structured warnings so one bad
project does not fail the entire group.

The output is summary-oriented: group context, per-project status, metadata
counts, semantic/vector readiness, and federated metadata rows with project
identity attached. The federation layer deliberately does not create
cross-project edges, infer route/message/package relationships, merge
databases, or build a service map.

### Phase 11.1.1 Context Efficiency Benchmark

Phase 11.1.1 adds an end-to-end local efficiency benchmark in `b3-bench`. It
models a deterministic file-by-file exploration baseline and compares it with
`search_code_only`, `semantic_search_only`,
`semantic_search_context_pack:minimal`, `semantic_search_context_pack:balanced`,
`semantic_search_context_pack:deep`, and a low-risk `group_federated_summary`
workflow. The group workflow measures summary efficiency only and does not
perform cross-project matching.

Each task has expected files, optional symbols, source kinds, answer facts, and
coverage tags. Answer quality is scored from those deterministic fixture hits
plus top-k relevance, producing a compact `0.0..1.0` approximation. This is not
human answer grading and does not require an LLM.

Token estimates use `chars / 4`, avoiding external tokenizers and billing
claims. Tool-call estimates use a fixed local model: the naive baseline counts
broad search plus full-file opens, while B3-assisted modes count the modeled
search, context assembly, metadata, or group-summary call. Output includes
`current_value`, `target_value`, `target_met`, and `gap_to_target`.

Current balanced context-pack fixture results are about `1.11x` fewer tokens
against the `10.0x` target, `4.05x` fewer modeled tool calls against the `2.1x`
target, and `0.699` answer-quality approximation against the `0.8` target. The
benchmark therefore proves only the tool-call target for this fixture/model.
Token reduction and quality remain work items.

### Phase 11.2 Cross-Repo Route/API Matching

Phase 11.2 matches HTTP client call literals to server route metadata inside a
registry-defined project group. The matcher keeps the Phase 11 storage model:
each project remains in its own repo-local `.b3/b3.db`, opened read-only during
federation. No global database merge, cloud graph database, hosted vector
database, telemetry, external API, runtime HTTP request, DNS lookup, or remote
OpenAPI fetch is used.

Route keys are deterministic: method is uppercased, unknown methods use
`UNKNOWN`, paths get a leading slash, duplicate/trailing slashes are normalized,
query strings are removed, and route parameters such as `:id`, `[id]`, and
`<id>` become `{id}`. Server endpoints come from existing route metadata and
frontend page routes are excluded as backend targets by default.

Client call extraction is conservative and local. It recognizes literal
JS/TS `fetch`, Axios/member-client calls, Angular-style `HttpClient`, C#
`HttpClient`, and Go `http` call patterns in already-indexed file text.
Literal same-file base URLs such as `"/api"` can be composed; runtime
environment variables, secrets, DNS, and HTTP are not resolved.

Matching rules are ordered by evidence strength:

- exact method plus normalized path: high confidence
- unknown client method plus exact normalized path: medium confidence
- exact method plus route pattern, such as `/api/users/123` to
  `/api/users/{id}`: high confidence with route-pattern evidence
- same path with different method: low confidence with a method-mismatch warning

Output is an `ArchitectureMatchCandidate` plus corresponding
`ArchitectureNode` and `ArchitectureEdge` records for `CallsHttpRoute`, with
compact evidence, provenance, deterministic IDs, deterministic sorting, and
dedupe. Messaging matching is added in Phase 11.3, package/contract/infra
matching is added in Phase 11.4, and group impact/context pack plus service-map
APIs remain deferred.

### Phase 11.3 Cross-Repo Messaging Matching

Phase 11.3 matches local producer and consumer messaging metadata inside a
registry-defined project group. It preserves the Phase 11 storage model: each
project remains in its own repo-local `.b3/b3.db`, opened read-only during
federation. No global database merge, cloud graph database, hosted vector
database, telemetry, external API, broker connection, runtime publish/consume,
or cloud Pub/Sub call is used.

Messaging keys are deterministic and conservative. Broker names are normalized
to known local buckets such as `kafka`, `rabbitmq`, `pubsub`, `nestjs`, or
`unknown`. Channel kinds are normalized to `topic`, `queue`, `pattern`,
`routing_key`, or related local kinds. Channel names are trimmed, surrounding
quotes/slashes are removed, repeated whitespace is collapsed, and comparison
uses a lower-case normalized key without fuzzy semantic matching.

Producer and consumer records come from existing messaging metadata. Producers
are identified by outbound/producer/publisher/client-style metadata, while
consumers are identified by inbound/consumer/subscriber/handler/pattern-style
metadata. The matcher considers topic, queue, pattern, routing key, and
exchange+routing-key keys where present, but it does not simulate broker
routing or infer wildcard bindings without explicit metadata.

Matching rules are ordered by evidence strength:

- same broker, channel kind, and normalized name: high confidence
- same broker with compatible topic/queue/pattern name: high confidence
- one side with unknown broker plus exact normalized name: medium confidence
- NestJS pattern matching topic/queue/event name: medium confidence
- same normalized name with conflicting broker kinds: low confidence with a
  broker-mismatch warning

Output is an `ArchitectureMatchCandidate` plus corresponding
`ArchitectureNode` and `ArchitectureEdge` records for `PublishesMessage`, with
compact evidence, provenance, deterministic IDs, deterministic sorting, and
dedupe. Package/contract/infra matching is added in Phase 11.4, group
impact/context pack in Phase 11.5, and service-map APIs in Phase 11.6.

### Phase 11.4 Cross-Repo Package / Contract / Infra Matching

Phase 11.4 matches local package, contract/schema, and infrastructure metadata
inside a registry-defined project group. It preserves one repo-local
`.b3/b3.db` per project and opens project DBs read-only through federation. It
does not run package managers, Docker, Kubernetes, Terraform, cloud CLIs,
remote schema fetches, schema compilers, external APIs, telemetry, hosted
vector databases, or cloud graph databases.

Package keys use `package:{ecosystem}:{name}` with conservative normalization:
npm names are lowercased, .NET names compare case-insensitively, Go module
paths are exact/lowercase where safe, Rust crate names normalize underscore and
hyphen variants, and unknown ecosystems remain allowed with lower confidence.
Providers come from local manifest identity such as `package.json` name,
`.csproj` package/assembly/root namespace, `go.mod module`, and Cargo package
name. Consumers come from local manifest dependencies/references only.

Contract keys use `contract:{kind}:{name}` for DTO/model/interface/type/enum
and local OpenAPI, GraphQL, protobuf, Avro, and JSON schema names. Exact names
across projects produce medium/high confidence depending on kind agreement.
Generic names such as `User`, `Request`, `Response`, `Model`, `Item`, and
`Data` are low confidence. The matcher does not validate schema compatibility.

Infrastructure keys use `infra:{kind}:{name}` with optional namespace for
Docker Compose services/images, Kubernetes services/deployments/configmaps/
secrets, Terraform resources/modules, databases, caches, queues, Pub/Sub, and
unknown resources. Relationships are conservative: Compose `depends_on`,
Kubernetes Service selectors matching workload labels, image/name overlap, and
Terraform module/resource name overlap. Secret/config names are used only by
name; values are never extracted.

Output is an `ArchitectureMatchCandidate` plus `ArchitectureNode` and
`ArchitectureEdge` records for `DependsOnPackage`, `ImportsPackage`,
`SharesContract`, `UsesContract`, `DependsOnInfrastructure`, `DeploysService`,
and `SelectsService`, with compact evidence, warnings, deterministic IDs,
deterministic sorting, and dedupe. Group impact/context pack is added in Phase
11.5; service-map APIs are added in Phase 11.6.

### Phase 11.5 Group-Level Impact + Context Pack

Phase 11.5 builds a bounded local impact graph from existing Phase 11.2 route
matches, Phase 11.3 messaging matches, and Phase 11.4 dependency matches. It
does not extract new runtime facts, merge project DBs, perform service
discovery, execute HTTP requests, connect to brokers, run package managers,
run Docker/Kubernetes/Terraform/cloud CLIs, call external APIs, or use hosted
graph/vector databases.

Impact requests support route, message, package, contract, infrastructure,
file, symbol, and query seeds. Seed paths must be relative and traversal is
bounded by clamped depth and limit values. Ambiguous seeds become multiple
deterministically sorted seed nodes; missing seeds return structured errors.

Traversal uses architecture nodes and edges from match candidates. Upstream
follows outgoing dependency edges, downstream follows dependent edges and
safe publish/share/deploy relationships, and both combines the two. Paths
dedupe nodes and edges, preserve evidence summaries, and aggregate confidence
conservatively by taking the weakest hop with a small depth penalty.

Context packs are generated from the impact result with `minimal`, `balanced`,
and `deep` profiles. Each profile has a fixed char budget, includes seed and
impact summaries, top relationship paths, key files/symbols, compact snippets,
warnings, skipped items, truncation reason, and chars/4 token estimates. File
content is read from existing local project DBs only, snippets are capped, and
full-file dumps are avoided.

### Phase 11.6 Architecture Graph / Service Map API

Phase 11.6 builds a bounded architecture graph on demand from existing Phase
11.2 route matches, Phase 11.3 messaging matches, Phase 11.4 dependency
matches, and Phase 11.1 group summaries. It does not persist graph state,
merge project DBs, perform runtime discovery, execute HTTP requests, connect to
brokers, run package managers, run Docker/Kubernetes/Terraform/cloud CLIs, call
external APIs, or use hosted graph/vector databases.

Graph requests support relationship-kind filters, project filters,
confidence filters, evidence/warning/unresolved toggles, bounded node/edge
limits, and optional seed-node expansion. Invalid relationship kinds and unsafe
project filters return structured errors. Nodes and edges are sorted
deterministically and deduped by stable architecture IDs.

Graph output includes project, route, messaging, package, contract, and
infrastructure nodes where existing match evidence provides them. Edges keep
their relationship kind, conservative confidence, evidence, warnings, and source
phase (`route_matching`, `messaging_matching`, `dependency_matching`, or
`federation_summary`). Unresolved relationships are reported as warnings instead
of being invented.

The service map is a project-level grouping over the graph. Each service summary
uses the registry project identity when deeper service identity is not available,
counts inbound/outbound route, messaging, dependency, and infrastructure
relationships, and emits service-to-service edges with aggregated confidence.
Summary metrics include project/service/node/edge counts, relationship-kind
counts, confidence distribution, connected projects, isolated projects,
unresolved count, and warning count. The API is intentionally non-UI; graph
visualization remains deferred.

### Phase 11.7 Cross-Project Benchmark + Docs

Phase 11.7 measures Phase 11 architecture behavior with local fixture data and
optional local repository candidates from `benchmarks/b3.benchmark.toml`. The
benchmark adds a `cross_project_benchmark` JSON section while preserving the
existing `semantic_quality` and `efficiency_metrics` sections. It does not
upload results, call external APIs, run package managers, run
Docker/Kubernetes/Terraform, connect to brokers, execute runtime HTTP calls,
merge databases, or require hosted graph/vector databases.

The architecture benchmark creates a deterministic local fixture group with
frontend, API, worker, and shared-package projects. It measures group
federation, route/API matching, messaging matching, package/contract/infra
matching, group impact, cross-repo context pack generation, architecture graph
construction, and service map construction. Optional local repos such as
`D:\Project\b3_mcp`, `D:\Project\Project_B`, and `D:\Project\Tuvi_B` are
inspected only when present; missing paths or DBs become warnings.

Metrics include readiness flags, match counts, impact counts, graph/service map
counts, warning and unresolved ratios, context-pack chars and chars/4 token
estimates, modeled token/tool-call reduction, and deterministic task coverage.
Targets are reported as comparisons: `10.0x` token reduction, `2.1x` tool-call
reduction, and `0.8` answer quality. They are not treated as product claims
unless the current local run meets them, and even then the result is scoped to
the local fixture/config inputs.

Branch safety is warning-only. The benchmark reports requested/used branch
assumptions and notes that after switching branches users should reindex before
comparing results. Full Git history, blame, branch diff, PR, remote, and GitHub
API intelligence remain Phase 21.

## Additional Planned Algorithms

The following algorithms and techniques are planned for future phases:

- **Query trace / retrieval explainability**: Exposing how and why specific nodes were retrieved.
- **Adaptive ranking by query intent**: Tuning hybrid retrieval weights dynamically.
- **Value-per-token context packing**: Maximizing utility of context windows based on token limits.
- **Diversity penalty**: Preventing redundant or highly similar context from crowding out varied results.
- **Dependency path finding**: Shortest paths between symbols or files.
- **Tarjan SCC / cycle detection**: Identifying strongly connected components and circular dependencies.
- **Impact risk scoring**: Assessing the potential blast radius of a change.
- **Test impact analysis**: Mapping changed files to affected test suites.
- **PageRank**: Identifying core, heavily-depended-upon modules.
- **Centrality scoring**: Finding bottlenecks and central orchestration points.
- **Community detection**: Grouping related symbols into architectural boundaries.
- **Git churn ranking**: Boosting relevance of frequently modified files.
- **AST fingerprinting**: Identifying structural patterns independent of names.
- **MinHash / SimHash duplicate detection**: Locating duplicated or highly similar code blocks.

## Benchmark Methodology

Optimization must be data-driven. Phase 8.2 establishes a local benchmark
baseline before refactor and optimization work.

The baseline runner measures:

- cold startup
- MCP tools/list latency by selected profile and simple tools/call latency
- control-server health/status handler latency
- find_symbol and search_code latency
- graph neighbors and graph path latency
- context_pack and impact_analysis latency
- full indexing speed
- changed-file reindex latency
- watcher debounce latency
- SQLite graph summary query latency
- parser worker request latency

Benchmark data is local-only and written as JSON under `target/benchmarks`.
Results are not uploaded, and regression thresholds are advisory unless
explicitly enabled. The `memory_kb` field is best-effort and may be `null`
until platform-specific memory collection is added.

Phase 8.6 adds profile metadata to MCP tools/list benchmark entries. The default
`mcp_tools_list_latency` entry measures the `optimized` profile, and additional
entries measure `full`, `tiny`, and `enterprise`. Existing JSON fields are
preserved; `metadata.profile` and `metadata.tool_count` record the selected
profile and returned tool count.

Phase 8.4 refined the watcher debounce benchmark after the baseline showed the
largest value was dominated by an intentional sleep. The benchmark now measures
coalescing overhead while preserving the JSON output shape; configured debounce
wait time remains a policy setting, not an optimization target.

## Command Output Compaction

Phase 8.5 adds rule-based local compaction for token-heavy command output. The
algorithm is intentionally deterministic:

- detect command family from command text or argv
- apply a family-specific string compactor when available
- preserve non-zero exit status, stderr, compiler errors, failed tests, conflict
  indicators, and concise summaries
- enforce a byte budget with explicit truncation metadata
- estimate token savings from byte reduction only

No commands are executed by the compactor, and no LLM/cloud summarization is
used. The first benchmark entry measures compaction latency against static local
fixture text.

## Language Backend Architecture

Phase 9.0 introduces capability discovery before broad language implementation.
Language detection is deterministic and local, based on file extensions and
selected filenames. Detection is separated from support level:

- `Good`: Rust through the existing tree-sitter parser path.
- `Basic`: implemented local static or tree-sitter-backed extraction for a
  bounded subset, or planned languages with detect-file rules only where noted.
- `Unsupported`: unknown files with no local detection rule.

No LSP process, semantic search, embeddings, framework intelligence, or
cross-project architecture analysis is part of this phase. Benchmark semantics
remain focused on existing Rust fixtures and current query/index behavior.
