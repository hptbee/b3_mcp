# Phase 5.1 Query Hardening

Phase 5.1 keeps query functionality library-local. It does not add MCP tools,
embeddings, UI, file watching, parser subprocesses, or additional language
packs.

## Future MCP Mapping

The query crate exposes storage-agnostic methods and serializable DTOs that can
be wrapped by MCP tools later:

- `find_symbol` -> `FindSymbolResponse`
- `search_code` -> `SearchCodeResponse`
- `find_callers` -> `FindCallersResponse`
- `find_callees` -> `FindCalleesResponse`
- `related_symbols` -> `RelatedSymbolsResponse`
- `impact_analysis` -> `ImpactAnalysisResponse`
- `get_context_pack` -> `ContextPackResponse`

All responses can include a `QueryTraceDto` when callers request trace output.
The trace records scope, intent, hits, traversal steps, ranking decisions,
context selection/skips, truncation, token budget usage, savings estimates, and
warnings such as non-fatal token ledger write failures.

## Boundaries

- Query remains branch-aware through `QueryScope`.
- Traversal is bounded, cycle-safe, edge-filtered, and confidence-aware.
- Context packs use symbol snippets, not full-file dumps by default.
- Token savings writes are best-effort and must not fail the main query path.
- Semantic/vector ranking remains deferred.
