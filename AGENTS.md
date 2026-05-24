# Agent Instructions

## Identity

This repository builds a high-performance local-first MCP code intelligence platform for Cursor and Codex.

It combines:
- TokenSave-style token-saving semantic graph queries
- Codebase Memory MCP-style persistent tree-sitter knowledge graphs

## Offline-First Requirement

This platform must remain fully functional offline.

Never introduce required dependencies on:
- OpenAI APIs
- Anthropic APIs
- hosted vector databases
- cloud auth providers
- remote telemetry systems

External services must always be:
- optional
- plugin-based
- disabled by default

The default installation must work entirely offline.

## Keep Stack

Backend:
- Rust
- Tokio
- Axum
- rmcp
- tree-sitter
- SQLite/libSQL local DB with WAL
- Qdrant local mode
- DashMap
- tracing

Frontend:
- Next.js
- TypeScript
- React Flow
- WebSocket

Packaging:
- Tauri optional

## Architecture

Use hybrid monolith:
- lightweight MCP runtime
- shared core engine
- async worker pipelines
- optional control server
- optional localhost UI

MCP runtime must only handle:
- stdio transport
- JSON-RPC
- tool routing
- static local tool profile filtering
- concise tools/list manifest generation
- streaming
- cancellation
- session lifecycle

Never put these in MCP hot path:
- full indexing
- embedding generation
- unbounded graph traversal
- blocking IO
- large filesystem scans

## Product Priorities

1. reduce agent tokens
2. reduce grep/glob/read-file calls
3. provide instant graph queries
4. preserve memory across sessions
5. support Cursor and Codex
6. expose localhost UI for control/config/graph
7. make local agent setup explicit, dry-run-first, and reversible

## MCP Tool Profiles

Default MCP runtime profile is `optimized`.

Use:
- `tiny` for the smallest high-value manifest
- `optimized` for normal agent work
- `full` or `debug` when trace-heavy tools are needed
- `readonly` when future mutation tools must be hidden
- `editing` only for future symbolic editing tools
- `web-app` for common web application workflows
- `enterprise` for future graph/impact/multi-service workflows

Profiles are static local configuration only. Do not add installer hooks,
command execution, registry behavior, cloud services, telemetry, embeddings,
LSP, language packs, session memory, or symbolic editing as part of profile
filtering.

## Agent Install Helper

The `b3` helper may generate or update local Codex/Cursor MCP config files.
Install behavior must be dry-run-first, preserve unrelated config, avoid
duplicate server entries, and create backups before writes.

It also owns local registry commands for project and group metadata. Registry
behavior must stay metadata-only:

- default path `~/.b3/registry.json`
- `B3_HOME` or `--registry` can redirect tests/smoke runs
- one project keeps one repo-local `.b3/b3.db`
- no automatic filesystem scans
- no cross-project query execution
- no graph merging
- no architecture intelligence

## Language Backend Architecture

Language backend contracts live in `b3-core`. Indexer implementations or
adapters live in `b3-indexer`; control may report capabilities but must not own
parser logic.

Phase 9.0 support must stay honest:

- Rust is available through tree-sitter metadata and the existing parser path.
- Planned languages can be detected locally but must not claim parser, LSP,
  framework, route, or semantic capabilities.
- LSP remains disabled until a later local/free LSP backend is implemented.
- Detection must not shell out, call external APIs, or require internet access.

Hook integration is foundation only. Hooks must stay disabled by default:

- no automatic command interception
- no shell proxy
- no terminal capture
- no telemetry
- no shell profile modification
- no background daemon watching user commands

## Apply These Algorithms

Use:
- incremental hash indexing
- tree-sitter AST extraction
- FTS5/BM25 lexical search
- vector semantic search
- graph expansion with depth limits
- PageRank/centrality for importance
- community detection for module maps
- impact analysis through callers/callees/references/tests
- token savings ledger
- crash-isolated parser workers
- branch-aware indexing

## Avoid

- semantic-only search
- full-file dumps
- unbounded result lists
- Neo4j dependency
- Electron
- Python hot paths
- microservices
- global mutable state
- sync mutexes in hot paths
