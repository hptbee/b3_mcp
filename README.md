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

## Current Status

Completed:

- Phase 8.5 - Command Output Compaction
- Phase 8.5.1 - Project Init + Manual Index Command
- Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup

Next:

- Phase 8.6 - MCP Tool Profiles + Manifest Slimming

Rust has the best current language support. Multi-language and domain-specific
application intelligence is planned for later phases.

## What Works Today

- Index this Rust repo into `.b3/b3.db`.
- Query indexed symbols, graph relationships, context packs, and impact data.
- Run the MCP runtime from an MCP client.
- Start the control server at `http://127.0.0.1:7777`.
- Start the web UI at `http://127.0.0.1:8888`.
- Trigger indexing from CLI, control API, or the web UI.
- Run local benchmark baselines.

## Current Limitations

- The workflow is single-project only; multi-repo registry and project groups
  are deferred to Phase 8.8.
- Rust is the only real language pack right now.
- Embeddings, semantic search, Qdrant, LSP, session memory, symbolic editing,
  installer tooling, and domain-specific intelligence are deferred.
- Reindex is currently safe incremental reindexing, not a separate force-delete
  full rebuild.
- Web UI dependencies must be installed locally before frontend checks/builds.

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
├── Backend API
├── Frontend App
├── Worker Service
├── Desktop Client
└── Runtime Infrastructure
```

Project groups and the multi-repo registry are deferred to Phase 8.8.

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

B3 currently exposes these MCP tools:

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

See `MCP_TOOLS.md` for request/response details.

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

MCP clients should launch this process over stdio.

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

- Phase 8.5.1.1 - Repository Structure Audit + Folder/File Cleanup

Next:

- Phase 8.6 - MCP Tool Profiles + Manifest Slimming

See `PLAN.md` for the full roadmap.

## License

See `LICENSE`.
