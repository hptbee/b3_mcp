# B3 MCP Code Intelligence

B3 is a local-first, offline-first, free-by-default code intelligence platform
for MCP-compatible coding agents such as Codex and Cursor.

B3 indexes a repository into a local SQLite-backed code graph so agents can ask
structured questions about symbols, files, calls, relationships, impact, and
context instead of repeatedly scanning the same source files.

## Core Features

- Thin stdio MCP runtime.
- Local SQLite graph, FTS/BM25 search, and token savings ledger.
- Rust tree-sitter indexing with incremental unchanged-file skips.
- Symbol lookup, code search, callers/callees, related symbols, dependency
  paths, cycle detection, impact analysis, and context packs.
- Query trace and explainability DTOs.
- PageRank/centrality snapshots for ranking and impact scoring.
- Localhost control server with health, project, graph, query, diagnostics, and
  manual index endpoints.
- Local Next.js web UI with project status, Run Index, Reindex Project, graph
  explorer, query trace, diagnostics, and SSE event display.
- File watcher, parser isolation worker boundary, parse failure registry,
  benchmark harness, and deterministic command output compaction.

## Offline And Free Requirement

Core B3 functionality must not require external APIs, cloud services, hosted
databases, SaaS auth, telemetry, paid UI kits, proprietary plugins, internet
access, or OpenAI/Anthropic/Gemini/cloud embedding APIs.

Optional integrations may be added later only as disabled-by-default plugins.
The default system must remain fully local.
B3 is offline-first and free-by-default.

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
- JetBrains paid plugin
- internet access

Allowed in core:

- local SQLite
- local FTS5
- local parser worker
- local benchmark harness
- local command output compaction
- local LSP servers when explicitly enabled/configured
- local embeddings when implemented later
- local Qdrant only as an optional local component when implemented later

External/cloud/paid integrations are allowed only as optional plugins:

- disabled by default
- not required for install
- not required for tests
- not required for benchmarks
- not required for core features

This requirement overrides all roadmap decisions.

---

## Current Status

Completed:

- Phase 8.5 - Command Output Compaction
- Phase 8.5.1 - Project Init + Manual Index Command
- Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup
- Phase 8.6 - MCP Tool Profiles + Manifest Slimming
- Phase 8.7 - Agent Install Helper + Hook Integration Foundation
- Phase 8.8 - Multi-repo Registry + Project Groups
- Phase 9.0 - Language Backend Architecture
- Phase 9.1 - LSP Backend MVP
- Phase 9.2 - Web Application Priority Support A
- Phase 9.2.1 - Node.js / REST API Intelligence
- Phase 9.2.2 - React / TSX Component Intelligence

Next:

- Phase 9.2.3 - Next.js Intelligence

B3 can run today as a local MCP/runtime/control/UI platform.

Rust currently has the best language support. JavaScript, TypeScript, JSX, and TSX have basic local tree-sitter indexing for symbols/imports. Node.js REST route intelligence is basic/static/local for Express, NestJS, and Fastify. React/TSX component intelligence is basic/static/local for common components, props types, JSX usages, and hooks. C# remains detect-only, and LSP remains local-only and disabled by default. Next.js intelligence is planned next; Angular, Docker, Kafka, RabbitMQ, SignalR, WPF, Three.js, and deeper app-stack intelligence remain planned for later Phase 9.x work.

---

## Why Phase 8.5.1 Exists

B3 already has the indexing engine, storage, query engine, watcher, and UI.

However, it still needs a simple user workflow for:

```text
init project -> index project -> open UI -> see files/symbols/edges
```

Phase 8.5.1 turns the existing indexing capability into an easy workflow:

- CLI init command
- CLI index command
- CLI reindex command
- control API index trigger
- UI Run Index button
- indexing status/events

Without this phase, B3 can index internally, but users do not yet have a clean `b3 init` / `b3 index` style experience.

---

## What Works Today

