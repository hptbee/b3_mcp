---
name: embeddings
description: Use when implementing semantic search, embedding pipelines, or vector retrieval.
---

# Embedding Skill

Embeddings must run in background workers.

Preferred local embedding providers:
- Ollama
- local GGUF embedding models
- sentence-transformers
- Candle
- fastembed

Do NOT require OpenAI embeddings.

Cloud embedding providers must remain optional plugins.

Preferred vector DB:
- local Qdrant

Avoid:
- synchronous embedding generation
- cloud-only embedding architectures
