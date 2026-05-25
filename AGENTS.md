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

## LSP Backend Boundaries

LSP complements the tree-sitter graph; it does not replace indexing, SQLite graph storage, FTS, query ranking, context pack generation, or Rust parser behavior.

LSP must remain:
- local-only
- disabled by default
- free-by-default
- configured explicitly
- non-fatal when a language server is missing

Do not install or download language servers. Do not add cloud, telemetry, paid/proprietary, symbolic editing, rename/refactor, semantic search, embeddings, or cross-project architecture intelligence while working in Phase 9.1.

## Web Language Boundaries

Phase 9.2 web language support is basic indexing only. JavaScript, TypeScript, JSX, and TSX may use bundled local tree-sitter parsers for symbols/imports, but indexing must not run `npm`, `node`, `tsc`, `eslint`, cloud parsers, or framework CLIs.

Do not add Node.js REST route intelligence, React hook/component graph intelligence, Angular route/template/module intelligence, C# semantic intelligence, JS/TS symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.2.

## Node.js REST Boundaries

Phase 9.2.1 Node.js REST intelligence is completed as basic local static analysis only. It may detect package.json dependencies and high-confidence Express, NestJS, and Fastify route declarations, but it must not execute `node`, `npm`, `tsc`, `eslint`, Nest CLI, app code, package-manager scripts, package registries, or cloud parsers.

Do not add React graph intelligence, Angular intelligence, ASP.NET Core/C# intelligence, Go support, realtime/socket intelligence, messaging intelligence, cloud/infrastructure intelligence, symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.2.1.

## React / TSX Boundaries

Phase 9.2.2 React / TSX component intelligence is completed as basic local static analysis only. It may detect common React components, props type names, JSX component usages, and hook names, but it must not execute `node`, `npm`, `tsc`, `eslint`, React dev servers, app code, package-manager scripts, package registries, or cloud parsers.

Do not add Angular intelligence, Vue/Svelte intelligence, ASP.NET Core/C# intelligence, Go support, realtime/socket intelligence, messaging intelligence, cloud/infrastructure intelligence, symbolic editing, rename/refactor, embeddings, semantic search, or cross-project architecture intelligence in Phase 9.2.2.

## Next.js Intelligence Boundaries

Phase 9.2.3 Next.js intelligence is completed as basic local static analysis
only on top of the completed React / TSX support. It may inspect local
`package.json`, `next.config.*`, `app/`, and `pages/` files, detect common
file-system routes and app route-handler method exports, and mark basic
`"use client"` / `"use server"` boundaries, but it must not run `next dev`,
`next build`, `node`, `npm`, `tsc`, `eslint`, package scripts, deployment
tooling, package registries, cloud parsers, or app code. It does not implement
full RSC semantics, middleware execution order, Vercel/deployment intelligence,
auth intelligence, or deep data fetching semantics.

## Angular Intelligence Boundaries

Phase 9.2.4 Angular intelligence is completed as basic local static analysis
only on top of TypeScript support. It may inspect local `package.json`,
`angular.json`, TypeScript decorators, route config object literals, and
literal template/style references, but it must not run `ng`, Angular compiler,
`node`, `npm`, `tsc`, `eslint`, package scripts, package registries, cloud
parsers, or app code. It does not implement full template type checking,
runtime lifecycle semantics, deep DI/module graph resolution, RxJS/NgRx flow,
or Angular Material intelligence.

Do not start ASP.NET Core / C# Web API intelligence until Phase 9.2.5.