- Index this Rust repo into `.b3/b3.db`.
- Query indexed symbols, graph relationships, context packs, and impact data.
- Run the MCP runtime from an MCP client.
- Choose an MCP tool profile to reduce `tools/list` manifest noise.
- Generate or apply local Codex/Cursor MCP config with the `b3` helper.
- Register local projects and metadata-only project groups in a local registry.
- Start the control server at `http://127.0.0.1:7777`.
- Start the web UI at `http://127.0.0.1:8888`.
- Trigger indexing from CLI, control API, or the web UI.
- Run local benchmark baselines.

## Current Limitations

- Cross-project query execution, graph merging, and architecture intelligence
  are deferred.
- Rust has the strongest implemented parser backend.
- JavaScript, TypeScript, JSX, and TSX have basic local indexing for symbols/imports.
- C#, Dockerfile, XAML, Python, Java, Go, and other planned languages are
  detect-only or unsupported until their phases land.
- Embeddings, semantic search, Qdrant, session memory, symbolic editing, and
  domain-specific intelligence are deferred.
- LSP exists as a local backend foundation and is disabled by default.
- Reindex is currently safe incremental reindexing, not a separate force-delete
  full rebuild.
- Web UI dependencies must be installed locally before frontend checks/builds.
- Rust has the best current language support.
- JavaScript, TypeScript, JSX, and TSX have basic local indexing for symbols/imports.
- Node.js REST route extraction for Express, NestJS, and Fastify is basic/static/local.
- React/TSX component extraction is basic/static/local for common function,
  arrow, class, memo/forwardRef components, props type names, JSX usages, and
  hook names.
- Deep middleware order, runtime routing, Nest module graphs, guards/interceptors,
  deep dependency injection, and request lifecycle inference are deferred.
- Manual project init/index workflow is planned for Phase 8.5.1.
- MCP tool profiles start in Phase 8.6.
- Multi-repo registry and project groups are planned for Phase 8.8.
- C#, Angular intelligence, deep React runtime behavior, deep Node runtime behavior, Docker, Kafka, ksqlDB, RabbitMQ, SignalR, WPF, Three.js, and other app-stack support are planned for Phase 9.x.
- Semantic search and embeddings are not implemented yet.
- Symbolic editing and rename/refactor tools are not implemented yet.
- Session memory is planned for Phase 10.2+.
- Command output compaction is rule-based and conservative.
- Token savings estimates are approximate and not tokenizer-exact.

## Architecture Summary

```text
MCP Client / Agent
        |
        v
b3-mcp-runtime
        |
        v
b3-query -------- b3-storage
        |              |
        v              v
b3-indexer ------ SQLite / FTS / Graph
        |
        v
tree-sitter / parser worker

Web UI -> b3-control -> query / storage / indexer
```

Boundary rules:

- `b3-mcp-runtime` handles protocol and tool routing only.
- `b3-control` is a localhost HTTP/SSE adapter and may trigger the indexer.
- `b3-indexer` owns discovery, parsing, indexing, watcher, and parser workers.
- `b3-storage` owns SQLite persistence and migrations.
- `b3-query` owns graph traversal, ranking, context packs, and impact analysis.
- `apps/web-ui` talks only to the local control server.

## Project Model

Today B3 uses a single-project model:

```text
1 repository = 1 local .b3/b3.db
```

Future project groups are generic workspace groupings, for example:

```text
Business Application
â”œâ”€â”€ Backend API
â”œâ”€â”€ Frontend App
â”œâ”€â”€ Worker Service
â”œâ”€â”€ Desktop Client
â””â”€â”€ Runtime Infrastructure
```

Project groups are metadata only in Phase 8.8. Each project keeps its own
repo-local `.b3/b3.db`.

## Repository Layout

- `crates/` - Rust workspace crates.
- `apps/web-ui/` - local Next.js web UI.
- `benchmarks/fixtures/` - deterministic benchmark repositories.
- `scripts/` - development scripts.
- `.github/workflows/` - CI skeleton.
- `.skills/` - local agent skills.
- `docs/reference/` - preserved external/reference material.
- `docs/archive/` - historical phase and patch notes.

## MCP Tools

B3 has 11 current MCP tools. The default `optimized` profile exposes 7 of
them to reduce manifest tokens:

