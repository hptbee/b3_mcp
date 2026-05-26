# B3 Project Plan

B3 is a local-first, offline-first, free-by-default AI-native code intelligence platform for coding agents and local developer workflows.

This document is the source of truth for the detailed roadmap. `README.md` should remain concise and traditional.

---

## Current Roadmap Status

```text
Completed:
- Phase 1 Workspace / Scaffold
- Phase 1.5 Contracts / Boundaries
- Phase 2 SQLite Storage / Schema Foundation
- Phase 3 Incremental Indexer Skeleton
- Phase 3.1 Indexer Audit / Cleanup
- Pre-Phase-4 Plugin Contracts / Docs / CI
- Phase 4 Real Rust Parsing + SqliteStorage â†” IndexStore Adapter
- Phase 4.1 Project/Branch Auto Ensure + Deleted File Cleanup
- Phase 5 Query Engine + Graph Traversal + Context Pack
- Phase 5.1 Query Hardening + Retrieval Explainability
- Phase 5.2 Ranking Algorithms Upgrade
- Phase 6 MCP Tools over Query Engine
- Phase 6.0.1 Live MCP Runtime Wiring
- Phase 6.1 Impact Intelligence
- Phase 6.2 PageRank / Centrality
- Phase 6.3 MCP Runtime Hardening + Real-world Smoke Test
- Phase 7 Control Server + Localhost API
- Phase 7.1 Web UI Foundation
- Phase 7.2 Graph Explorer UI
- Phase 8.4 Performance Optimization Pass A
- Phase 8.5 Command Output Compaction
- Phase 8.5.1 Project Init + Manual Index Command
- Phase 9.2.1 Node.js / REST API Intelligence
- Phase 9.2.2 React / TSX Component Intelligence
- Phase 9.2.3 Next.js Intelligence
- Phase 9.2.3.1 Indexer Module Split / Refactor Checkpoint B
- Phase 9.2.4 Angular Intelligence
- Phase 9.2.4.1 Web Module Split / Refactor Checkpoint C
- Phase 9.2.5 ASP.NET Core / C# Web API Intelligence
- Phase 9.2.6 ORM / Database Access Intelligence
- Phase 9.2.7 Realtime / Socket Intelligence
- Phase 9.2.8 Messaging / Event-driven Intelligence
- Phase 9.2.9 Cloud / Infrastructure Intelligence
- Phase 9.2.10 Go Language Support
- Phase 9.2.11 Scoped Indexing + Intelligence Targets
- Phase 9.2.12 .NET Desktop / WPF Intelligence
- Phase 10.0 Local Embeddings + Vector Search Architecture
- Phase 10.1 Local Embedding Provider MVP
- Phase 10.2 SQLite Vector Storage / Search Index
- Phase 10.3 Hybrid Search Ranking
- Phase 10.4 MCP / Control API Integration
- Phase 10.5 Benchmark + Quality Evaluation
- Phase 11.0 Cross-Project Architecture Model + Contracts
- Phase 11.1 Group Query Federation
- Phase 11.1.1 Context Efficiency + Tool Call Reduction Benchmark
- Phase 11.2 Cross-Repo Route / API Matching
- Phase 11.3 Cross-Repo Messaging Matching
- Phase 11.4 Cross-Repo Package / Contract / Infra Matching
- Phase 11.5 Group-Level Impact + Context Pack
- Phase 11.6 Architecture Graph / Service Map API
- Phase 11.7 Cross-Project Benchmark + Docs
- Phase 12 Symbolic Editing MVP
- Phase 13 Rename / Refactor MVP
- Phase 14 Additional Backend Language Support
- Phase 15 Systems / Mobile / Config / Web File Support A
- Phase 16 Config / Data / Web File Support B / Hardening
- Phase 17 Language and Technology Quality Audit
- Phase 18.1 Test Organization Split
- Phase 18.2 Control Server Route Module Split
- Phase 18.3 Storage Module Split
- Phase 18.4 Indexer Pipeline / Dispatch Split
- Phase 18.5 Shared Helper Consolidation
- Phase 18.6 Optional Core / Query Architecture Split Review
- Phase 18.7 Preliminary Refactor Checkpoint Verification
- Phase 18.8 b3-indexer Deep Restructure
- Phase 18.9 Final Phase 18 Verification

Current/Next:
- Phase 19 Performance Optimization Pass B

Upcoming:
- Phase 20 Web UI Developer Console Refresh
- Phase 21 Git Intelligence
```

## Previously Deferred -> Scheduled Phases (12–20)

The items previously listed under an older deferred block
have been renumbered and scheduled in the Phase 12–20 sequence below.

## Phase 12 Symbolic Editing MVP

Status: Completed.

### Tools

- `replace_symbol_body`
- `insert_before_symbol`
- `insert_after_symbol`
- `preview_edit`
- `apply_edit`
- file/range replace
- append/prepend file

### Rules

- dry-run first
- show affected files
- require explicit apply
- create backups by default
- return reindex recommended warnings after apply
- no blind text replacement
- no full-file rewrite unless needed
- readonly profile must hide mutation tools
- MCP editing tools are deferred; Phase 12 exposes local Control API endpoints only

### Scope Completed

- added editing DTOs/contracts in `b3-core`
- added `b3-query::editing::SymbolicEditEngine`
- added deterministic target resolution for explicit file ranges, whole files, indexed symbols, and symbol IDs
- added safe single-file preview/apply with UTF-8 and binary validation, project-root containment, stale text/hash checks, edit size limits, unified diff output, and default backups
- added local Control API endpoints `POST /api/edit/preview` and `POST /api/edit/apply`
- kept rename/refactor, update-all-references, multi-file rename workflows, MCP edit tools, UI editing, formatter/compiler execution, and Git Intelligence deferred

---

## Phase 13 Rename / Refactor MVP

Status: Completed.

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
- Phase 13 implementation does not require LSP; it uses local indexed symbols and conservative identifier matching
- MCP rename/refactor tools are deferred; Control API only

### Scope Completed

- added rename/refactor DTOs/contracts in `b3-core`
- added `b3-query::refactor::RenameRefactorEngine`
- added deterministic symbol target resolution by symbol ID or file path plus symbol name/old name
- added conservative occurrence discovery from indexed symbol evidence, graph/FTS candidate files, and bounded identifier scanning
- skipped comments/strings by default, with low-confidence inclusion only when explicitly requested
- added bounded single-file and explicit bounded multi-file rename planning
- added backup-protected explicit apply with all-file validation before writes
- added local Control API endpoints `POST /api/refactor/rename/preview` and `POST /api/refactor/rename/apply`
- kept broad refactoring, extract method, move symbol/module, IDE-grade semantic rename, compiler/formatter execution, MCP refactor tools, UI editing, and Git Intelligence deferred

---

## Phase 14 Additional Backend Language Support

Status: Completed.

Scope completed:

