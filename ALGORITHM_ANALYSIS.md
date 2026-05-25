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

### Planned Phases

- Phase 9.2.7: Realtime / Socket Intelligence
- Phase 9.2.8: Messaging / Event-driven Intelligence
- Phase 9.2.9: Cloud / Infrastructure Intelligence
- Phase 9.2.10: Go Language Support
- Phase 9.2.11: Scoped Indexing + Intelligence Targets
- Phase 9.2.12: .NET Desktop / WPF Intelligence
- Phase 10: Local Embeddings + Vector Search
- Phase 10.1: Semantic Context Upgrade
- Phase 11: Architecture Intelligence
- Phase 12: Git Intelligence
- Phase 13: Duplicate / Similarity Detection
- Phase 14: Real Plugin System
- Phase 15: Packaging + Installers

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