```text
find_symbol
search_code
related_symbols
impact_analysis
get_context_pack
compact_command_output
savings_report
```

Use `--profile full` for all 11 tools, `--profile tiny` for the smallest
5-tool set, or `--profile enterprise` for 9 graph/impact-oriented tools. Hidden
tools are rejected with a structured profile-aware error. See `MCP_TOOLS.md` for
profile tables and request/response details.

## Command Output Compaction

`compact_command_output` is deterministic and local. It compacts provided
stdout/stderr for common command families such as `git`, `cargo`, `npm`,
`dotnet`, `docker`, `rg`, `grep`, `cat`, and `tree`.

It never executes commands, opens shells, calls an LLM, uploads output, or emits
telemetry.

## Quick Start

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Initialize and index the repository:

```powershell
cargo run -p b3-control --bin b3-control-server -- init --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db"
```

Reindex safely:

```powershell
cargo run -p b3-control --bin b3-control-server -- reindex --project "." --database ".b3/b3.db"
```

## MCP Runtime Usage

```powershell
cargo run -p b3-mcp-runtime -- serve --project "." --database ".b3/b3.db"
```

Equivalent explicit optimized profile:

```powershell
cargo run -p b3-mcp-runtime -- serve --project "." --database ".b3/b3.db" --profile optimized
```

MCP clients should launch this process over stdio.

## Agent Install Helper

Generate Codex or Cursor MCP config without writing:

```powershell
cargo run -p b3-cli -- install --agent codex --project "." --database ".b3/b3.db" --profile optimized --dry-run
cargo run -p b3-cli -- install --agent cursor --project "." --database ".b3/b3.db" --profile optimized --dry-run
```

Apply with backup:

```powershell
cargo run -p b3-cli -- install --agent codex --project "." --database ".b3/b3.db" --profile optimized --apply --backup
```

Run local diagnostics:

```powershell
cargo run -p b3-cli -- doctor --project "." --database ".b3/b3.db" --profile optimized
```

The helper only edits local config files. Hooks are documented as future
foundation and remain disabled by default.

## Registry And Groups

The optional registry is local JSON at `~/.b3/registry.json` unless `B3_HOME`
or `--registry` points elsewhere.

```powershell
cargo run -p b3-cli -- register "." --name "B3 MCP" --tag rust --tag mcp
cargo run -p b3-cli -- list
cargo run -p b3-cli -- status b3-mcp
cargo run -p b3-cli -- group create "Business Application" --id business-app
cargo run -p b3-cli -- group add business-app b3-mcp
cargo run -p b3-cli -- group status business-app
```

Registry use is optional. Existing single-project commands still work without
`~/.b3/registry.json`.

## Language Backends

Phase 9.0 adds shared language backend contracts, local detection, support
levels, and capability discovery. Rust reports an available tree-sitter backend
with symbol/import/relationship extraction. Planned languages are detected where
possible but only report detect-file support until later phases.

## Control Server Usage

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

Control server URL:

```text
http://127.0.0.1:7777
```

Manual index API:

- `POST /api/index/run`
- `POST /api/index/reindex`
- `GET /api/index/status`

## Web UI Usage

```powershell
cd apps/web-ui
npm install
npm run dev
```

Web UI URL:

```text
http://127.0.0.1:8888
```

The UI still targets the control server at `http://127.0.0.1:7777` by default.
Override with `NEXT_PUBLIC_B3_API_BASE_URL` when needed.

## Benchmarks

```powershell
cargo run -p b3-bench -- baseline
```

Output:

```text
target/benchmarks/baseline.json
```

Benchmark data stays local.

## Documentation

- `PLAN.md` - detailed roadmap and phase plan.
- `REQUIREMENTS.md` - product and architecture requirements.
- `DEVELOPMENT.md` - local development and verification workflow.
- `ALGORITHM_ANALYSIS.md` - algorithm notes and benchmark methodology.
- `MCP_TOOLS.md` - MCP tool reference.
- `CONTROL_SERVER.md` - control server commands and API notes.
- `WEB_UI.md` - web UI usage and sections.
- `AGENTS.md` - contributor and agent guidance.
- `docs/reference/` - reference material.
- `docs/archive/` - historical documents.