- added modular static backend language extraction under `b3-indexer::backend_languages`
- added basic local Python detection for `.py`, Python project metadata files, imports, classes, functions, async functions, decorators, constants, FastAPI/Flask/Django route hints, SQLAlchemy/Django ORM/raw SQL hints, and Celery/Pika messaging hints
- added basic local Java detection for `.java`, Maven/Gradle project metadata, packages/imports, classes/interfaces/enums/records, methods, Spring/JAX-RS route hints, JPA/JDBC hints, and Kafka/Rabbit listener hints
- added basic local Kotlin detection for `.kt`/`.kts`, Gradle Kotlin metadata, packages/imports, classes/data classes/objects/interfaces/enums, functions, Spring/Ktor route hints, JPA hints, and Kafka/Rabbit listener hints
- added basic local PHP detection for `.php`, Composer metadata, namespaces/use statements, classes/interfaces/traits/enums, functions/methods, Laravel/Symfony/Slim route hints, Eloquent/raw SQL hints, and Laravel queue hints
- added basic local Ruby detection for `.rb`, Gemfile metadata, require/module/class/method extraction, Rails/Sinatra route hints, ActiveRecord hints, and Sidekiq/ActiveJob hints
- wired new languages into `DefaultLanguagePack`, language detection, `/api/languages`, `/api/capabilities`, existing route/data-access/messaging metadata shapes, graph relationships, storage, query, and architecture matching paths
- added focused tests for detection, symbols/imports, route hints, data-access hints, messaging hints, and capability/status reporting

Rules:

- static/local/offline analysis only
- no Python/PHP/Ruby/JVM runtime execution
- no pip, poetry, uv, composer, bundle, Maven, or Gradle execution
- no compiler, formatter, package restore, language server, external API, cloud service, telemetry, or internet requirement
- compiler-grade semantics, deep framework analysis, mobile/system language work, config/data/web file support, quality audit, architecture graph UI, and full Git Intelligence remain deferred

Go is already handled in Phase 9.2.10 as basic local static analysis.

---

## Phase 15 Systems / Mobile / Config / Web File Support A

Status: Completed.

Scope completed:

- added modular static systems/mobile parsers under `b3-indexer::systems_languages`
- added basic C/C++ file and project metadata detection for `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `CMakeLists.txt`, `Makefile`, and `compile_commands.json`
- extracted conservative C/C++ includes, macros, structs, enums, typedefs, namespaces, classes, methods, and obvious functions
- added basic Swift detection for `.swift` and `Package.swift`, imports, classes, structs, enums, protocols, extensions, functions, SwiftUI View hints, app-entry hints, and URLSession literals
- added basic Objective-C detection for `.m`/`.mm`, imports, interfaces, implementations, protocols, properties, methods, UIViewController hints, and NSURLSession literals
- added basic Dart/Flutter detection for `.dart`, `pubspec.yaml`, and `analysis_options.yaml`, imports, classes, mixins, enums, functions, Widget/build hints, route literals, and HTTP literals
- added modular static config-file parsers for YAML, JSON, TOML, and XML with key paths/tables/elements/attributes plus safe package/dependency names
- kept sensitive config values redacted or skipped; secret-like names are indexed only as names/keys
- added static HTML/template extraction for titles, ids/classes/data attributes, script/style references, hrefs, and form action route hints
- added static CSS/SCSS extraction for class/id selectors, custom properties, imports, url asset references, SCSS variables/mixins, and keyframes
- hardened XAML metadata with Style TargetType, ControlTemplate/DataTemplate keys, x:Name, xmlns namespace hints, merged dictionaries, resource references, bindings, commands, and code-behind hints
- added static Three.js/WebGL hints from JS/TS imports/usages and asset literals without browser/WebGL execution
- added static ksqlDB parsing for streams, tables, connectors, Kafka topic literals, SELECT/INSERT dependencies, and messaging hints without Kafka/ksqlDB connections
- wired new support into `DefaultLanguagePack`, language detection, `/api/languages`, `/api/capabilities`, existing route/messaging/infrastructure-compatible metadata shapes, and tests

Rules:

- static/local/offline analysis only
- no clang, gcc, make, CMake, xcodebuild, swift, dart, flutter, npm, browser, WebGL, Kafka, ksqlDB, Docker, Kubernetes, Terraform, compiler, formatter, package-manager, runtime, broker, database, cloud API, telemetry, internet, or mandatory language-server requirement
- support is Basic unless explicitly detect-only/project metadata; compiler-grade semantics, deeper config/data/web hardening, architecture graph UI, Phase 17 quality audit, full Git Intelligence, and broad refactor work remain deferred

---

## Phase 16 Config / Data / Web File Support B / Hardening

Status: Completed.

Scope completed:

- added shared secret redaction/value classification for config files, with expanded secret-like key coverage
- added safe env-like file support for `.env.example`, `.env.sample`, `.env.defaults`, `.env.template`, `example.env`, and `sample.env`; real `.env.*` files are parsed key-only/redacted
- added static config reference hints for env placeholders such as `${ORDER_TOPIC}` without reading OS environment
- hardened YAML metadata with value classes, env reference hints, Kubernetes Secret evidence, and ConfigMap reference hints while keeping values secret-safe
- hardened JSON metadata with value classes, safe hints, appsettings messaging config evidence, launchSettings URL hints, and dependency/package extraction
- hardened TOML metadata with value classes, dependency/project section extraction, and secret redaction
- hardened XML metadata with safe attribute classification, app/web config `<add>` entries, Spring bean names/refs where simple, Android manifest package hints, and Maven dependencies
- hardened HTML/template extraction for `.html`, `.htm`, `.cshtml`, `.erb`, `.ejs`, and `.hbs` through existing HTML language mapping, including form actions, local href/API literals, data attrs, and secret-like attr redaction
- hardened CSS/SCSS extraction with `@use`, `@forward`, media queries, selectors, custom properties, imports, url assets, variables, mixins, and keyframes
- continued XAML hardening from Phase 15 for merged dictionaries, resources, bindings, commands, names, namespaces, styles/templates, code-behind, and ViewModel hints
- added Basic static SQL parsing for `.sql` files: CREATE TABLE/VIEW/PROCEDURE/FUNCTION, SELECT/FROM/JOIN, INSERT/UPDATE/DELETE table references, and migration path hints
- hardened ksqlDB parsing with stricter SQL-vs-ksqlDB detection, KEY_FORMAT/VALUE_FORMAT, JOIN dependencies, topic direction metadata, and messaging hints
- hardened Three.js/WebGL hints with lights, ShaderMaterial, WebGLRenderer, shader asset refs, and canvas ids
- added compact local fixtures under `benchmarks/fixtures/config_data_web_hardening`
- updated `/api/languages` and `/api/capabilities` with Phase 16 Basic hardened/static support for config/data/web, SQL, env, ksqlDB, and Three.js/WebGL hints

Rules:

- static/local/offline analysis only
- no package managers, compilers, formatters, Docker/Kubernetes/Terraform execution, Kafka/ksqlDB/RabbitMQ/broker/database connections, browsers, WebGL, runtime code execution, OS environment reads, external APIs, cloud services, telemetry, internet, or mandatory language server
- Phase 17 quality audit, architecture graph UI, full Git Intelligence, broad refactor work, and advanced messaging/runtime intelligence remain deferred

---

## Phase 17 Language and Technology Quality Audit

Status: Completed.

Completed audit scope:

- audited support levels for core languages, web/backend frameworks, systems/mobile languages, config/data/web files, SQL/ksqlDB/env, Three.js/WebGL hints, integration surfaces, symbolic editing, and rename/refactor
- aligned core language registry truthfulness with implemented JS/TS/JSX/TSX, Dockerfile/Compose, and XAML static/basic support instead of stale planned/detect-only metadata
- added a distinct `DetectOnly` support level for future detection-only entries so Basic is not overloaded
- added `/api/capabilities` Phase 17 quality-audit metadata covering support matrix, capability reporting, fixture coverage, metadata consistency, secret redaction, false-positive guardrails, benchmark audit, and explicit non-claims
- hardened SQL and ksqlDB comment handling so commented-out SQL/ksqlDB statements do not create table/topic/stream metadata
- added regression tests for support matrix truthfulness, capability/status non-claims, SQL/ksqlDB comment false positives, HTML remote route handling, and secret-safe static metadata
- confirmed Phase 15/16 fixture trees exist for systems/mobile/config/web and config/data/web hardening
- kept storage/schema unchanged and continued using existing symbol, metadata, route, data-access, messaging, infrastructure, WPF, graph, and query surfaces

Support matrix audit summary:

| Area | Phase 17 level |
|---|---|
| Rust | Good |
| JavaScript / TypeScript / JSX / TSX | Basic static/local |
| C# / ASP.NET Core, Go, Python, Java, Kotlin, PHP, Ruby | Basic static/local |
| Node REST, React, Next.js, Angular, WPF/XAML | Basic static/local |
| ORM/data-access, realtime, messaging, infrastructure | Basic static/local |
| C / C++ / Swift / Objective-C / Dart / Flutter | Basic static/local |
| YAML / JSON / TOML / XML / HTML / CSS / SCSS / SQL / ksqlDB / env | Basic static/local |
| Three.js / WebGL | Basic static hints only |
| Cross-project route/message/dependency matching, group impact, graph/service-map API | Basic local/read-only |
| Symbolic editing and rename/refactor | Conservative local Control API MVP |
| Architecture graph UI, full Git Intelligence, broad refactor engine | Unsupported/deferred |

Phase 17 does not add compiler-grade parsing, runtime validation, package-manager execution, cloud/external APIs, telemetry, browser/WebGL execution, Kafka/ksqlDB/RabbitMQ/database/broker connections, mandatory LSP, architecture graph UI, full Git Intelligence, or broad refactor behavior.

### Future Hardening: RabbitMQ Advanced Messaging Intelligence

Goal:

Improve B3's static cross-repo RabbitMQ/message-flow understanding beyond simple name matching.

Scope:

- model RabbitMQ topology statically:
  - Producer
  - Exchange
  - Binding
  - Queue
  - Consumer
- extract and connect:
  - exchange name
  - exchange type: direct, topic, fanout, headers, unknown
  - queue name
  - binding key / routing key
  - producer publish exchange + routing key
  - consumer queue/handler
- add deterministic matching for:
  - direct exchange routing-key match
  - topic exchange wildcard matching using `*` and `#`
  - fanout exchange through explicit bindings
  - headers exchange only when literal header arguments are visible
