# Codex MCP Code Intelligence Pack

Instruction + skills pack for building a high-performance local-first MCP code intelligence platform for Cursor and Codex.

Inspired by:
- TokenSave: token-efficient semantic knowledge graph for coding agents
- Codebase Memory MCP: persistent tree-sitter codebase knowledge graph for MCP code exploration

Keep the original stack:
- Rust backend/core
- Tokio async runtime
- Axum control server
- rmcp MCP runtime
- tree-sitter parsing
- SQLite/libSQL-style local persistence with WAL
- Qdrant vector search
- Next.js web UI
- React Flow graph visualization
- Tauri optional packaging

## Phase 1 Workspace

This repository starts with a minimal Rust Cargo workspace and base crates:

- `crates/b3-core`: shared domain types
- `crates/b3-mcp-runtime`: thin MCP protocol boundary
- `crates/b3-storage`: local persistence boundary
- `crates/b3-indexer`: indexing pipeline boundary
- `crates/b3-embeddings`: offline local embedding provider boundary
- `crates/b3-query`: hybrid query and graph retrieval boundary
- `crates/b3-control`: localhost control server boundary

The default architecture is offline-first:

- no required external APIs
- no cloud-only embedding provider
- no hosted vector database dependency
- no remote telemetry or SaaS authentication
- external integrations must be optional plugins and disabled by default

Phase 1 intentionally defines boundaries and shared metadata only. It does not
implement the MCP runtime, indexing workers, graph traversal, storage schema, or
web UI yet.

## Phase 1.5 Contracts

Stable contracts live in `crates/b3-core`:

- IDs: project, file, node, edge, symbol, branch, session, and tool call IDs
- config models: app, project, indexing, retrieval, embedding, graph, UI, and offline config
- events: indexing, parser, graph, query, tool, token-saving, and config events
- traits: storage, graph, symbol, file, token ledger, index queue, indexer, query engine, embedding provider, vector store, config provider, and event bus

Implementation crates should implement these contracts instead of depending on
each other's concrete internals.

## Commands

```powershell
cargo fmt
cargo check --workspace
cargo test --workspace
```

For a local pre-commit verification pass:

```powershell
.\scripts\verify.ps1
```

## Phase 3 Indexer

`crates/b3-indexer` owns the offline incremental indexing pipeline:

- repository discovery and ignore filtering
- content hashing and unchanged-file skips
- tree-sitter parser, symbol extractor, and relationship extractor contracts
- parser worker isolation boundary
- bounded index job queue and worker batch planning
- filesystem watcher contract
- branch-aware index metadata
- index lifecycle events and cancellation support

It does not implement retrieval ranking, embedding generation, MCP request
handling, or UI behavior.

Phase 3 boundary notes:

- `IndexStore` is the storage port used by the indexer. It keeps the indexer
  storage-agnostic; storage crates should adapt to it instead of being imported
  directly into indexing logic.
- `ParserIsolation::SubprocessWorker` records the required crash-isolation
  boundary. The current phase defines the boundary only; the subprocess worker
  implementation is future work.
- `LocalIndexJobQueue` is an in-process bounded queue for index jobs. It
  prevents unbounded enqueue growth and stays independent of MCP request
  handling.
- `BoundedWorkerPool` currently plans bounded batches. It is the replacement
  point for real parallel worker execution later.
- `NoopTreeSitterParser`, `NoopSymbolExtractor`, and
  `NoopRelationshipExtractor` are intentional placeholders. Phase 4 should
  replace them with language-specific tree-sitter query packs while preserving
  the `TreeSitterParser`, `SymbolExtractor`, and `RelationshipExtractor`
  contracts.

## Pre-Phase-4 Stabilization

`crates/b3-core` includes minimal plugin readiness contracts:

- `PluginId`, `PluginCapability`, `PluginMetadata`, and `PluginRegistry`
- capability discovery descriptors for offline-first feature negotiation
- lifecycle boundary traits for load, activate, pause, and unload behavior
- execution policy expectations for timeout-bound and cancellable plugins

These are contracts only. Real language packs, storage adapters, embedding
providers, retrieval ranking, MCP tools, graph traversal, and UI features are
Phase 4+ work.
