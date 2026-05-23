# Offline-First Patch

## Hard Requirement

The MCP platform must function fully offline.

The system must NOT require:
- external APIs
- cloud authentication
- hosted vector databases
- remote telemetry
- SaaS dependencies

All core functionality must work locally:
- indexing
- graph traversal
- retrieval
- token savings
- UI
- diagnostics
- embeddings

Preferred local providers:
- Ollama
- GGUF embedding models
- sentence-transformers
- Candle
- fastembed
- local Qdrant
- SQLite/libSQL local mode

External APIs may be optionally supported through plugins only.

The default installation must remain fully usable without internet access.

## Forbidden by Default

Do not require:
- OpenAI APIs
- Anthropic APIs
- Gemini APIs
- hosted Qdrant
- cloud telemetry
- SaaS authentication

Cloud integrations must:
- be optional
- be disabled by default
- use plugin architecture
