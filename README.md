# B3 MCP Code Intelligence

## What Is This?

B3 MCP Code Intelligence is a local-first, offline-first MCP code intelligence
platform for Cursor and Codex. It indexes a repository into a persistent local
code graph so agents can answer structural questions without repeatedly reading
files, grepping, or dumping large context.

## Features

- Local-first MCP server
- Incremental code indexing
- Persistent SQLite-backed graph storage
- FTS/BM25 lexical search path
- Query engine for symbols, graph traversal, impact, and context packs
- Token-saving retrieval and context packing
- Localhost control server
- Next.js web UI
- React Flow graph explorer
- Query trace UI
- File watcher and daemon mode
- Local benchmark harness and JSON baseline output

## Architecture

- **MCP Runtime**: thin stdio/JSON-RPC/tool-routing boundary for Cursor/Codex.
- **Indexer**: repository discovery, hashing, parsing, graph extraction, watcher integration, and parser isolation.
- **Storage**: local SQLite/libSQL-style persistence with WAL, graph tables, FTS, token ledger, and diagnostics tables.
- **Query Engine**: bounded graph traversal, lexical search, ranking, impact analysis, and context packing.
- **Control Server**: localhost Axum API for status, query, graph, config, diagnostics, and events.
- **Web UI**: local Next.js interface for dashboard, graph explorer, query trace, diagnostics, and events.

The MCP runtime stays thin. Indexing, parsing, storage, ranking, graph traversal,
and UI work remain outside the MCP hot path.

## Quick Start

```powershell
cargo build --workspace
cargo test --workspace
```

## Build And Test

```powershell
cargo fmt
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

For the local verification helper:

```powershell
.\scripts\verify.ps1
```

## Benchmarks

```powershell
cargo run -p b3-bench -- baseline
```

Benchmark output is written to `target/benchmarks/baseline.json`.

## MCP Runtime

```powershell
b3-mcp-runtime serve --project "." --database ".b3/b3.db"
```

During development:

```powershell
cargo run -p b3-mcp-runtime -- serve --project "." --database ".b3/b3.db"
```

See [MCP_TOOLS.md](MCP_TOOLS.md).

## Control Server

```powershell
b3-control-server serve --project "." --database ".b3/b3.db" --port 7777
```

During development:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

With watch mode:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777 --watch
```

See [CONTROL_SERVER.md](CONTROL_SERVER.md).

## Web UI

```powershell
cd apps/web-ui
npm install
npm run dev
```

Open `http://127.0.0.1:3000`.

See [WEB_UI.md](WEB_UI.md).

## Offline & Free Hard Requirement

B3 core **must not require** any of the following:
- External APIs
- Cloud services
- Hosted vector databases
- SaaS authentication providers
- Remote telemetry
- Paid UI kits
- Paid backend services
- Paid or proprietary plugins
- OpenAI / Anthropic / Gemini / cloud embedding APIs
- JetBrains paid plugin
- Internet access

All external/cloud/paid integrations are allowed **only as optional plugins**, which are **disabled by default**.

## Current Status

- **Completed**: Phase 8.4 - Performance Optimization Pass A
- **Current**: Phase 8.5 - Command Output Compaction
- **Next**: Phase 8.6 - MCP Tool Profiles + Manifest Slimming

## What Works Today

- MCP runtime
- SQLite graph storage
- Rust indexing and query flow
- Query / context-pack tools
- Control server
- Web UI
- Graph explorer
- Query trace UI
- File watcher
- Parser isolation
- Benchmark baseline harness

## Current Limitations

- Rust has the most complete language support right now.
- C#, TypeScript, React, Angular, Node, Docker, Kafka, RabbitMQ, SignalR, and WPF are planned for Phase 9.x.
- Embeddings and semantic search are not yet implemented.
- Command output compaction starts in Phase 8.5.

## Repository Layout

- `crates/b3-core`
- `crates/b3-storage`
- `crates/b3-indexer`
- `crates/b3-query`
- `crates/b3-mcp-runtime`
- `crates/b3-control`
- `crates/b3-bench`
- `apps/web-ui`
- `benchmarks/fixtures`

## Roadmap

See [PLAN.md](PLAN.md).

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md).

## Documentation

- [PLAN.md](PLAN.md)
- [REQUIREMENTS.md](REQUIREMENTS.md)
- [ALGORITHM_ANALYSIS.md](ALGORITHM_ANALYSIS.md)
- [MCP_TOOLS.md](MCP_TOOLS.md)
- [CONTROL_SERVER.md](CONTROL_SERVER.md)
- [WEB_UI.md](WEB_UI.md)
- [DEVELOPMENT.md](DEVELOPMENT.md)
- [OFFLINE_REQUIREMENTS_PATCH.md](OFFLINE_REQUIREMENTS_PATCH.md)
- [AGENTS.md](AGENTS.md)

*Optional example MCP configuration for Codex:* see [MCP_TOOLS.md](MCP_TOOLS.md) for a minimal JSON configuration snippet.
