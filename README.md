# B3 MCP Code Intelligence

B3 is a local-first, offline-first, free-by-default AI-native code intelligence platform for coding agents such as Codex, Cursor, and other MCP-compatible tools.

B3 indexes a repository into a local code graph so agents can query structured code knowledge instead of repeatedly using grep, reading full files, or dumping large context into the conversation.

The long-term goal is to become a local code intelligence layer for AI agents:

- code graph
- token-saving retrieval
- command output compaction
- query trace
- graph explorer
- local control UI
- future multi-language support
- future LSP-backed semantic operations
- future symbolic editing
- future session memory

B3 is not a cloud service. It is designed to run locally.

---

## What Is This?

AI coding agents often waste tokens and time by repeatedly exploring the same repository:

```text
grep -> read file -> grep again -> read more files -> lose context -> repeat
```

B3 changes that workflow:

```text
index repo once -> query local graph -> return compact context pack
```

Instead of asking the agent to rediscover the codebase every time, B3 provides MCP tools backed by a local SQLite graph, FTS search, query engine, and token-aware context packing.

---

## Core Features

- Local MCP runtime over stdio
- SQLite-backed code graph
- FTS/BM25 text search
- Symbol search
- Code search
- Callers/callees query
- Related symbol discovery
- Impact analysis
- Dependency tracing
- Cycle detection
- Token-aware context packs
- Query trace and explainability
- Token savings report
- Localhost control server
- Web UI
- Graph Explorer UI
- Query Trace UI
- File watcher and daemon mode
- Parser isolation with local subprocess worker
- Parse failure registry
- Local benchmark harness
- Deterministic local command output compaction
- `compact_command_output` MCP tool

---

## Offline & Free Hard Requirement

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
- local LSP servers when implemented later
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

```text
Phase 8.5 — Command Output Compaction
```

Current:

```text
Phase 8.6 — MCP Tool Profiles + Manifest Slimming
```

Next:

```text
Phase 8.7 — Agent Install Helper + Hook Integration
```

B3 can run today as a local MCP/runtime/control/UI platform.

Rust currently has the best language support. Broader real-world app-stack intelligence for C#, TypeScript, React, Angular, Node.js, Docker, Kafka, RabbitMQ, SignalR, and WPF is planned for Phase 9.x.

---

## What Works Today

Today B3 supports:

- local MCP runtime
- 11 MCP tools
- SQLite graph storage
- Rust indexing and query flow
- query/context-pack tools
- control server
- web UI
- graph explorer
- query trace UI
- file watcher
- parser isolation
- parse failure diagnostics
- benchmark baseline
- command output compaction
- `compact_command_output` MCP tool

Best current use cases:

- dogfooding B3 on the B3 Rust repo
- testing MCP integration with Codex/Cursor
- querying Rust symbols and relationships
- inspecting graph/query behavior
- testing local benchmark baseline
- compacting large command outputs before sending them to an agent

---

## Current Limitations

- Rust has the best current language support.
- MCP tool profiles start in Phase 8.6.
- Multi-repo registry is planned for Phase 8.8.
- C#, TypeScript, React, Angular, Node.js, Docker, Kafka, ksqlDB, RabbitMQ, SignalR, and WPF support are planned for Phase 9.x.
- Semantic search and embeddings are not implemented yet.
- Symbolic editing and rename/refactor tools are not implemented yet.
- Session memory is planned for Phase 10.2+.
- Command output compaction is rule-based and conservative.
- Token savings estimates are approximate and not tokenizer-exact.

---

## Architecture

B3 is split into focused crates and apps.

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
```

The control server and web UI sit beside the MCP runtime:

```text
Web UI
  |
  v
b3-control
  |
  v
