---
name: offline-first
description: Use when implementing storage, embeddings, telemetry, retrieval, or external integrations.
---

# Offline-First Skill

This platform must remain fully functional offline.

Core features must never require:
- internet connectivity
- cloud APIs
- hosted databases
- SaaS authentication

Default runtime:
- local storage
- local embeddings
- local vector search
- local telemetry

Preferred local providers:
- SQLite
- local Qdrant
- Ollama
- GGUF embedding models
- sentence-transformers
- Candle
- fastembed

External integrations:
- optional
- plugin-based
- disabled by default

Never hardcode API keys or cloud endpoints.