- classify retry/dead-letter flows:
  - `DeadLettersTo`
  - `RetriesThrough`
  - primary business message flow vs infrastructure retry/DLQ flow
- add local static resolution for:
  - constants
  - enums/static fields
  - local config files
  - `appsettings.json`
  - yaml/json config
  - `.env.example`/default values
  - docker-compose environment literals
  - Kubernetes ConfigMap names/literals where safe
- add wrapper/event-bus pattern detection:
  - `EventBus.PublishAsync(...)`
  - `rabbitPublisher.publish(...)`
  - `ClientProxy.emit/send` wrappers
  - message class/type name inference where safe
- produce better evidence chains:
  - Producer -> Exchange -> Binding -> Queue -> Consumer
- add RabbitMQ-specific confidence scoring:
  - high for literal exchange/routing key/binding/consumer queue
  - medium for local config/const resolution
  - medium/low for topic wildcard matches
  - low for broad wildcard or unknown broker
  - skip dynamic runtime-only values
- keep output deterministic, bounded, and evidence-based

Out of scope:

- connecting to RabbitMQ brokers
- inspecting live exchanges/queues
- publishing test messages
- consuming messages
- requiring Docker Compose or Kubernetes runtime
- calling cloud RabbitMQ services
- calling external APIs
- requiring internet
- telemetry
- runtime service discovery

Important:

This is static/local/offline intelligence only. It must preserve B3's offline-first and free-by-default requirement.

---

## Phase 18 Refactor Checkpoint D

Status: Completed.

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

### Phase 18.1 Test Organization Split

Status: Completed.

Completed scope:

- split the large `crates/b3-indexer/src/tests.rs` body into domain-focused child test modules under `crates/b3-indexer/src/tests/`
- kept shared in-memory test store, event bus, and failing parser helpers in the test module root
- did not change production logic, API behavior, database schema, metadata formats, MCP tools/profiles, language support levels, or benchmark targets
- left control and storage embedded tests untouched for later checkpoints because the indexer split was the primary safe target
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, browser/WebGL runtime, or frontend changes

### Phase 18.2 Control Server Route Module Split

Status: Completed.

Completed scope:

- split `crates/b3-control/src/lib.rs` route registration and HTTP handlers into focused `crates/b3-control/src/routes/` modules
- kept `lib.rs` responsible for public crate exports, shared state, server startup, shared helpers, and router glue
- preserved endpoint paths, request/response DTOs, status semantics, capability/language reporting, and production behavior
- did not change database schema, migrations, metadata formats, MCP tools/profiles, benchmark targets, frontend files, or feature behavior
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, browser/WebGL runtime, or frontend checks required

### Phase 18.3 Storage Module Split

Status: Completed.

Completed scope:

- split migration SQL and migration application helpers from `crates/b3-storage/src/lib.rs` into `crates/b3-storage/src/migrations.rs`
- split storage metadata row conversion and metadata decode helpers into `crates/b3-storage/src/metadata.rs`
- preserved database schema, migration numbers, SQL behavior, metadata formats, vector storage format, public storage APIs, and repository behavior
- did not change API behavior, MCP tools/profiles, benchmark targets, frontend files, or feature behavior
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, browser/WebGL runtime, or frontend checks required

### Phase 18.4 Indexer Pipeline / Dispatch Split

Status: Completed.

Completed scope:

- split `crates/b3-indexer/src/lib.rs` into focused indexer modules for language dispatch, path/language detection, parser worker isolation, local indexing pipeline orchestration, and metadata string helpers
- kept public `b3-indexer` exports compatible through `lib.rs` re-exports for existing callers
- preserved language/path detection behavior, `DefaultLanguagePack` dispatch order, parser worker timeout/error/retry semantics, scoped indexing behavior, invalid UTF-8 skip behavior, extraction metadata formats, and symbol/route extraction behavior
- did not change database schema, migration numbers, control APIs, storage APIs, MCP tools/profiles, benchmark targets, language support levels, frontend files, or feature behavior
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, browser/WebGL runtime, or frontend checks required