query / storage / indexer
```

### Boundary Rules

B3 keeps strict crate boundaries:

- MCP runtime stays thin.
- MCP runtime handles protocol, tool routing, validation, and structured errors only.
- Storage owns persistence internals.
- Indexer owns parsing, indexing, watcher, and parser worker behavior.
- Query owns graph traversal, ranking, impact analysis, and context packs.
- Control server owns localhost HTTP/SSE adapter behavior.
- Web UI talks only to the local control server.
- Benchmark harness measures local behavior only.
- Command compaction compacts provided output only; it does not execute commands.

---

## Repository Layout

```text
crates/b3-core
```

Shared contracts, config, events, and common DTOs.

```text
crates/b3-storage
```

SQLite persistence, migrations, storage repositories, and SQLite-backed adapters.

```text
crates/b3-indexer
```

Indexing, parsing, file watcher, parser isolation, and parser worker.

```text
crates/b3-query
```

Search, graph traversal, ranking, context packs, impact analysis, query trace, and centrality usage.

```text
crates/b3-mcp-runtime
```

Thin MCP stdio runtime and MCP tool exposure.

```text
crates/b3-control
```

Localhost HTTP/SSE control server.

```text
crates/b3-bench
```

Local benchmark harness and deterministic benchmark fixtures.

```text
crates/b3-compaction
```

Deterministic local command output compaction.

```text
apps/web-ui
```

Local Next.js web UI.

```text
benchmarks/fixtures
```

Small deterministic benchmark repositories.

---

## MCP Tools

B3 currently exposes 11 MCP tools.

```text
find_symbol
search_code
find_callers
find_callees
related_symbols
impact_analysis
get_context_pack
trace_dependency
detect_cycles
savings_report
compact_command_output
```

### Tool Summary

| Tool | Purpose |
|---|---|
| `find_symbol` | Find symbols by name or qualified name |
| `search_code` | Search indexed code content |
| `find_callers` | Find inbound callers |
| `find_callees` | Find outbound callees |
| `related_symbols` | Find nearby or related symbols |
| `impact_analysis` | Estimate impact/risk of a symbol or change |
| `get_context_pack` | Return token-aware compact context |
| `trace_dependency` | Trace dependency path between nodes/symbols |
| `detect_cycles` | Detect graph cycles |
| `savings_report` | Report estimated token savings |
| `compact_command_output` | Compact provided command stdout/stderr locally |

`compact_command_output` does not execute commands. It only compacts output that is passed to it.

---

## Command Output Compaction

B3 includes deterministic local command output compaction.

Supported command families:

- `git`
- `cargo`
- `dotnet`
- `npm`
- `pnpm`
- `yarn`
- `ng`
- `tsc`
- `eslint`
- `docker`
- `docker compose`
- `rg`
- `grep`
- `cat`
- `tree`
- `unknown`

The compaction layer keeps important information such as:

- errors
- warnings
- failed tests
- compiler diagnostics
- changed files
- diff summaries
- non-zero exit status
- container/service status
- search result summaries
- truncation metadata

It never:

- executes commands
- shells out
- calls an LLM
- calls an external API
- uploads command output
- emits telemetry

Savings are currently returned as estimates from byte reduction. Ledger persistence is deferred.

---

## Quick Start

### Prerequisites

Install:

- Rust toolchain
- Node.js / npm for the web UI
- SQLite support via bundled Rust dependencies

Clone the repository:

```bash
git clone https://github.com/hptbee/b3_mcp.git
cd b3_mcp
```

Build and test:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Run benchmark baseline:

```bash
cargo run -p b3-bench -- baseline
```

The benchmark writes:

```text
target/benchmarks/baseline.json
```

---

## Run MCP Runtime

Build the MCP runtime:

```bash
cargo build -p b3-mcp-runtime
```

Run it manually:

```bash
cargo run -p b3-mcp-runtime -- serve --project "." --database ".b3/b3.db"
```

The MCP runtime uses stdio and is intended to be launched by an MCP client such as Codex or Cursor.

---

## Codex MCP Example

Example Codex config:

```toml
[mcp_servers.b3]
command = "D:\\Tools\\b3\\b3-mcp-runtime.exe"
args = [
  "serve",
  "--project",
  "D:\\Project\\b3_mcp",
  "--database",
  "D:\\Project\\b3_mcp\\.b3\\b3.db"
]
enabled = true
```

On Windows, Codex config is usually located at:

```text
C:\Users\<YOUR_USER>\.codex\config.toml
```

B3 can also be launched directly from the Rust target directory:

```toml
[mcp_servers.b3]
command = "D:\\Project\\b3_mcp\\target\\debug\\b3-mcp-runtime.exe"
args = [
  "serve",
  "--project",
  "D:\\Project\\b3_mcp",
  "--database",
  "D:\\Project\\b3_mcp\\.b3\\b3.db"
]
enabled = true
```

When developing B3 itself, using a copied stable binary such as `D:\Tools\b3\b3-mcp-runtime.exe` can avoid Windows file-lock issues during `cargo build`.

---

## Cursor MCP Example

Example Cursor config:

```json
{
  "mcpServers": {
    "b3": {
      "command": "D:\\Tools\\b3\\b3-mcp-runtime.exe",
      "args": [
        "serve",
        "--project",
        "D:\\Project\\b3_mcp",
        "--database",
        "D:\\Project\\b3_mcp\\.b3\\b3.db"
      ]
    }
  }
}
```

---

## Run Control Server

Run the local control server:

```bash
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

