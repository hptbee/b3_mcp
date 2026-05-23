# Requirements

## Product Vision

Build a high-performance local-first MCP code intelligence platform for Cursor and Codex.

The system combines:
- TokenSave-style token-saving semantic graph queries
- Codebase Memory MCP-style persistent tree-sitter knowledge graphs

Primary goal:
Agents should query indexed structure instead of repeatedly scanning files.

## Non-Goals

Do not build:
- cloud-first architecture
- microservice architecture
- Electron app
- Neo4j-dependent graph engine
- Python hot-path runtime
- Docker-first local UX
- semantic-only RAG system

## Stack

Keep:
- Rust backend/core
- Tokio
- Axum
- rmcp
- tree-sitter
- SQLite/libSQL local DB with WAL
- local Qdrant only
- DashMap
- tracing
- Next.js
- TypeScript
- React Flow
- WebSocket
- Tauri optional

## Offline-First Requirement

The default installation must work fully offline.

Core functionality must not require:
- external APIs
- cloud authentication
- hosted vector databases
- remote telemetry
- SaaS dependencies
- internet connectivity

External services may be supported only as optional plugins, disabled by default.

Preferred local providers:
- SQLite/libSQL local mode
- local Qdrant
- Ollama
- GGUF embedding models
- sentence-transformers
- Candle
- fastembed

## Core Capabilities

Must provide, over the implementation phases:
- MCP server for Cursor/Codex
- persistent code graph
- semantic search with local embeddings
- FTS/BM25 keyword search
- graph traversal
- impact analysis
- smart context packs
- token savings ledger
- incremental indexing
- daemon/file watcher mode
- branch-aware indexing
- cross-session memory
- localhost UI
- graph explorer
- config editor
- tool playground
- diagnostics dashboard

## Required MCP Tools

Indexing:
- `index_project`
- `sync_project`
- `project_status`
- `watch_project`
- `doctor`

Search:
- `find_symbol`
- `search_code`
- `semantic_search`
- `search_code_graph`

Graph:
- `find_references`
- `find_callers`
- `find_callees`
- `trace_dependency`
- `impact_analysis`
- `detect_cycles`
- `community_map`

Context:
- `get_context_pack`
- `explain_symbol`
- `summarize_module`
- `list_related_files`

Memory:
- `record_decision`
- `record_code_area`
- `session_recall`

Token Saving:
- `estimate_tokens_saved`
- `savings_report`

Edit:
- `anchored_replace`
- `atomic_multi_replace`
- `insert_at_anchor`

## Architecture

Use a hybrid monolith:
- lightweight MCP runtime
- shared core engine
- async/background worker pipelines
- optional control server
- optional localhost UI

MCP runtime responsibilities:
- stdio transport
- JSON-RPC
- tool routing
- streaming
- cancellation
- session lifecycle

Never put these in the MCP hot path:
- full indexing
- embedding generation
- unbounded graph traversal
- blocking IO
- large filesystem scans

## Indexing Pipeline

Discovery
-> Ignore Filtering
-> Language Detection
-> File Hashing
-> tree-sitter Parsing
-> Symbol Extraction
-> Relationship Extraction
-> Graph Update
-> FTS Update
-> Embedding Queue
-> Cache Update

Must support:
- incremental indexing
- changed-file-only reindexing
- per-symbol hashes
- git-aware change detection
- branch-aware database namespace
- subprocess-isolated parser workers
- parallel worker pools
- debounced filesystem watcher
- generated/vendor file skipping
- crash recovery

## Graph Requirements

Node types:
- Project
- File
- Module
- Namespace
- Class
- Struct
- Interface
- Enum
- Function
- Method
- Variable
- Route
- Endpoint
- ConfigKey
- Test
- Package
- Decision
- CodeArea

Edge types:
- CONTAINS
- IMPORTS
- CALLS
- REFERENCES
- IMPLEMENTS
- INHERITS
- DEPENDS_ON
- TESTS
- ROUTES_TO
- READS_CONFIG
- WRITES_CONFIG
- SIMILAR_TO
- TOUCHES
- DECIDES

Every edge should support:
- confidence
- source/provenance
- created_at
- updated_at

## Retrieval Requirements

Use hybrid retrieval. Never rely on semantic search alone.

Ranking should include:
- exact symbol match
- FTS/BM25 lexical score
- semantic similarity
- graph distance
- active session relevance
- recency
- centrality
- test relevance

Context packs must:
- be token-budget aware
- deduplicate snippets
- include why each item was included
- include expansion handles
- avoid full-file dumps by default

## UI Requirements

The localhost UI must remain separate from the MCP hot path.

Features:
- dashboard
- project list
- indexing monitor
- graph explorer
- dependency path view
- call graph view
- community map
- token savings dashboard
- session memory viewer
- tool playground
- config editor
- logs viewer
- cache inspector
- diagnostics

## Performance Requirements

Optimize for:
1. low MCP query latency
2. token reduction
3. fast startup
4. low RAM
5. incremental indexing speed

Use:
- bounded graph expansion
- hot cache
- WAL mode
- prepared statements
- batched writes
- cancellation tokens
- bounded worker pools

Avoid:
- semantic-only search
- full-file dumps
- unbounded result lists
- global mutable state
- sync mutexes in hot paths
- blocking IO in MCP hot paths
