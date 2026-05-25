# B3 — Local Code Intelligence (Concise)

B3 is a local-first, offline-first, free-by-default code intelligence platform
for MCP-compatible coding agents and local developer workflows. `PLAN.md` is
the source of truth for the full roadmap and detailed phase notes.

Quick links:
- Roadmap (source of truth): PLAN.md
- Dev docs: DEVELOPMENT.md
- MCP tools: MCP_TOOLS.md

---

## What B3 Is

- A Rust-native indexer that builds a local SQLite-backed code graph (.b3/b3.db).
- A thin MCP runtime exposing a curated toolset for agent integrations.
- A localhost control server and a local Next.js web UI for inspection and manual workflows.
- Scoped indexing and conservative, explainable static intelligence for supported stacks.

## Roadmap (short)

Current / Next:
- Phase 10.3 — Hybrid Search Ranking (current/next)

Upcoming major phases:
- Phase 10.4 — MCP / Control API Integration
- Phase 10.5 — Benchmark + Quality Evaluation
- Phase 11 — Cross-Project Architecture Intelligence
- Phase 12–20 — Symbolic editing, refactor assistant, additional language and UI work

See `PLAN.md` for the full, authoritative roadmap and phase details.

---

## What Works Today (concise)

- Repository indexing into `.b3/b3.db` (local project model).
- MCP runtime (stdio) and curated MCP tool profiles for agent testing.
- Local control server (`http://127.0.0.1:7777`) and local web UI (`http://127.0.0.1:8888`).
- Scoped indexing (path / file / glob / language / framework filters).
- Registry and project-groups metadata (local only, metadata-first).
- Language & technology support (basic/static/local):
  - Rust (best support)
  - JavaScript / TypeScript / JSX / TSX
  - Node.js REST (Express / NestJS / Fastify)
  - React / Next.js / Angular (basic static extraction)
  - ASP.NET Core / C# Web API
  - ORM / data-access hints (EF Core, Dapper, Prisma, TypeORM, Sequelize)
  - Realtime / Socket hints (WebSocket, Socket.IO, SignalR)
  - Messaging/event-driven hints (AMQP/RabbitMQ, Kafka, Google Pub/Sub)
  - Cloud/infrastructure hints (Docker, Compose, Kubernetes, Terraform)
  - Go (basic static support)
  - C# WPF / XAML (basic static/local)

- Local embeddings & vectors (Phase 10 work): `local_hash` embeddings and
  SQLite vector storage/search are implemented (local/offline raw vector search);
  production semantic embedding providers, hybrid ranking, and MCP semantic tools
  are staged in later phases.

---

## Current Limitations (short)

- Most non-Rust stacks are supported with conservative, static extraction (not full semantic analysis).
- `local_hash` embeddings are lexical/hash-based, not neural semantic-quality vectors.
- Hybrid ranking and semantic integration are in Phase 10.3–10.4 (not yet general-purpose).
- Quality benchmarking and user-facing quality reports are Phase 10.5.
- Cross-project architecture intelligence is Phase 11.
- Symbolic editing and rename/refactor are Phase 12 / Phase 13.
- B3 does not execute code, call cloud APIs, or connect to external brokers/databases by default.

---

## When Can We Use It? (short table)

| Use case | Status |
|---|---|
| MCP runtime (Codex/Cursor) | Usable now |
| Local repository indexing (.b3/b3.db) | Usable now |
| Project init / index / reindex | Usable now |
| MCP tool profiles | Usable now |
| Multi-project local workflow / groups | Usable now, metadata-only |
| JS/TS / JSX / TSX indexing | Usable now, basic/static |
| Node.js REST route intelligence | Usable now, basic/static |
| React / Next.js / Angular | Usable now, basic/static |
| C# Web API / ASP.NET Core | Usable now, basic/static |
| ORM / data-access hints | Usable now, basic/static |
| Realtime / Socket hints | Usable now, basic/static |
| Messaging (Kafka/RabbitMQ) | Usable now, basic/static |
| Docker / Compose / Kubernetes / Terraform | Usable now, basic/static |
| SignalR | Usable now, basic/static |
| Scoped indexing targets | Usable now |
| C# WPF / XAML | Usable now, basic/static/local |
| Local embeddings & SQLite vector search | Usable now, local/offline raw vector search |
| Hybrid semantic ranking | Phase 10.3 |
| MCP semantic search tool | Phase 10.4 |
| Cross-project architecture intelligence | Phase 11 |
| Refactor assistant / rename & refactor | Phase 12 / Phase 13 |

Refer to `PLAN.md` for complete phase definitions and caveats.

---

## Quick Start (keeps core commands)

Run checks and tests:

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

Initialize and index a repository (example):

```powershell
cargo run -p b3-control --bin b3-control-server -- init --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db"
```

Run the control server:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

Run the MCP runtime:

```powershell
cargo run -p b3-mcp-runtime -- serve --project "." --database ".b3/b3.db"
```

Start the web UI (local):

```powershell
cd apps/web-ui
npm install
npm run dev
# web UI: http://127.0.0.1:8888
```

---

## Project Model (ASCII)

Business Application
├── Backend API
├── Frontend App
├── Worker Service
├── Desktop Client
└── Runtime Infrastructure

---

## Offline / Free Guarantee

B3 remains offline-first and free-by-default. Core features do not require any
external APIs, cloud services, hosted vector DBs, OpenAI/cloud embedding APIs,
telemetry, or internet access. Optional external or paid integrations are
explicit plugins disabled by default.

---

For detailed roadmap, phase status, and implementation notes see `PLAN.md`.
