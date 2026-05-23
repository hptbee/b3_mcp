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
