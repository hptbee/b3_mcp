# Development

## Required Local Tools

- Rust stable toolchain with `cargo`
- Git
- PowerShell on Windows

Future phases may also require:
- Node.js for the Next.js UI
- optional neural/model embedding providers such as Ollama, GGUF models,
  sentence-transformers, Candle, or fastembed when a later plugin phase adds
  them
- optional local vector components only when disabled by default
- optional local language servers such as `typescript-language-server`, `csharp-ls`, or OmniSharp when LSP is explicitly enabled

All required dependencies should be available locally for normal development.
Do not make verification depend on live network access, hosted databases,
cloud authentication, or external telemetry endpoints.

## Rust Installation

Install Rust from:

```text
https://rustup.rs/
```

After installation, confirm:

```powershell
cargo --version
rustc --version
```

## Verification Commands

Run from the repository root:

```powershell
cargo fmt
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

For a stricter local pre-commit pass:

```powershell
.\scripts\verify.ps1
```

## Manual Project Indexing

Initialize and index a local project database:

```powershell
cargo run -p b3-control --bin b3-control-server -- init --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- reindex --project "." --database ".b3/b3.db"
```

Then run the control server:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

The control server uses `http://127.0.0.1:7777` by default. The Web UI dev
server uses `http://127.0.0.1:8888` by default and calls the local control
server through `NEXT_PUBLIC_B3_API_BASE_URL`.

`reindex` is currently a safe incremental reindex. It skips unchanged files,
cleans deleted files for the current branch, and does not delete unrelated
project data.

Scoped indexing can be previewed and run locally:

```powershell
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db" --scope "path:crates/b3-indexer" --dry-run
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db" --scope "language:go"
```

Supported scopes include path, file, glob, language, framework, route,
component, module, data access, realtime, messaging topic/queue/routing key,
and infrastructure. Target scopes use existing local metadata, so zero matches
are valid when a broader first index has not populated the database.

