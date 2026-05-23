---
name: architecture
description: Use when making system design, boundary, runtime, or product architecture decisions.
---

# Architecture Skill

Keep the original architecture:
- Rust backend/core
- Next.js UI
- React Flow graph visualization
- Tauri optional packaging

The architecture must remain fully functional offline.

Core capabilities must not depend on:
- external APIs
- remote inference
- cloud databases
- internet connectivity

Local-first is a hard requirement.

Use hybrid monolith:
- lightweight MCP runtime
- shared core engine
- background workers
- optional control server
- optional localhost UI

Hard boundary:
MCP runtime is protocol only. Heavy work belongs to core services.