### Phase 18.5 Shared Helper Consolidation

Status: Completed.

Completed scope:

- consolidated duplicated indexer metadata escaping and prefixed metadata lookup helpers into `crates/b3-indexer/src/metadata_helpers.rs`
- consolidated duplicated SQL/ksqlDB line-comment stripping and statement line lookup helpers into `crates/b3-indexer/src/data_files/mod.rs`
- kept existing local metadata accessor wrapper names for test and module compatibility
- preserved metadata formats, escaping/unescaping semantics, redaction/value classification behavior, comment stripping behavior, literal extraction behavior, confidence values, language support levels, APIs, schema, migrations, MCP tools/profiles, and benchmark targets
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, browser/WebGL runtime, or frontend checks required

### Phase 18.6 Optional Core / Query Architecture Split Review

Status: Completed as review-only / no-op.

Review result:

- reviewed `crates/b3-core/src/language.rs`, `crates/b3-core/src/architecture.rs`, `crates/b3-query/src/lib.rs`, and the large `crates/b3-query/src/architecture/*` modules
- deferred source refactor because the likely split points are public DTO/contract surfaces or behavior-sensitive matching, graph, service-map, and group-impact builders
- did not change core contracts, query architecture behavior, confidence/evidence semantics, response shapes, database schema, metadata formats, MCP tools/profiles, benchmark targets, or feature behavior
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, or runtime service connections

### Phase 18.7 Preliminary Refactor Checkpoint Verification

Status: Completed, superseded by Phase 18.9 final verification.

Completed scope:

- ran final Phase 18 verification across formatting, workspace compilation, workspace tests, and the benchmark baseline
- confirmed Phase 18 was behavior-preserving: no schema migration, endpoint behavior change, metadata format change, MCP tool/profile change, language support-level change, benchmark target change, or feature expansion
- confirmed API capability/language tests, storage migration/metadata tests, MCP profile/tool-count tests, indexer parser/scoped/language tests, and cross-project benchmark sections still pass
- kept core/query architecture split deferred for a later contract-aware review if needed
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, mandatory LSP, or frontend checks required

### Phase 18.8 b3-indexer Deep Restructure

Status: Completed.

Completed scope:

- scanned `crates/b3-indexer/src/**` for largest modules, oversized tests, mixed responsibilities, duplicate helpers, and safe split candidates
- split the oversized `crates/b3-indexer/src/tests/web.rs` module into focused web test modules for core web language behavior, Angular, Node REST, Next.js, React, and web language pack/indexer behavior
- deferred production splits in `go.rs`, `csharp.rs`, `data_access/mod.rs`, `web/angular.rs`, and other extraction-heavy modules because they are more behavior-sensitive than the test-only split
- preserved extraction behavior, language/path detection, dispatch order, parser isolation, scoped indexing, metadata formats, confidence values, API behavior, schema, MCP tools/profiles, benchmark targets, and feature scope
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, mandatory LSP, or frontend checks required

### Phase 18.9 Final Phase 18 Verification

Status: Completed.

Completed scope:

- ran final Phase 18 verification after the Phase 18.8 b3-indexer test restructure across formatting, workspace compilation, workspace tests, and the benchmark baseline
- confirmed Phase 18 was behavior-preserving: no schema migration, endpoint behavior change, metadata format change, MCP tool/profile change, language support-level change, benchmark target change, or feature expansion
- confirmed storage schema, migration numbers, SQL definitions, metadata formats, and vector storage formats were unchanged
- confirmed indexing behavior remained unchanged for language/path detection, dispatch order, parser isolation, scoped indexing, invalid UTF-8/binary skip behavior, and metadata/extraction behavior
- kept the Phase 18.6 core/query architecture split deferred for a later contract-aware review if needed
- kept production extraction-heavy b3-indexer modules such as `data_access/mod.rs`, `web/angular.rs`, `backend_languages/mod.rs`, `go.rs`, and `csharp.rs` deferred for a later targeted behavior-preserving pass
- preserved offline/free behavior: no package managers, external APIs, telemetry, Docker/Kubernetes/Terraform, brokers, databases, Kafka/ksqlDB/RabbitMQ, mandatory LSP, or frontend checks required

Next: Phase 19 Performance Optimization Pass B.

---

## Phase 19 Performance Optimization Pass B

Status: Current / Next.

Rules:

- benchmark first
- optimize measured bottlenecks only
- no telemetry
- no unrelated refactor

---

## Phase 20 Web UI Developer Console Refresh
Status: Planned.

### Scope:

- refresh local Web UI developer console with improved debugging views
- prioritize local/offline data sources only
- no cloud telemetry or required external services


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
- `GET /api/wpf`

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

### Rust Good

Backend:

- `tree-sitter-rust`

Capabilities:

- file detection
- parsing
- symbol extraction
- import extraction
- basic relationship extraction

### JavaScript Basic

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

### TypeScript Basic

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

### JSX / TSX Basic

Backend:

- JavaScript/TypeScript tree-sitter grammars

Capabilities:

- `.jsx`, `.tsx` detection
- parsing
- component-like declarations where structurally obvious
- JSX component usages where safe
- imports / exports
- conservative relationships

### C# Basic Static

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
â”œâ”€â”€ Backend API
â”œâ”€â”€ Frontend App
â”œâ”€â”€ Worker Service
â”œâ”€â”€ Desktop Client
â””â”€â”€ Runtime Infrastructure
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

## Phase 9.2.1 Node.js / REST API Intelligence

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

## Phase 9.2.2 React / TSX Component Intelligence

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

- Phase 9.2.3 Next.js Intelligence
## Previously Deferred -> Scheduled Phases (12â€“20)

The items previously listed under an older deferred block have been renumbered and scheduled in the Phase 12â€“20 sequence below.

## Phase 12 Symbolic Editing MVP
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

## Phase 9.2.3.1 Indexer Module Split / Refactor Checkpoint B

Status: Completed as a behavior-preserving refactor checkpoint.

The indexer keeps orchestration and shared contracts in `crates/b3-indexer/src/lib.rs`.

Web-language extraction moved under `crates/b3-indexer/src/web/`, and the large inline indexer test module moved to `crates/b3-indexer/src/tests.rs`.

No runtime behavior, storage schema, control API response, MCP tool/profile, or dependency changes are part of this checkpoint.

---

## Phase 9.2.4 Angular Intelligence

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

## Phase 9.2.4.1 Web Module Split / Refactor Checkpoint C

Status: Completed as a behavior-preserving refactor checkpoint.

`crates/b3-indexer/src/web/mod.rs` is now a small orchestration and re-export layer.

Existing JS/TS symbol extraction, Node REST routes, React component metadata, Next.js routes/config detection, shared route/component metadata, and tree-sitter helpers were split into focused web modules.

No runtime behavior, storage schema, control API response, MCP tool/profile, dependency, or Web UI behavior changed.

---

## Phase 9.2.5 ASP.NET Core / C# Web API Intelligence

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