The CI skeleton mirrors these commands with:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace`

Phase 10.4 MCP/control hybrid search integration is verified by the Rust workspace tests.
The default provider id is `local_hash`, embeddings remain disabled by default,
and vector search runs as local SQLite candidate loading plus Rust cosine
scoring. Hybrid ranking combines lexical, vector, and metadata signals inside
`b3-query`. `POST /api/search/hybrid` and the MCP `semantic_search` tool are
thin read-only adapters over that ranking layer. They work without internet
access, API keys, hosted vector databases, Qdrant, native SQLite vector
extensions, model downloads, Node.js, Python, Docker, `dotnet`, or Go.

## Benchmark Commands

Run the local benchmark baseline from the repository root:

```powershell
cargo run -p b3-bench -- baseline
```

The runner uses deterministic local fixtures from `benchmarks/fixtures` and
writes JSON output to:

```text
target/benchmarks/baseline.json
```

The baseline includes local timings for MCP tool listing by profile, simple MCP
tool calls, control-server handlers, query operations, indexing, changed-file
reindexing, watcher debounce behavior, SQLite graph summary queries, and parser
worker request handling. MCP tools/list benchmark entries record `profile` and
`tool_count` metadata without removing existing JSON fields. The `memory_kb`
field may be `null` on platforms where a rough process-memory snapshot is not
available yet.

Phase 10.5 adds local hybrid search quality evaluation to the same baseline.
`semantic_quality` compares lexical-only, vector-only, and hybrid modes on a
small checked-in fixture set, reports hit@1/3/5/10, MRR, latency, fallback
counts, and a chars/4 token estimate. These are fixture baselines only; they do
not claim neural semantic quality or production-level relevance.

Phase 11.0 adds cross-project architecture contracts and status only. The
contracts live in `b3-core`; `GET /api/architecture/status` reports that
architecture contracts are available.

Phase 11.1 adds local group query federation in `b3-query::architecture`.
Federation reads registry-defined project groups, opens existing project DBs
read-only, returns partial project status/warnings, and aggregates metadata
  summaries without merging databases. Route/API, messaging,
  package/contract/infra matching, group impact/context packs, and architecture
  graph/service map APIs are now local/read-only query features. The local
  storage model remains one repo-local `.b3/b3.db` per project.

Phase 11.1.1 adds context efficiency and tool-call reduction measurements to
the same local baseline. The JSON output includes `efficiency_metrics` with
target comparisons for `10.0x` token reduction, `2.1x` tool-call reduction, and
a fixture answer-quality target of `0.8`. The deterministic model compares
file-by-file exploration with `search_code`, `semantic_search`,
`semantic_search_context_pack` profiles (`minimal`, `balanced`, `deep`), and a
Phase 11.1 group-summary workflow. Token estimates use chars/4, tool calls are
modeled counts, and answer quality is fixture coverage, not LLM grading. The
current balanced context-pack fixture result is about `1.11x` fewer tokens,
`4.05x` fewer modeled tool calls, and `0.699` quality, so the token and quality
targets are not yet met.

Phase 11.2 adds local cross-repo route/API matching in
`b3-query::architecture`. It reads each registry project DB independently,
collects existing server route metadata, extracts conservative HTTP client
literals from indexed local file text, and returns deterministic match
candidates through `GET /api/architecture/groups/{group_id}/route-matches`.
It does not merge databases, execute HTTP requests, resolve DNS/runtime env
vars, fetch OpenAPI documents, add MCP tools, or infer messaging/package/infra
relationships.

Phase 11.3 adds local cross-repo messaging matching in
`b3-query::architecture`. It reads existing federated messaging metadata,
matches producer and consumer topics/queues/patterns/routing keys through
deterministic local keys, and returns candidates through
`GET /api/architecture/groups/{group_id}/message-matches`. It does not connect
to brokers, publish or consume messages, call cloud Pub/Sub APIs, merge
databases, add MCP tools, or infer package/contract/infra relationships.

Phase 11.4 adds local cross-repo package/contract/infra matching in
`b3-query::architecture`. It reads local manifest, schema/contract, and
infrastructure metadata from federated project DBs and returns candidates
through `GET /api/architecture/groups/{group_id}/dependency-matches`. It does
not run package managers, Docker, Kubernetes, Terraform, cloud CLIs, remote
schema fetches, schema validators, merge databases, add MCP tools, or add group
impact/service-map APIs.

Phase 11.5 adds local group impact and cross-repo context packs in
`b3-query::architecture`. It resolves seeds against existing route/message/
dependency match candidates, traverses them with bounded depth/limit settings,
and returns results through `POST /api/architecture/groups/{group_id}/impact`.
It does not execute HTTP requests, connect to brokers, run package managers,
run Docker/Kubernetes/Terraform/cloud CLIs, merge databases, add MCP tools, or
add service-map APIs.

Phase 11.6 adds local architecture graph and service map APIs in
`b3-query::architecture`. It builds graph/service-map responses on demand from
existing route/message/dependency match candidates and group federation
summaries, returning them through `GET /api/architecture/groups/{group_id}/graph`
and `GET /api/architecture/groups/{group_id}/service-map`. It does not persist
a global graph, merge databases, execute HTTP requests, connect to brokers, run
package managers, run Docker/Kubernetes/Terraform/cloud CLIs, add MCP tools, or
add architecture graph UI.

Phase 11.7 adds cross-project benchmark coverage and documentation in
`b3-bench::architecture`. The baseline command now writes a
`cross_project_benchmark` JSON section and prints a human-readable Phase 11
architecture summary. It benchmarks a deterministic local fixture group and
inspects optional local candidates from `benchmarks/b3.benchmark.toml`; missing
local candidate paths/DBs warn and skip. It is local fixture/local-repo
coverage only, not a 31 public real-world repository claim. It does not index
optional real repos automatically, merge DBs, call external services, run package
managers, run Docker/Kubernetes/Terraform, connect to brokers, execute HTTP
requests, or implement graph UI, symbolic editing, rename/refactor, or full Git
Intelligence. After switching branches, reindex before comparing benchmark
results until branch-aware indexing is implemented.

Phase 12 adds the local Symbolic Editing MVP. Editing contracts live in
`b3-core`, planning/apply logic lives in `b3-query::editing`, and `b3-control`
only exposes thin local adapters at `POST /api/edit/preview` and
`POST /api/edit/apply`. Preview never mutates files. Apply requires
`mode=apply` and `dry_run=false`, revalidates the planned text/hash, writes a
local backup by default, and returns a reindex-recommended warning. The MVP is
single-file and bounded; rename/refactor, update-all-references, MCP edit tools,
UI editing, formatter/compiler execution, package manager execution, generated
code execution, external APIs, telemetry, and full Git Intelligence remain
deferred.

Phase 13 adds the local Rename / Refactor MVP. Rename/refactor contracts live in
`b3-core`, planning/apply logic lives in `b3-query::refactor`, and `b3-control`
exposes `POST /api/refactor/rename/preview` plus
`POST /api/refactor/rename/apply`. Preview is the default. Apply requires
`mode=apply` and `dry_run=false`, validates all changed files before writing,
creates backups by default, and returns `reindex_recommended=true`. The MVP is
conservative: it uses indexed symbol targets, graph/FTS candidate files, and
bounded identifier scanning; comments/strings are excluded unless low-confidence
occurrences are explicitly requested. It is not an IDE-grade semantic rename
engine, and broad refactoring, compiler/formatter execution, package managers,
LSP-required edits, MCP refactor tools, UI editing, and Git Intelligence remain
deferred.

The watcher debounce benchmark measures event coalescing overhead. It does not
include an intentional sleep for the configured debounce wait, because that
would benchmark the configured delay rather than processing cost.

Regression thresholds live in:

```text
benchmarks/benchmark-thresholds.json
```

Thresholds are advisory by default. They do not fail CI unless
`fail_on_regression` is explicitly enabled.

The default local benchmark configuration path is:

```text
benchmarks/b3.benchmark.toml
```

This file is the Phase 11.7 input and source of truth for local benchmark
project names/paths. It may reference optional local repositories with
machine-specific paths. Missing optional repositories must be reported as
warnings, not failures, and normal `cargo build`, `cargo check`, and
`cargo test --workspace` runs must not require those paths to exist. The current
`b3-bench baseline` command uses this config for optional project discovery and
still remains local/offline.

## Phase 15 Static Parser Boundaries

Systems/mobile/config/web support must stay static and local. New extractors
under `b3-indexer` may scan source text and metadata files for conservative
symbols, imports/includes, key paths, safe package names, route/client hints,
asset references, Three.js/WebGL hints, XAML hints, and ksqlDB topic/dependency
hints. They must not run compilers, preprocessors, package managers, formatters,
runtimes, browsers, WebGL, Docker/Kubernetes/Terraform, Kafka, ksqlDB, brokers,
databases, language servers, external APIs, telemetry, or internet access.
Secret-like config values should be redacted or skipped; names and keys are
acceptable evidence.

## Phase 16 Hardening Boundaries

Config/data/web hardening may improve redaction, env-example parsing,
config-reference hints, HTML/template links, CSS/SCSS asset/media metadata,
XAML resource/binding quality, ksqlDB dependencies, and SQL table-reference
metadata. It must remain deterministic static analysis: no OS environment reads,
SQL execution, database/broker/Kafka/ksqlDB connections, browser/WebGL runtime,
package manager, compiler, formatter, Docker/Kubernetes/Terraform execution,
external API, cloud service, telemetry, or internet access.

## Phase 17 Audit Boundaries

Language and technology quality audit work is limited to truthful capability
reporting, support matrix alignment, fixture/test coverage, metadata
consistency, secret redaction guarantees, and small false-positive guardrails.
Support levels stay conservative: Rust is Good; implemented language/file/
framework surfaces are Basic static/local; Three.js/WebGL is Basic static
hints; architecture graph UI, full Git Intelligence, broad refactor behavior,
runtime validation, and compiler-grade parsing are deferred. Do not run package
managers, compilers, formatters, runtimes, browsers/WebGL, Docker/Kubernetes/
Terraform, brokers, databases, Kafka, ksqlDB, RabbitMQ, external APIs, cloud
services, telemetry, or mandatory language servers.

### Optional Local Benchmark Projects

Enabled `local_repo` projects in `benchmarks/b3.benchmark.toml` can participate
in the Phase 11.7 cross-project benchmark when they exist locally. Prepare them
from the B3 repository root with:

```powershell
.\scripts\setup-local-benchmark.ps1
```

If local PowerShell execution policy blocks `.ps1` files, run the same helper
with:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\setup-local-benchmark.ps1
```