With file watcher enabled:

```bash
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777 --watch --debounce-ms 500
```

The control server binds to localhost by default.

Default API base:

```text
http://127.0.0.1:7777
```

Useful endpoints:

```text
GET  /health
GET  /api/status
GET  /api/projects
GET  /api/project
GET  /api/diagnostics
GET  /api/capabilities
GET  /api/config
GET  /api/events
POST /api/query/find-symbol
POST /api/query/search-code
POST /api/query/impact-analysis
POST /api/query/context-pack
POST /api/graph/neighbors
POST /api/graph/path
POST /api/graph/cycles
GET  /api/graph/summary
```

---

## Run Web UI

Install dependencies:

```bash
cd apps/web-ui
npm install
```

Run dev server:

```bash
npm run dev
```

Default web UI:

```text
http://127.0.0.1:3000
```

Default control server:

```text
http://127.0.0.1:7777
```

Optional environment variable:

```bash
NEXT_PUBLIC_B3_API_BASE_URL=http://127.0.0.1:7777
```

---

## Benchmarks

B3 includes a local benchmark harness.

Run:

```bash
cargo run -p b3-bench -- baseline
```

Output:

```text
target/benchmarks/baseline.json
```

The benchmark runner measures local behavior such as:

- cold startup
- MCP tools/list latency
- MCP tools/call latency
- control server handler latency
- query latency
- graph query latency
- context pack latency
- impact analysis latency
- indexing speed
- changed-file reindex latency
- watcher debounce overhead
- SQLite summary latency
- parser worker request latency
- command compaction latency

Benchmark thresholds are advisory by default.

Config:

```text
benchmarks/benchmark-thresholds.json
```

No benchmark data is uploaded.

---

## Development Commands

From the repository root:

```bash
cargo fmt
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo run -p b3-bench -- baseline
```

For the web UI:

```bash
cd apps/web-ui
npm run typecheck
npm run lint
npm run build
npm audit --json
```

Frontend checks are only required when frontend code, scripts, or UI behavior changes.

---

## Documentation

Important docs:

- `PLAN.md` — detailed roadmap and phase plan
- `REQUIREMENTS.md` — requirements and constraints
- `DEVELOPMENT.md` — development workflow and boundaries
- `ALGORITHM_ANALYSIS.md` — algorithm notes and benchmark methodology
- `MCP_TOOLS.md` — MCP tool reference
- `CONTROL_SERVER.md` — control server API notes
- `WEB_UI.md` — web UI notes
- `AGENTS.md` — agent/contributor guidance

`PLAN.md` is the source of truth for the detailed roadmap.

README stays concise and traditional.

---

## Roadmap Summary

B3 is being developed in phases.

Recently completed:

```text
Phase 8.5 — Command Output Compaction
```

Current:

```text
Phase 8.6 — MCP Tool Profiles + Manifest Slimming
```

Next:

```text
Phase 8.7 — Agent Install Helper + Hook Integration
```

Upcoming roadmap highlights:

- MCP tool profiles
- agent install helper
- multi-repo registry
- language backend architecture
- LSP backend
- C# / TypeScript / React / Angular support
- Node.js REST API intelligence
- Kafka / ksqlDB / RabbitMQ intelligence
- Docker runtime infrastructure intelligence
- SignalR intelligence
- C# WPF intelligence
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
| Multi-project local workflow | Phase 8.8 |
| C# Web API / F&B backend | Phase 9.1 / 9.2 |
| React / Angular / TypeScript / JavaScript | Phase 9.2 |
| Node.js REST API | Phase 9.2.1 |
| Kafka / ksqlDB | Phase 9.2.2 |
| RabbitMQ | Phase 9.2.2 |
| Docker / docker-compose | Phase 9.2.3 |
| SignalR | Phase 9.2.4 |
| C# WPF | Phase 9.2.5 |
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

For production-like usage on C#, Node.js, React, Angular, Docker, Kafka, RabbitMQ, SignalR, and WPF projects, wait for the Phase 9.x language and domain intelligence work.