## Phase 9.2.6 ORM / Database Access Intelligence

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

## Phase 9.2.7 Realtime / Socket Intelligence

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

## Phase 9.2.8 Messaging / Event-driven Intelligence

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

## Phase 9.2.9 Cloud / Infrastructure Intelligence

Status: Completed.

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

## Phase 9.2.10 Go Language Support

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

## Phase 9.2.11 Scoped Indexing + Intelligence Targets

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

## Phase 9.2.12 .NET Desktop / WPF Intelligence

Status: Completed.

### Scope

- Modern SDK-style WPF project detection from `<UseWPF>true</UseWPF>`,
  WindowsDesktop SDK, `net*-windows`, and `WinExe` hints.
- Older .NET Framework WPF project detection from PresentationCore,
  PresentationFramework, WindowsBase, System.Xaml, `Page`, and
  `ApplicationDefinition` project metadata.
- XAML Application, Window, UserControl, Page, ResourceDictionary, and
  NavigationWindow detection.
- `x:Class`, obvious `.xaml.cs` code-behind path hints, static DataContext
  hints, ViewModel naming hints, binding paths, command bindings,
  StaticResource/DynamicResource keys, resource definitions, and
  ResourceDictionary source extraction.
- Read-only WPF metadata exposure through `GET /api/wpf`.
- Scoped indexing integration for `language:xaml`, `framework:wpf`, and
  `framework:dotnet_desktop`.

### Out of Scope

- Visual Studio automation
- MSBuild or `dotnet` execution
- XAML compiler/designer integration
- XAML runtime execution
- full WPF binding type checking
- runtime DataContext inference
- deep MVVM framework analysis
- symbolic editing, rename/refactor, embeddings, semantic search, and
  cross-project architecture intelligence

---

## Current Roadmap

Completed:

- Phase 10.0 - Local Embeddings + Vector Search Architecture
- Phase 10.1 - Local Embedding Provider MVP
- Phase 10.2 - SQLite Vector Storage / Search Index
- Phase 10.3 - Hybrid Search Ranking
- Phase 10.4 - MCP / Control API Integration
- Phase 10.5 - Benchmark + Quality Evaluation
- Phase 11.0 - Cross-Project Architecture Model + Contracts
- Phase 11.1 - Group Query Federation
- Phase 11.1.1 - Context Efficiency + Tool Call Reduction Benchmark
- Phase 11.2 - Cross-Repo Route / API Matching
- Phase 11.3 - Cross-Repo Messaging Matching
- Phase 11.4 - Cross-Repo Package / Contract / Infra Matching
- Phase 11.5 - Group-Level Impact + Context Pack
- Phase 11.6 - Architecture Graph / Service Map API
- Phase 11.7 - Cross-Project Benchmark + Docs
- Phase 12 - Symbolic Editing MVP
- Phase 13 - Rename / Refactor MVP
- Phase 14 - Additional Backend Language Support
- Phase 15 - Systems / Mobile / Config / Web File Support A
- Phase 16 - Config / Data / Web File Support B / Hardening
- Phase 17 - Language and Technology Quality Audit
- Phase 18.1 - Test Organization Split
- Phase 18.2 - Control Server Route Module Split
- Phase 18.3 - Storage Module Split
- Phase 18.4 - Indexer Pipeline / Dispatch Split
- Phase 18.5 - Shared Helper Consolidation
- Phase 18.6 - Optional Core / Query Architecture Split Review
- Phase 18.7 - Preliminary Refactor Checkpoint Verification
- Phase 18.8 - b3-indexer Deep Restructure
- Phase 18.9 - Final Phase 18 Verification

Current/Next:

- Phase 19 - Performance Optimization Pass B

Upcoming:

- Phase 20 - Web UI Developer Console Refresh
- Phase 21 - Git Intelligence

## Phase 10.0 - Local Embeddings + Vector Search Architecture

Status: Completed.

Purpose:

Add the local/offline architecture foundation for embeddings and vector search without making semantic search usable yet.

Scope completed:

- embedding provider contracts and truthful provider capabilities
- offline/free embedding config defaults with embeddings disabled by default
- vector document, source kind, metadata, embedding vector, search request/result, and store traits
- deterministic chunk planning with symbol-level preference and file-level fallback
- SQLite-compatible vector document and embedding vector tables using normal SQLite storage
- read-only control endpoints: `GET /api/vector/status` and `GET /api/vector/stats`
- tests for defaults, provider metadata, chunking, vector hashes, SQLite upsert/search/cleanup, and control status

Rules:

- no OpenAI, Anthropic, Gemini, or cloud embedding API integration
- no hosted vector database requirement
- no model download or tokenizer requirement
- no Qdrant requirement
- no semantic search MCP tool
- no hybrid ranking
- no telemetry, SaaS auth, API keys, or internet requirement

---

## Phase 10.1 - Local Embedding Provider MVP

Status: Completed.

Scope completed:

- added `local_hash`, a deterministic lexical/hash embedding provider
- default embedding config names `local_hash` but keeps embeddings disabled
- added an offline provider registry and config-to-provider construction
- added batch document embedding from planned chunks to `EmbeddingVector` records
- added L2 normalization, dot product, cosine similarity, and dimension validation helpers
- added read-only `GET /api/vector/providers`
- updated `GET /api/vector/status` to report local provider availability and readiness truthfully

Rules:

- no OpenAI/cloud embedding API integration
- no hosted vector database requirement
- no model download or tokenizer requirement
- no Qdrant requirement
- no semantic search MCP tool
- no hybrid ranking
- no telemetry, SaaS auth, API keys, or internet requirement

---

## Phase 10.2 - SQLite Vector Storage / Search Index

Status: Completed.

Scope completed:

- added migration v5 for durable `embedding_vectors` rows keyed by document/provider/dimension
- validated little-endian `Vec<f32>` BLOB encoding/decoding with finite-value checks
- hardened vector document/vector upsert and dedupe behavior
- added cleanup by file and project/branch with vector cascade behavior
- added local brute-force cosine search over SQLite-filtered candidates
- added provider, dimension, language, framework, source kind, file, symbol, and path-prefix filters
- added deterministic search tie-breaking and sane limit handling
- expanded vector stats with providers, dimensions, source kinds, languages, and frameworks
- updated control status/stats to report storage/search readiness truthfully
- verified local_hash vectors can persist to SQLite and be searched locally

Rules:

- no native SQLite vector extension requirement
- no hosted vector database requirement
- no OpenAI/cloud embedding API integration
- no semantic search MCP tool
- no hybrid ranking
- no telemetry, SaaS auth, API keys, model downloads, or internet requirement

---

## Phase 10.3 - Hybrid Search Ranking

Status: Completed.

Scope completed:

- added reusable `b3-query::hybrid` ranking module
- added `HybridSearchRequest`, `HybridSearchResult`, compact explanations, and warnings
- combined local lexical token overlap, SQLite vector cosine scores, and metadata boosts
- added deterministic score normalization, weight validation, fallback behavior, and tie-breaking
- added local_hash query embedding for vector scoring without external calls
- added lexical-only, vector-only, merged, filtered, and explained ranking tests