The helper reads `benchmarks/b3.benchmark.toml`, verifies enabled local
repository candidates, creates each repo-local `.b3` directory when needed, and
indexes each available project with a missing database into its configured
database. Local paths are machine-specific and should live only in the benchmark
config.

Missing optional paths print warnings and are skipped. They must not fail normal
builds, tests, or CI. To index available optional projects and immediately run
the local baseline:

```powershell
.\scripts\setup-local-benchmark.ps1 -RunBenchmark
```

The setup script stays local-only. It does not call external APIs, run package
managers, start Docker/Kubernetes/Terraform, connect to brokers, use telemetry,
or merge project databases.

## Agent Install Helper

The `b3` helper lives in `crates/b3-cli` and only reads/writes local agent
configuration files.

Dry-run is the default:

```powershell
cargo run -p b3-cli -- install --agent codex --project "." --database ".b3/b3.db" --profile optimized --dry-run
cargo run -p b3-cli -- install --agent cursor --project "." --database ".b3/b3.db" --profile optimized --dry-run
```

Writing requires `--apply` or `--write`. Backups are enabled by default for
apply mode:

```powershell
cargo run -p b3-cli -- install --agent codex --project "." --database ".b3/b3.db" --profile optimized --apply --backup
```

Use `--config <path>` for tests and smoke checks so real user Codex/Cursor
config files are not touched. Uninstall is also dry-run by default and removes
only the named server entry.

