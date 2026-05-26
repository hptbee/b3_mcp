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

## Current Status

Completed:
- Phase 9.2.12 — .NET Desktop / WPF Intelligence
- Phase 10.0 — Local Embeddings + Vector Search Architecture
- Phase 10.1 — Local Embedding Provider MVP
- Phase 10.2 — SQLite Vector Storage / Search Index
- Phase 10.3 - Hybrid Search Ranking
- Phase 10.4 - MCP / Control API Integration
- Phase 10.5 - Benchmark + Quality Evaluation
- Phase 11.0 - Cross-Project Architecture Model + Contracts
- Phase 11.1 - Group Query Federation

Current / Next:
- Phase 11.2 - Cross-Repo Route / API Matching

Upcoming (major):
- Phase 11.3 - Cross-Repo Messaging Matching
- Phase 11.4 - Cross-Repo Package / Contract / Infra Matching
- Phase 12 — Symbolic Editing MVP
- Phase 13 — Rename / Refactor MVP

See `PLAN.md` for the full, authoritative roadmap and phase details.

---

## What Works Today

Core platform:
- Repository indexing into `.b3/b3.db` (local project model).
- MCP runtime (stdio) with curated tool profiles.
- Local control server (`http://127.0.0.1:7777`) and local web UI (`http://127.0.0.1:8888`).
- Scoped indexing (path/file/glob/language/framework).
- Local registry and metadata-first project groups (local JSON registry).

Language and framework intelligence (basic/static/local):
- Rust (best support)
- JavaScript / TypeScript / JSX / TSX
- Node.js REST (Express / NestJS / Fastify)
- React / Next.js / Angular (static extraction)
- ASP.NET Core / C# Web API
- Go (basic static support)

Application intelligence (static hints):
- ORM / data-access (EF Core, Dapper, Prisma, TypeORM, Sequelize)
- Realtime / Socket (WebSocket, Socket.IO, SignalR)
- Messaging/event-driven (AMQP/RabbitMQ, Kafka, Google Pub/Sub)
- Cloud / infrastructure hints (Docker, Compose, Kubernetes, Terraform)
- C# WPF / XAML (basic static/local)

Vector foundation:
- `local_hash` embeddings and SQLite vector persistence/search (local/offline raw vector search).
- Hybrid ranking combines lexical, vector, and metadata signals inside `b3-query`.
- Local hybrid search is exposed through `POST /api/search/hybrid` and the MCP `semantic_search` tool.
- Fixture-based benchmark/quality evaluation is available through `cargo run -p b3-bench -- baseline`.
- Production-grade neural embedding providers are planned in later optional-provider phases.

Cross-project architecture foundation:
- Phase 11.0 adds serializable local architecture contracts for projects, groups, services, nodes, edges, match candidates, confidence, provenance, and normalization.
- Phase 11.1 resolves local registry groups and federates read-only summaries across repo-local `.b3/b3.db` files.
- B3 still preserves `1 project = 1 repo-local .b3/b3.db`; matching is deferred to later Phase 11 subphases.

---

## Current Limitations

- Support for non-Rust stacks is mostly conservative/static/local, not full semantic analysis.
- `local_hash` embeddings are lexical/hash-based, not neural semantic-quality vectors.
- MCP/control semantic search is local/offline and uses lexical/hash-based `local_hash`, not neural semantic vectors.
- Quality metrics are fixture baselines, not production guarantees.
- Cross-project relationship matching and service maps are not usable yet; Phase 11.1 only adds group federation/status/summary.
- Symbolic editing / rename & refactor: Phase 12 / Phase 13.
- B3 does not execute code or run cloud/broker/database/package-manager commands by default.

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
| Hybrid semantic ranking | Usable now, local/offline |
| MCP semantic search tool | Usable now, local/offline |
| Search quality benchmark | Usable now, local fixture baseline |
| Cross-project architecture contracts | Usable now, model/status only |
| Cross-project group federation | Usable now, read-only summaries |
| Cross-project matching | Phase 11.2+ |
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