Rules:

- no MCP semantic search tool
- no full control semantic integration
- no benchmark/quality dataset
- no hosted vector database requirement
- no OpenAI/cloud embedding API integration
- no telemetry, SaaS auth, API keys, model downloads, or internet requirement

---

## Phase 10.4 - MCP / Control API Integration

Status: Completed.

Scope completed:

- added read-only `POST /api/search/hybrid` for local hybrid search
- updated vector/capability status to report local hybrid readiness truthfully
- added MCP `semantic_search` as a thin adapter over `b3-query`
- exposed `semantic_search` in optimized, full, debug, readonly, editing, web-app, and enterprise profiles
- kept `tiny` intentionally small without `semantic_search`
- added request validation for query, limit, weights, min score, source kind, and path prefix
- preserved lexical/metadata fallback when vector data is unavailable
- supported compact explanations through both Control API and MCP

Rules:

- no benchmark/quality claims
- no cross-project semantic search
- no hosted vector database requirement
- no OpenAI/cloud embedding API integration
- no telemetry, SaaS auth, API keys, model downloads, or internet requirement

---

## Phase 10.5 - Benchmark + Quality Evaluation

Status: Completed.

Scope completed:

- added deterministic local `semantic_search_repo` fixture covering Rust, TypeScript, React, Next.js, Angular, ASP.NET Core/C#, Go, WPF/XAML, messaging, realtime, infrastructure, and data access examples
- added fixture-based evaluation queries with expected files, optional symbols, source kinds, languages, frameworks, and notes
- added `semantic_quality` baseline JSON with hit@1/3/5/10, MRR, score, latency, fallback, source-kind, file-match, and symbol-match metrics
- compared lexical-only, vector-only, and hybrid modes without changing ranking defaults
- added MCP `semantic_search` and Control `POST /api/search/hybrid` guardrail checks
- added conservative token/context savings estimates using chars/4 approximation
- added human-readable semantic quality summary to `b3-bench baseline`
- added tests for fixture loading, expected targets, metrics, offline benchmark execution, and Phase 10.4 MCP profile counts

Current local fixture baseline:

- lexical-only: hit@1 0.100, hit@3 0.100, MRR 0.100
- vector-only: hit@1 0.900, hit@3 1.000, MRR 0.950
- hybrid: hit@1 0.900, hit@3 1.000, MRR 0.950
- token estimate: 2,843 naive chars vs 1,962 selected chars, about 31% estimated reduction

These are fixture baselines only. They are not neural semantic quality claims or production guarantees.

Rules:

- no benchmark upload
- no cross-project semantic search
- no hosted vector database requirement
- no OpenAI/cloud embedding API integration
- no neural embedding provider
- no telemetry, SaaS auth, API keys, model downloads, or internet requirement

---

## Phase 11.0 - Cross-Project Architecture Model + Contracts

Status: Completed.

Scope completed:

- added `b3-core` architecture DTOs for project/group identity, service identity, architecture nodes, architecture edges, future match candidates, confidence, evidence, warnings, and provenance
- added deterministic ID helpers for services, nodes, edges, and match candidates
- added deterministic normalization helpers for HTTP methods/routes, messaging keys, package names, infrastructure resource keys, and service names
- added `ArchitectureCapabilityStatus` and read-only `GET /api/architecture/status`
- updated `GET /api/capabilities` to report architecture contracts available while future matching/federation/service-map flags were still false at Phase 11.0
- preserved the local model: 1 project = 1 repo-local `.b3/b3.db`

Rules:

- no group query federation
- no cross-repo route/API matching
- no cross-repo messaging matching
- no package/contract/infra matching
- no group-level impact or context pack
- no architecture graph UI or service map API
- no global DB merge
- no cloud graph database, hosted vector database, external API, telemetry, or internet requirement
- no MCP architecture tool or MCP tool count change

## Phase 11.1 - Group Query Federation

Status: Completed.

Scope completed:

- added `b3-query::architecture` group federation over local registry JSON
- added registry group resolution, deterministic project ordering, project DB status, partial-result warnings, and read-only project handles
- added read-only SQLite opening for existing project DBs without creating or migrating missing DBs
- added group context and summary DTOs with route/component/data-access/realtime/messaging/infrastructure/WPF/vector counts
- added federated metadata helpers for routes, components, data access, realtime, messaging, infrastructure, and WPF, with project identity attached to each result
- added read-only control endpoints: `GET /api/architecture/groups`, `GET /api/architecture/groups/{group_id}/status`, and `GET /api/architecture/groups/{group_id}/summary`
- updated architecture status/capabilities to report group federation ready while matching, group impact, and service map remain false
- preserved one repo-local `.b3/b3.db` per project and avoided global database merging

Rules:

- no cross-repo route/API matching
- no cross-repo messaging matching
- no package/contract/infra matching
- no group-level impact or context pack
- no architecture graph UI or service map API
- no global DB merge
- no cloud graph database, hosted vector database, telemetry, external API, or internet requirement
- no MCP architecture tool or MCP tool count change

## Phase 11.1.1 - Context Efficiency + Tool Call Reduction Benchmark

Status: Completed.

Scope completed:

- added `b3-bench::efficiency` as a local/offline benchmark module over the checked-in `semantic_search_repo` fixture
- defined deterministic `naive_file_by_file`, `search_code_only`, `semantic_search_only`, `semantic_search_context_pack`, and `group_federated_summary` workflow models
- added minimal, balanced, and deep context profiles for context-pack-style selection
- added task-level expected files, optional symbols, source kinds, answer facts, and coverage tags
- added chars/4 token estimates, deterministic tool-call estimates, target comparison objects, warnings, limitations, and JSON serialization under `efficiency_metrics`
- added human-readable efficiency output to `cargo run -p b3-bench -- baseline`
- verified `semantic_search`, `get_context_pack`, and Phase 10.4 MCP profile counts without adding MCP tools

Current local fixture baseline:

- selected comparison: `semantic_search_context_pack:balanced`
- token reduction multiplier: about `1.11x` versus the `10.0x` target, not met
- tool-call reduction multiplier: about `4.05x` versus the `2.1x` target, met by the deterministic model
- answer-quality approximation: about `0.699` versus the documented `0.8` target, not met
- prior Phase 10.5 context baseline remains about `31%` estimated token reduction, or about `1.45x` fewer tokens

Rules:

- benchmark/measurement only
- no cross-repo route/API matching
- no cross-repo messaging matching
- no package/contract/infra matching
- no group-level impact or context pack
- no architecture graph UI or service map API
- no symbolic editing or rename/refactor
- no neural embedding provider, hosted vector database, cloud embeddings, external APIs, telemetry, paid dependency, model download, or internet requirement

## Phase 11.2 - Cross-Repo Route / API Matching

Status: Completed.

Scope completed:

- added modular route/API matching in `b3-query::architecture`
- added deterministic HTTP route match keys with method uppercase, slash/trailing-slash normalization, query removal, and `:id` / `[id]` / `<id>` parameter normalization
- collected server routes from existing federated route metadata and classified backend API endpoints separately from frontend page routes
- added conservative local static HTTP client literal extraction for JS/TS `fetch`, Axios/member clients, Angular-style `HttpClient`, C# `HttpClient`, and Go `http` calls
- supported literal local base URL/prefix composition without resolving runtime env vars, secrets, DNS, or HTTP
- matched exact method/path, unknown-method exact path, method plus route pattern, and same-path/different-method low-confidence candidates
- produced `ArchitectureMatchCandidate`, `ArchitectureNode`, `ArchitectureEdge`, confidence, evidence, warnings, and deterministic sorting/dedupe
- added read-only Control endpoint `GET /api/architecture/groups/{group_id}/route-matches`
- updated architecture status/capabilities so route matching is ready while messaging, package/contract/infra, group impact, and service maps remain false
- preserved one repo-local `.b3/b3.db` per project and avoided global database merging

Rules:

- no cross-repo messaging matching
- no package/contract/infra matching
- no group-level impact or context pack
- no architecture graph UI or service map API
- no runtime HTTP calls or remote OpenAPI fetch
- no cloud graph database, hosted vector database, external API, telemetry, paid dependency, model download, or internet requirement

## Phase 11.3 - Cross-Repo Messaging Matching

Status: Completed.

Scope completed:

- added modular messaging matching in `b3-query::architecture`
- added deterministic messaging keys for broker, channel kind, and normalized topic/queue/pattern/routing-key names
- collected producers and consumers from existing federated messaging metadata
- matched same broker plus exact channel kind/name, compatible topic/queue/pattern names, unknown broker exact names, NestJS pattern names, and same-name conflicting broker cases
- current Phase 11.3 messaging matching supports static name/key matching; advanced RabbitMQ topology matching is deferred to RabbitMQ Advanced Messaging Intelligence
- produced `ArchitectureMatchCandidate`, `ArchitectureNode`, `ArchitectureEdge`, confidence, evidence, warnings, and deterministic sorting/dedupe
- added read-only Control endpoint `GET /api/architecture/groups/{group_id}/message-matches`
- updated architecture status/capabilities so messaging matching is ready while package/contract/infra, group impact, and service maps remain false
- preserved one repo-local `.b3/b3.db` per project and avoided global database merging

Rules:

- no package/contract/infra matching
- no group-level impact or context pack
- no architecture graph UI or service map API
- no broker connection, runtime publish/consume, or cloud Pub/Sub API calls
- no cloud graph database, hosted vector database, external API, telemetry, paid dependency, model download, or internet requirement

## Phase 11.4 - Cross-Repo Package / Contract / Infra Matching

Status: Completed.

Scope completed:

- added deterministic package/dependency keys for npm, .NET, Go, Rust, Python, and unknown ecosystems
- added deterministic contract/schema keys for DTO/model/interface/type/enum and local OpenAPI, GraphQL, protobuf, Avro, and JSON schema names
- added deterministic infrastructure keys for Docker Compose services/images, Kubernetes services/deployments/configmaps/secrets, Terraform resources/modules, databases, caches, queues, Pub/Sub, and unknown resources
- matched local package providers to dependency consumers, including package.json names/dependencies, .NET PackageReference/ProjectReference, Go module/require prefixes, and Rust crate dependencies where local manifest content is indexed
- matched exact shared contract/schema names across projects, with generic names such as `User`, `Request`, `Response`, `Model`, `Item`, and `Data` kept low confidence unless stronger evidence is added later
- matched infrastructure relationships from existing metadata, including Docker Compose `depends_on`, Kubernetes Service selector to Deployment labels, image/name overlap, and Terraform module/resource name overlap
- produced `ArchitectureMatchCandidate`, `ArchitectureNode`, `ArchitectureEdge`, confidence, compact evidence, warnings, deterministic IDs, deterministic sorting, and dedupe for package, contract, and infrastructure matches
- added read-only Control endpoint `GET /api/architecture/groups/{group_id}/dependency-matches` with filters for kind, ecosystem, contract kind, infra kind, name, source project, target project, confidence, limit, and branch
- updated architecture status/capabilities so package/contract/infra matching is ready while group impact/context pack and service maps remain false
- counted indexed files in federated metadata readiness so manifest/schema-only project DBs can participate without requiring route/component/infra records
- preserved one repo-local `.b3/b3.db` per project and avoided global database merging

Rules:

- no group-level impact or context pack
- no architecture graph UI or service map API
- no symbolic editing or rename/refactor
- no package manager execution, dependency restore, lockfile resolution by tools, or registry access
- no Docker, Docker Compose, kubectl, Terraform, gcloud, cloud provider, broker, or database server execution/connection
- no remote OpenAPI/GraphQL/schema parsing, schema compatibility validation, cloud graph database, hosted vector database, telemetry, paid dependency, model download, external API, or internet requirement

## Phase 11.5 - Group-Level Impact + Context Pack

Status: Completed.

Scope completed:

- added `b3-query::architecture::group_impact` for static/read-only group impact traversal
- added request validation for seed type, relative paths, direction, depth, limit, context profile, optional confidence threshold, and context-pack inclusion
- resolved seeds for route, message, package, contract, infrastructure, file, symbol, and query against local architecture nodes from existing match candidates
- reused Phase 11.2 route matches, Phase 11.3 messaging matches, and Phase 11.4 dependency matches without adding extraction or runtime discovery
- added bounded deterministic traversal over existing cross-project match candidates, with deduped nodes/edges/paths, confidence propagation, evidence summaries, warnings, and project summaries
- added minimal, balanced, and deep cross-repo context pack profiles with bounded char budgets, chars/4 token estimates, sectioned summaries, deduped snippets, skipped-item reporting, and truncation reasons
- added read-only Control endpoint `POST /api/architecture/groups/{group_id}/impact`
- updated architecture status/capabilities so group impact and group context pack are ready while service maps remain false
- preserved one repo-local `.b3/b3.db` per project and avoided global database merging

Rules:

- no architecture graph UI or service map API
- no Phase 11.7 cross-project benchmark/docs expansion
- no symbolic editing or rename/refactor
- no package manager execution, Docker/Kubernetes/Terraform/cloud CLI execution, runtime HTTP calls, broker connections, cloud APIs, schema compatibility validation, telemetry, hosted vector database, cloud graph database, external API, paid dependency, model download, or internet requirement

## Phase 11.6 - Architecture Graph / Service Map API

Status: Completed.

Scope completed:

- added `b3-query::architecture::architecture_graph` for static/read-only graph construction
- added deterministic architecture graph request/response models with project, relationship-kind, confidence, evidence, unresolved, seed, node, edge, and limit filters
- built graph nodes and edges on demand from Phase 11.2 route matches, Phase 11.3 messaging matches, Phase 11.4 dependency matches, and Phase 11.1 group summaries
- added project/service/resource summaries, unresolved relationship reporting, confidence distribution, relationship-kind counts, connected-project ranking, and isolated-project reporting
- added project-level service map summaries and service-to-service edges from existing match evidence
- added read-only Control endpoints `GET /api/architecture/groups/{group_id}/graph` and `GET /api/architecture/groups/{group_id}/service-map`
- updated architecture status/capabilities so service map and architecture graph API are ready while architecture graph UI remains false
- preserved one repo-local `.b3/b3.db` per project and avoided global database merging or graph persistence