`PLAN.md` is the source of truth for the detailed roadmap.

## Roadmap

Recently completed:

- Phase 9.0 - Language Backend Architecture
- Phase 9.1 - LSP Backend MVP
- Phase 9.2 - Web Application Priority Support A
- Phase 9.2.1 - Node.js / REST API Intelligence
- Phase 9.2.2 - React / TSX Component Intelligence

Next:

- Phase 9.2.3 - Next.js Intelligence

See `PLAN.md` for the full roadmap.

Upcoming roadmap highlights:

- project init/index workflow
- MCP tool profiles
- agent install helper
- multi-repo registry
- project groups
- language backend architecture
- LSP backend foundation, local-only and disabled by default
- basic JavaScript / TypeScript / JSX / TSX indexing
- basic Node.js REST route intelligence
- basic React / TSX component intelligence
- Next.js intelligence
- C# / Angular deeper support
- Node.js REST API intelligence, basic/static/local
- Kafka / ksqlDB / RabbitMQ intelligence
- Docker runtime infrastructure intelligence
- SignalR intelligence
- C# WPF intelligence
- Three.js / WebGL intelligence
- symbolic editing
- rename/refactor support
- local embeddings
- session memory
- architecture intelligence
- git intelligence
- duplicate/similarity detection
- plugin system
- packaging/installers

For the full plan, see `PLAN.md`.

---

## When Can We Use It?

| Use case | Status |
|---|---|
| Test MCP runtime with Codex/Cursor | Usable now |
| Rust repositories | Usable now |
| Command output compaction | Usable now |
| Project init/index workflow | Phase 8.5.1 |
| MCP tool profiles | Phase 8.6 |
| Multi-project local workflow | Phase 8.8 |
| Project groups | Phase 8.8 |
| Basic JavaScript / TypeScript / JSX / TSX indexing | Phase 9.2 |
| Basic Node.js REST route intelligence | Usable now, basic/static |
| C# Web API / backend services | Phase 9.2.5 |
| Basic React / TSX component intelligence | Usable now, basic/static |
| Next.js intelligence | Phase 9.2.3 |
| Angular deep graph intelligence | Phase 9.2.4 |
| Node.js REST API | Usable now, basic/static |
| Kafka / ksqlDB | Phase 9.2.8 |
| RabbitMQ | Phase 9.2.8 |
| Docker / docker-compose | Phase 9.2.9 |
| SignalR | Phase 9.2.7 |
| C# WPF | Deferred |
| Three.js / WebGL | Deferred |
| Refactor assistant | Phase 9.3 / 9.4 |
| Full memory/context platform | Phase 10.2+ |

---

## Reference Models

B3 is inspired by several projects and product patterns, but does not depend on them.

References include:

- codebase-memory-mcp
- TokenSave
- RTK / Rust Token Killer
- Context Mode
- Token Savior
- CodeGraph
- GitNexus
- Serena
- Neo4j Browser-style graph UX
- Sourcegraph/Cursor-style code intelligence workflows

These are architectural inspirations only.

B3 remains:

```text
local-first
offline-first
free-by-default
Rust-native where appropriate
MCP-compatible
SQLite-backed
```

---

## Security and Privacy

B3 is designed for local development.

By default:

- repository data stays local
- command output stays local
- benchmark data stays local
- SQLite database stays local
- no telemetry is sent
- no cloud service is required
- no external API is required

Command output compaction only processes output that is provided to B3. It does not execute commands or capture terminal output automatically.

---

## License

License information should be reviewed before public distribution.

If this repository is intended to be open source, add or verify the final license file before release.

---

## Status Note

B3 is under active development.

Current best use:

- local MCP experiments
- Rust code intelligence
- graph/query debugging
- command output compaction
- benchmark-driven development
- dogfooding on the B3 repository itself

For production-like usage on deep React runtime behavior, deep Node.js REST behavior, C#, Angular graphs, Docker, Kafka, RabbitMQ, SignalR, WPF, Three.js, and other application stacks, wait for the later Phase 9.x language and domain intelligence work.
