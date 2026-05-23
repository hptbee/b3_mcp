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

## Workspace

This repository is a Rust Cargo workspace with local-first code intelligence
crates:

- `crates/b3-core`: shared domain types
- `crates/b3-mcp-runtime`: thin MCP protocol boundary
- `crates/b3-storage`: local SQLite persistence boundary
- `crates/b3-indexer`: incremental indexing and Rust tree-sitter pipeline
- `crates/b3-query`: hybrid query, graph traversal, ranking, and context packs
- `crates/b3-embeddings`: offline local embedding provider boundary
- `crates/b3-control`: localhost control server boundary

The default architecture is offline-first:

- no required external APIs
- no cloud-only embedding provider
- no hosted vector database dependency
- no remote telemetry or SaaS authentication
- external integrations must be optional plugins and disabled by default

Stable contracts live in `crates/b3-core`:

- IDs: project, file, node, edge, symbol, branch, session, and tool call IDs
- config models: app, project, indexing, retrieval, embedding, graph, UI, and offline config
- events: indexing, parser, graph, query, tool, token-saving, and config events
- traits: storage, graph, symbol, file, token ledger, index queue, indexer, query engine, embedding provider, vector store, config provider, and event bus

Implementation crates should implement these contracts instead of depending on
each other's concrete internals.

## Current Capabilities

Implemented through Phase 6.2:

- Rust-only real parsing through tree-sitter, with unsupported languages kept on
  the noop fallback path.
- SQLite-backed branch-aware storage for files, file hashes, symbols, graph
  nodes/edges, FTS content, and token savings ledger entries.
- Incremental indexing with unchanged-file skips, changed-file replacement, and
  deleted-file cleanup.
- Query engine APIs for symbol lookup, FTS/BM25 lexical search, callers,
  callees, related symbols, impact analysis, dependency paths, cycle detection,
  and token-budgeted context packs.
- Serializable query traces and MCP-ready DTOs.
- Live stdio MCP runtime with thin JSON-RPC/tool routing for Cursor/Codex use.
- Impact intelligence with deterministic risk scoring, public API heuristics,
  related-test discovery, missing-test warnings, and explainable trace entries.
- PageRank/centrality snapshots persisted locally in SQLite and used as bounded
  ranking/risk signals.

The MCP runtime remains protocol-only. Indexing, graph traversal, ranking,
storage, centrality, and impact logic stay in query/storage/indexer crates.

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

## Project Roadmap

### Completed Phases

- Phase 1: Workspace / Scaffold
- Phase 1.5: Contracts / Boundaries
- Phase 2: SQLite Storage / Schema Foundation
- Phase 3: Incremental Indexer Skeleton
- Phase 3.1: Indexer Audit / Cleanup
- Pre-Phase-4: Plugin Contracts / Docs / CI
- Phase 4: Real Rust Parsing + Storage Adapter
- Phase 4.1: Project/Branch Auto Ensure + Deleted File Cleanup
- Phase 5: Query Engine + Graph Traversal + Context Pack
- Phase 5.1: Query Hardening + Explainability
- Phase 5.2: Ranking Algorithms Upgrade
- Phase 6: MCP Tools over Query Engine
- Phase 6.0.1: Live MCP Runtime Wiring
- Phase 6.1: Impact Intelligence
- Phase 6.2: PageRank / Centrality

### Planned Phases

- Phase 7: Control Server + Localhost API
- Phase 7.1: Web UI Foundation
- Phase 7.2: Graph Explorer UI
- Phase 7.3: Query Trace UI
- Phase 8: File Watcher + Daemon Mode
- Phase 8.1: Parser Isolation
- Phase 9: Local Embeddings + Vector Search
- Phase 9.1: Semantic Context Upgrade
- Phase 10: Multi-language Packs
- Phase 11: Architecture Intelligence
- Phase 12: Git Intelligence
- Phase 13: Duplicate / Similarity Detection
- Phase 14: Real Plugin System
- Phase 15: Packaging + Installers
