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
- Phase 8.6 - MCP Tool Profiles + Manifest Slimming
- Phase 8.7 - Agent Install Helper + Hook Integration Foundation
- Phase 8.8 - Multi-repo Registry + Project Groups
- Phase 9.0 - Language Backend Architecture

Next:

- Phase 9.1 - LSP Backend MVP

Rust has the best current language support. Multi-language and domain-specific
application intelligence is planned for later phases.

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
- Rust is the only implemented parser backend right now.
- C#, TypeScript, JavaScript, TSX/JSX, Dockerfile, XAML, Python, Java, Go, and
  other languages are detected for capability reporting only.
- Embeddings, semantic search, Qdrant, LSP runtime, session memory, symbolic editing,
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

Next:

- Phase 9.1 - LSP Backend MVP

See `PLAN.md` for the full roadmap.

## License

See `LICENSE`.