Doctor is local-only:

```powershell
cargo run -p b3-cli -- doctor --project "." --database ".b3/b3.db" --profile optimized
```

Phase 21.4.1 adds setup-oriented MCP helper aliases. These are dry-run by
default and do not change MCP tool profiles or expose Git MCP tools:

```powershell
cargo run -p b3-cli -- mcp config cursor --project "." --database ".b3/b3.db" --profile optimized
cargo run -p b3-cli -- mcp config codex --project "." --database ".b3/b3.db" --profile optimized
cargo run -p b3-cli -- mcp doctor --project "." --database ".b3/b3.db" --profile optimized
cargo run -p b3-cli -- mcp profiles
```

Use `--cargo-run --repo <b3-repo-path>` when generating templates for a source
checkout instead of an installed `b3-mcp-runtime` binary. Writing remains
explicit through `install --apply`; generated `mcp config` templates are
print-only so setup never overwrites Cursor or Codex config by surprise.

## Registry And Groups

The optional registry is local JSON. Default path:

```text
~/.b3/registry.json
```

Use `B3_HOME` or `--registry <path>` for tests and smoke runs so real user
registry files are not touched.

```powershell
cargo run -p b3-cli -- register "." --name "B3 MCP" --tag rust --tag mcp --registry "target/smoke-registry/registry.json"
cargo run -p b3-cli -- list --registry "target/smoke-registry/registry.json"
cargo run -p b3-cli -- status b3-mcp --registry "target/smoke-registry/registry.json"
cargo run -p b3-cli -- group create "Business Application" --id business-app --registry "target/smoke-registry/registry.json"
cargo run -p b3-cli -- group add business-app b3-mcp --registry "target/smoke-registry/registry.json"
cargo run -p b3-cli -- group status business-app --registry "target/smoke-registry/registry.json"
```

`b3 unregister <project-id>` is dry-run by default and requires `--apply` to
modify the registry. It never deletes project files or repo-local `.b3` DBs.

## Language Backends

Phase 9.0 adds shared language backend contracts in `b3-core`, Rust tree-sitter
backend metadata in `b3-indexer`, and control capability reporting at:

```text
GET /api/capabilities
GET /api/languages
```