Rules:

- no architecture graph UI or service map UI
- no Phase 11.7 cross-project benchmark/docs expansion
- no symbolic editing or rename/refactor
- no package manager execution, Docker/Kubernetes/Terraform/cloud CLI execution, runtime HTTP calls, broker connections, cloud APIs, graph database, hosted vector database, schema compatibility validation, telemetry, external API, paid dependency, model download, or internet requirement

## Phase 11.7 - Cross-Project Benchmark + Docs

Status: Completed.

Scope completed:

- added `b3-bench::architecture` for Phase 11 cross-project benchmark coverage
- added a deterministic local architecture fixture group for frontend, API, worker, and shared package projects
- benchmarked group federation, route/API matching, messaging matching, dependency matching, group impact, cross-repo context pack, architecture graph, and service map behavior
- added `cross_project_benchmark` to `target/benchmarks/baseline.json` without removing `semantic_quality` or `efficiency_metrics`
- parsed `benchmarks/b3.benchmark.toml` with local/offline guardrails and optional project handling
- treated configured optional local benchmark repositories as optional local candidates; missing or unindexed paths warn and skip
- added branch-safety reporting as benchmark/docs warnings only; full Git Intelligence remains Phase 21
- documented current benchmark methodology, limitations, target comparisons, and offline/free behavior
- clarified that Phase 11.7 is local fixture/local-repo benchmarking only, not a 31 public real-world repository claim

Current measured fixture/local run:

- route matches: 1
- message matches: 1
- dependency matches: 1
- impact successes: 2
- graph nodes: 10
- graph edges: 3
- services: 4
- architecture target comparison in the local fixture run: token reduction `25.42x` against `10.0x`, tool-call reduction `2.57x` against `2.1x`, deterministic task quality `1.000` against `0.8`
- warnings reported for configured optional local benchmark DBs that were not present
- after switching branches, users should reindex before comparing results until branch-aware indexing is implemented

Rules:

- no architecture graph UI
- no symbolic editing or rename/refactor
- no full Git Intelligence
- no package manager execution, Docker/Kubernetes/Terraform/cloud CLI execution, runtime HTTP calls, broker connections, cloud APIs, graph database, hosted vector database, cloud embeddings, telemetry, external API, paid dependency, model download, or internet requirement

---

## Phase 21 Git Intelligence

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

## Phase 22 Duplicate / Similarity Detection

Status: Planned.

Scope:

- AST fingerprinting
- normalized AST hash
- MinHash
- SimHash
- duplicate function detection
- similar code search

---

## Phase 23 Real Plugin System

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

## Phase 24 Packaging + Installers

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
| Docker / Docker Compose / Kubernetes / GCP / GKE / Terraform | Usable now, basic/static |
| Go language support | Usable now, basic/static |
| Python / Java / Kotlin / PHP / Ruby backend basics | Usable now, basic/static |
| Scoped indexing targets | Usable now |
| C# WPF / XAML | Usable now, basic/static |
| Three.js / WebGL | Usable now, static hints only |
| Registry Web UI | Deferred |
| Control registry APIs | Deferred |
| Symbolic editing preview/apply | Usable now, local Control API MVP |
| Refactor assistant | Usable now, local bounded rename/refactor MVP |
| Local embeddings / vector search | Phase 10.0-10.5 |
| Cross-project architecture contracts | Usable now, model/status only |
| Cross-project group federation | Usable now, read-only summaries |
| Cross-project route/API matching | Usable now, local/static/read-only |
| Cross-project messaging matching | Usable now, local/static/read-only static name/key matching; advanced RabbitMQ topology matching deferred |
| Cross-project package/contract/infra matching | Usable now, local/static/read-only |
| Group impact/context pack | Usable now, local/static/read-only |
| Architecture graph/service map API | Usable now, local/static/read-only |
| Cross-project architecture benchmark | Usable now, local fixture/optional repo baseline |
| Full memory/context platform | Later phase |
| Release-grade packaging | Phase 24 |

B3 can run today as a local MCP/runtime/control/UI platform with Rust, basic JS/TS/JSX/TSX indexing, basic static Node.js REST route intelligence, basic static React/TSX component intelligence, basic static Next.js route/boundary intelligence, basic static Angular metadata, basic static ASP.NET Core / C# Web API route intelligence, basic static ORM/database access metadata, basic static realtime/socket metadata, basic static messaging/event-driven metadata, basic static cloud/infrastructure metadata, basic static Go language support, scoped indexing, and basic static WPF/XAML intelligence.

Local embeddings and vector search progress through Phase 10.0-10.5.
Cross-project architecture contracts begin in Phase 11.0; read-only group federation begins in Phase 11.1; local route/API matching begins in Phase 11.2; local messaging matching begins in Phase 11.3 with static name/key matching; local package/contract/infra matching begins in Phase 11.4; local group impact/context pack begins in Phase 11.5; local architecture graph/service map APIs begin in Phase 11.6; local cross-project architecture benchmark/docs begin in Phase 11.7. Advanced RabbitMQ topology matching is deferred to RabbitMQ Advanced Messaging Intelligence. Architecture graph UI remains deferred.

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

Benchmark fixtures must remain local, deterministic, small enough to commit, and free from network/cloud/API calls. The default broader local benchmark config is `benchmarks/b3.benchmark.toml`; it may name optional real-local repositories, but missing optional paths must produce warnings rather than failures and must not be required by `cargo test --workspace`.

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
- Python
- Java
- Kotlin
- PHP
- Ruby
- C
- C++
- Swift
- Objective-C
- Dart / Flutter
- YAML
- JSON
- HTML
- CSS / SCSS
- TOML
- XML
- XAML
- Three.js / WebGL
- ksqlDB

Then:

- SQL
- deeper config/data/web hardening from Phase 16

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

### Persistent Tree-Sitter Code Memory

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

Covered by a later memory/context phase after Phase 10 vector search work.

### Token Savior

B3 learns structural MCP navigation, MCP tool profiles, manifest slimming, command output compaction, benchmark-driven development, transcript discovery, and install helper ideas.

Covered by Phase 8.2, Phase 8.5, Phase 8.6, Phase 8.7, and Phase 10.3.

### CodeGraph

B3 learns broad multi-language graph MCP, framework-aware route detection, with/without MCP benchmarks, and agent steering.

Covered by Phase 8.2, Phase 9.0+, Phase 9.2+, and Phase 9.2.1+.

### GitNexus

B3 learns multi-repo registry, setup/install UX, bridge UI/product UX, repo groups, architecture contracts, and later multi-service analysis.

Covered by Phase 8.7, Phase 8.8, and Phase 11.0+.

### Serena

B3 learns LSP backend, IDE-grade semantic operations, find definition/references/implementations, symbolic editing, rename/refactor, and mode/profile system.

Covered by Phase 8.6, Phase 9.0, Phase 9.1, Phase 12, and Phase 13.

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
