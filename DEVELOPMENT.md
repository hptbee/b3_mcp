# Development

## Required Local Tools

- Rust stable toolchain with `cargo`
- Git
- PowerShell on Windows

Future phases may also require:
- Node.js for the Next.js UI
- local Qdrant for vector search
- local embedding providers such as Ollama, GGUF models, sentence-transformers, Candle, or fastembed
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

The watcher debounce benchmark measures event coalescing overhead. It does not
include an intentional sleep for the configured debounce wait, because that
would benchmark the configured delay rather than processing cost.

Regression thresholds live in:

```text
benchmarks/benchmark-thresholds.json
```

Thresholds are advisory by default. They do not fail CI unless
`fail_on_regression` is explicitly enabled.

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
declarations, conservative local call edges, and HTTP route hints. Other
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
designer integration, or app execution. Phase
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