Detection is local and rule-based: extensions, selected filenames such as
`Dockerfile`, and compose filenames. Detection does not mean parser support.
Rust is currently implemented through `tree-sitter-rust`. JavaScript,
TypeScript, JSX, and TSX have basic local tree-sitter indexing. C# has basic
local static extraction for ASP.NET Core Web API controllers, route attributes,
action methods, and constructor dependency type names. Go has basic local static
extraction for `.go` and `go.mod` files, package/import metadata, functions,
receiver methods, structs, interfaces, type declarations, const/var
declarations, conservative local call edges, and HTTP route hints. Phase 14 adds
basic local static backend extraction for Python, Java, Kotlin, PHP, and Ruby:
project metadata detection, symbols/imports, conservative route/API hints,
data-access hints, and messaging hints where literal evidence is visible. Other
planned languages remain detect-only or unsupported until their phases land.
LSP exists as a local backend foundation and is disabled by default.
React/TSX component intelligence is basic static analysis only and is exposed
through indexed symbol metadata and the local control API. Next.js intelligence
is basic static analysis only and is exposed through route/component metadata
and `GET /api/routes`. Angular intelligence is basic static analysis only and
is exposed through route/component/symbol metadata without invoking the Angular
compiler. ASP.NET Core / C# Web API intelligence is basic static analysis only
and is exposed through existing route/symbol metadata without invoking Roslyn,
dotnet CLI, package restore, language servers, or app code. ORM/database access
intelligence is basic static analysis only and is exposed through
`GET /api/data-access` without database connections, SQL execution, migrations,
package managers, ORM CLIs, or app code. Realtime/socket intelligence is basic
static analysis only and is exposed through `GET /api/realtime` without network
connections, socket server startup, package managers, protocol execution, or app
code. Messaging/event-driven intelligence is basic static analysis only and is
exposed through `GET /api/messaging` without broker connections, broker startup,
cloud API calls, package managers, protocol execution, or app code.
Cloud/infrastructure intelligence is basic static analysis only and is exposed
through `GET /api/infrastructure` without Docker, Kubernetes, Terraform,
`gcloud`, cloud APIs, credentials, registries, package managers, or app code. Go
language support is basic static analysis only and is exposed through existing
symbol metadata, `/api/languages`, `/api/capabilities`, and `/api/routes` for
route hints without the Go toolchain, `go` commands, module downloads,
registries, package managers, or app code. .NET Desktop / WPF intelligence is
basic static analysis only and is exposed through existing symbol metadata,
`/api/languages`, `/api/capabilities`, and `GET /api/wpf`; it detects WPF
project hints, XAML views/resources, code-behind hints, binding paths, command
bindings, resource references, DataContext hints, and ViewModel naming hints
without Visual Studio, MSBuild, `dotnet`, Windows runtime, a XAML compiler,
designer integration, or app execution. Python/Java/Kotlin/PHP/Ruby backend
support is exposed through existing symbol metadata, `/api/languages`,
`/api/capabilities`, `GET /api/routes`, `GET /api/data-access`, and
`GET /api/messaging` where hints are extracted; it does not run pip, poetry,
uv, Maven, Gradle, composer, bundle, compilers, runtimes, language servers,
external APIs, or app code. Phase
9.2.4.1 is a behavior-preserving web module split checkpoint:
`crates/b3-indexer/src/web/mod.rs` now orchestrates focused web extraction
modules without behavior, API, schema, MCP, dependency, or Web UI changes. The
current roadmap is completed through Phase 9.2.12; the next planned
implementation phase is Phase 10 - Local Embeddings + Vector Search.

## Offline-First Expectations

The default project must work without internet access after dependencies are available locally.

Core features must not require:
- cloud APIs
- hosted vector databases
- SaaS authentication
- remote telemetry
- OpenAI, Anthropic, or Gemini APIs

External integrations must remain:
- optional
- plugin-based
- disabled by default

## Boundary Expectations

- MCP runtime remains protocol-only.
- MCP tool profiles are static local runtime configuration; default profile is
  `optimized`.
- Agent install helper logic lives outside query/index/storage internals and
  must not execute commands or install automatic hooks.
- Registry and project groups are metadata only; they must not trigger
  cross-project queries, graph merging, filesystem scans, or indexing.
