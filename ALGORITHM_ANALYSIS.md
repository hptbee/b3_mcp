# Algorithm Analysis and Improvements

## Ideas Taken From TokenSave

TokenSave focuses on replacing agent exploration with compact local graph queries.

Strategies adopted:
- pre-indexed semantic knowledge graph
- FTS5/local database lookup
- one-call smart context building
- semantic search by meaning
- impact analysis through callers/callees
- file watcher / daemon mode
- branch-aware indexing
- cross-session memory
- token savings ledger
- guardrails to reduce wasteful grep/read-file exploration
- atomic edit primitives with re-index after writes

## Ideas Taken From Codebase Memory MCP

Codebase Memory MCP focuses on persistent tree-sitter knowledge graphs for MCP clients.

Strategies adopted:
- tree-sitter based parsing
- persistent knowledge graph
- multi-phase indexing pipeline
- parallel worker pools
- call-graph traversal
- impact analysis
- community discovery / graph clustering
- graph-native query support

## Algorithms To Apply

### Incremental File Hash Indexing

Use content hashes to skip unchanged files.

Algorithm:
1. walk repo with ignore rules
2. compute file hash
3. compare against stored hash
4. only parse changed/deleted/new files
5. update affected graph nodes, edges, FTS entries, and caches

Improvement:
- store per-symbol hash
- avoid rewriting unchanged symbol nodes

### Tree-Sitter AST Symbol Extraction

Use tree-sitter queries per language.

Extract:
- functions
- methods
- classes
- structs
- interfaces
- enums
- modules
- imports
- routes
- tests
- config keys

Improvement:
- add language packs as plugins
- support fallback text extraction for unsupported languages

### Relationship Extraction

Build graph edges from AST and import analysis.

Edges:
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

Improvement:
- confidence scoring per edge
- edge provenance: AST, import analysis, text heuristic, semantic, user recorded

### Hybrid Retrieval

Do not rely on embeddings alone.

Retrieval score should combine:
- exact symbol match
- lexical FTS/BM25 score
- semantic similarity
- graph proximity
- active session relevance
- recency
- centrality
- test relevance

Improvement:
- adaptive weights by query intent
- exact symbol queries favor graph/AST
- conceptual queries favor semantic + graph

### Context Pack Compression

Return compact context instead of raw files.

Pipeline:
1. classify query intent
2. retrieve candidates
3. graph expand with depth limits
4. rerank
5. deduplicate
6. summarize
7. pack under token budget
8. include expansion handles

Context packs must include why each item was included.

### Impact Analysis

Given a symbol:
1. find direct callers
2. find callees
3. expand references by bounded depth
4. include tests that cover affected area
5. include config/routes impacted
6. rank by risk

Risk score factors:
- fan-in
- fan-out
- centrality
- dependency depth
- test gap
- recent churn
- public API exposure

### Community Discovery

Use graph clustering to detect modules/domains.

Possible algorithms:
- Louvain community detection
- Label Propagation
- strongly connected components
- weakly connected components

Use cases:
- summarize module boundaries
- detect architecture layers
- detect circular dependencies
- improve retrieval ranking

### Centrality and Hotspot Ranking

Compute:
- PageRank
- betweenness centrality
- in-degree / out-degree
- dependency fan-in/fan-out

Use cases:
- identify critical files
- rank context
- improve impact analysis
- detect risky changes

### Token Savings Ledger

Track every MCP call.

Store:
- query type
- estimated raw exploration tokens
- returned tokens
- avoided tool calls
- latency
- files avoided
- cache hit/miss

Expose:
- per-project savings
- per-session savings
- compression ratio
- avoided grep/read-file counts

### Crash-Isolated Parser Workers

Tree-sitter grammars may fail or panic.

Use subprocess workers:
- parent process queues file jobs
- worker parses one file or batch
- if worker crashes, parent logs file and respawns
- sync continues

Improvement:
- mark file as parse_failed with reason
- retry after grammar/version changes

### Branch-Aware Indexing

Store separate graph snapshots per branch or worktree.

Use:
- branch name
- git commit hash
- working tree dirty state
- per-branch database namespace

Use cases:
- compare branches
- avoid polluted graph from branch switching
- cross-branch search

## Improvements Beyond Both Projects

Added requirements:
- strict offline-first default
- no required cloud APIs or hosted databases
- local embeddings by default
- local Qdrant only
- plugin-only cloud integrations
- disabled-by-default external services
- edge confidence and provenance
- adaptive ranking weights
- token-budget context packing
- expansion handles instead of large blobs
- UI-visible diagnostics
- token savings dashboard
- dependency path visualization
- test-gap risk scoring
- architecture boundary detection
- duplicate code cluster detection
- generated/vendor classifiers
- profile-based language/indexing depth