- Language backend contracts must report support honestly. Do not claim
  C#/TypeScript/JavaScript/LSP/framework support until the implementation
  exists.
- Indexing runs outside the MCP hot path.
- Storage exposes repository contracts instead of leaking SQLite details across crates.
- Thread-safe SQLite index-store adapters belong in `b3-storage`, not HTTP or MCP adapters.
- Command output compaction only transforms provided stdout/stderr; it must not execute commands.
- LSP runtime belongs in `b3-indexer` as a local backend capability; it must stay disabled by default and must not install, download, or shell out beyond launching configured local server binaries.
- JavaScript/TypeScript/JSX/TSX support is tree-sitter based and local-only; do not require `npm`, `node`, `tsc`, or `eslint` during indexing.
- Node.js REST intelligence is basic static analysis only; route extraction must not execute app code, framework CLIs, package managers, or package-registry lookups.
- React/TSX component intelligence is basic static analysis only; component extraction must not execute React apps, dev servers, package managers, `node`, `tsc`, or `eslint`.
- Next.js intelligence is basic static analysis only; route and boundary
  extraction must not run `next dev`, `next build`, `node`, `npm`, `tsc`,
  `eslint`, package scripts, registries, app code, or deployment tooling.
- Angular intelligence is basic static analysis only; decorator, component,
  service, module, route, template reference, and constructor DI extraction
  must not run `ng`, Angular compiler, `node`, `npm`, `tsc`, `eslint`, package
  scripts, registries, app code, or deployment tooling.
- ASP.NET Core / C# Web API intelligence is basic static analysis only; project
  detection, controller/action extraction, route composition, and constructor DI
  type-name extraction must not run `dotnet`, restore packages, require Roslyn,
  launch Visual Studio/Rider/OmniSharp/language servers, query NuGet, or execute
  app code.
- ORM/database access intelligence is basic static analysis only; package
  detection, DbContext/DbSet extraction, query callsite extraction, operation
  classification, and literal SQL capture must not connect to databases, execute
  SQL, run migrations, run `dotnet`, `node`, `npm`, Prisma generate, TypeORM
  CLI, Sequelize CLI, package registries, or app code.
- Realtime/socket intelligence is basic static analysis only; WebSocket,
  Socket.IO, SignalR, and minimal RSocket detection must not open network
  connections, start socket servers, run `node`, `npm`, `dotnet`, package
  managers, package registries, or app code.
- Messaging/event-driven intelligence is basic static analysis only; AMQP,
  RabbitMQ, Kafka, Google Pub/Sub, and NestJS messaging detection must not open
  broker connections, start brokers, call cloud APIs, run `node`, `npm`,
  `dotnet`, package managers, package registries, or app code.
- Cloud/infrastructure intelligence is basic static analysis only; Dockerfile,
  Docker Compose, Kubernetes, Terraform, GCP, and GKE extraction must not run
  Docker, Docker Compose, `kubectl`, Terraform, `gcloud`, cloud APIs, registry
  calls, provider/module downloads, credentials, package managers, or app code.
- Go language support is basic static analysis only; `.go` and `go.mod`
  extraction must not run the Go toolchain, `go build`, `go test`, `go run`,
  `go list`, `go mod download`, module registries, package managers, network,
  or app code.
- Phase 9.2.3.1 split the indexer source so `lib.rs` stays focused on
  orchestration, web extraction lives under `crates/b3-indexer/src/web/`, and
  indexer unit tests live in `crates/b3-indexer/src/tests.rs`.
- Embeddings run in background workers in later phases.
- UI/control plane stays separate from MCP runtime.
- Hook integration foundation is disabled by default and must not intercept
  shells or modify shell profiles.

## Command Output Compaction

Phase 8.5 adds deterministic local compaction for noisy command output. Use the
MCP tool `compact_command_output` when a client already has stdout/stderr and
wants a smaller summary.

The compactor supports conservative string-based detection for `git`, `cargo`,
`dotnet`, `npm`, `pnpm`, `yarn`, `ng`, `tsc`, `eslint`, `docker`,
`docker compose`, `rg`, `grep`, `cat`, `tree`, and unknown commands. It does not
run commands, open shells, call an LLM, upload output, or emit telemetry.

## Parser Worker Development

Phase 8.1 adds a local parser isolation worker binary:

```powershell
cargo build -p b3-indexer --bin b3-parser-worker
```

The worker uses stdin/stdout JSON lines, parses locally, never opens network
sockets, and never emits telemetry. `ParserIsolation::InProcess` remains the
default compatibility mode; `ParserIsolation::SubprocessWorker` is available for
crash/timeout isolation. Defaults are:

- `parser_timeout_ms = 10000`
- `parser_max_retries = 1`
- `parser_worker_path = None` (resolved next to the current executable when subprocess mode is used)

Parse failures are stored locally in SQLite table `parse_failures` and surfaced
through `GET /api/diagnostics`.

## Phase 21 Git Intelligence Development Boundary

Phase 21.0 is a design and safety checkpoint only. Git Intelligence
implementation must stay local-only and read-only by default. Future code may
read `.git` metadata or run bounded read-only local Git commands, but must not
run checkout, switch, commit, merge, rebase, reset, clean, push, pull, fetch,
branch/tag/ref mutation, auto-stash, auto-reindex on branch switch, working-tree
edits, remote hosting APIs, telemetry, cloud services, package managers,
Docker/Kubernetes/Terraform, brokers/databases, mandatory LSP, paid
dependencies, or internet-required workflows.

Keep boundaries clear: `b3-core` owns contracts only; a future `b3-git` or
equivalent reader owns local read-only Git inspection; storage only persists
future branch/index metadata; indexer may record an indexing-time Git snapshot
later; query may consume Git/diff metadata for impact; control and MCP remain
thin adapters; the Web UI only displays Git data after dedicated API support.

Phase 21.1 adds the first implementation boundary: `b3-core` owns Git status
contracts and `b3-git` owns local read-only Git status detection. The reader may
run only bounded local read-only commands for repository root, `.git`
directory, branch or detached HEAD, HEAD commit, and porcelain status counts.
It must return warnings rather than panic when Git is unavailable, the project
is not a Git repository, or status cannot be read.

Phase 21.2 adds branch-aware index metadata. `b3-core` owns
`GitIndexSnapshot`, storage migration 6 owns the local `index_git_snapshots`
table, and the indexer records one read-only Git snapshot per full index or
explicit path index run. The snapshot is metadata only; it does not store full
Git output, diffs, changed-file lists, or secrets.

Phase 21.3 adds pure stale-index and auto-index policy evaluation. `b3-core`
owns the freshness/policy DTOs and `b3-git` compares current read-only Git
status with the latest indexed snapshot. The evaluator can return Fresh, Dirty,
Stale, Unsafe, or Unknown and emits reindex/manual-action recommendations.

Auto-index execution remains disabled. Branch changes, commit changes, detached
HEAD, conflicts, no-git/unknown state, excessive changed files, and unavailable
changed-file details must block auto-index.

Phase 21.4 adds local read-only changed-file and diff-summary support in
`b3-git`. It parses status porcelain and numstat output with bounded stdout and
timeouts, never full patches or file contents. The summary can feed the
conservative policy evaluator, but no auto-index execution, Control endpoint,
MCP tool, Web UI panel, schema migration, branch comparison, or diff-aware
impact is added. Phase 21.5 is next for diff-aware impact analysis.

Phase 21.4.1 adds Cursor/Codex MCP setup helpers in `b3-cli` only. It can print
Cursor JSON and Codex TOML templates, validate local project/database/profile
inputs through doctor checks, and recommend existing profiles. It does not
change MCP tool counts, expose Git MCP tools, add Control endpoints, mutate Git,
or write config through the new `mcp config` templates.

Manual reindex controls remain later UI/control work: preview reindex, reindex
current branch, and reindex changed files only. A future auto-index toggle must
be off or conservative by default and must never run on branch change, commit
change, detached HEAD, conflicts, unknown Git state, no-git projects, excessive
changed files, unsafe delete/rename batches, indexed branch mismatch, or
indexed commit mismatch.
