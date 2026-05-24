# Project Plan

## Vision

A local-first/offline-first AI-native code intelligence platform for coding agents such as Cursor, Codex, Claude-compatible MCP clients, and local developer workflows.

It combines:

- AST-aware indexing
- persistent code graph
- SQLite/FTS storage
- query engine
- context packing
- token-saving retrieval
- MCP runtime
- control server
- localhost UI
- graph explorer
- query trace UI

## Hard Requirement: Offline and Free

B3 must work fully offline and free by default.

Core features must not require:
- external APIs
- cloud services
- hosted vector databases
- SaaS authentication
- telemetry
- paid services
- paid UI kits
- paid backend services
- paid/proprietary plugins
- OpenAI / Anthropic / Gemini / cloud embedding APIs
- JetBrains paid plugin
- proprietary paid plugins

External providers are allowed only as optional plugins, disabled by default.
- local SQLite
- local FTS5
- local parser worker
- local LSP servers
- local command output compaction
- local benchmark harness
- local embeddings only when implemented later
- local Qdrant only as optional component

## Completed Phases

- Phase 1 - Workspace / Scaffold
- Phase 1.5 - Contracts / Boundaries
- Phase 2 - SQLite Storage / Schema Foundation
- Phase 3 - Incremental Indexer Skeleton
- Phase 3.1 - Indexer Audit / Cleanup
- Pre-Phase-4 - Plugin Contracts / Docs / CI
- Phase 4 - Real Rust Parsing + Storage Adapter
- Phase 4.1 - Project/Branch Auto Ensure + Deleted File Cleanup
- Phase 5 - Query Engine + Graph Traversal + Context Pack
- Phase 5.1 - Query Hardening + Retrieval Explainability
- Phase 5.2 - Ranking Algorithms Upgrade
- Phase 6 - MCP Tools over Query Engine
- Phase 6.0.1 - Live MCP Runtime Wiring
- Phase 6.1 - Impact Intelligence
- Phase 6.2 - PageRank / Centrality
- Phase 6.3 - MCP Runtime Hardening + Real-world Smoke Test
- Phase 7 - Control Server + Localhost API
- Phase 7.1 - Web UI Foundation
- Phase 7.2 - Graph Explorer UI
- Phase 7.2.1 - Real Graph API Wiring
- Phase 7.3 - Query Trace UI
- Phase 8 - File Watcher + Daemon Mode
- Phase 8.1 - Parser Isolation
- Phase 8.2 - Benchmark Harness + Performance Baseline
- Phase 8.3 - Refactor Checkpoint A
- Phase 8.4 - Performance Optimization Pass A
- Phase 8.5 - Command Output Compaction

## Current / Next Phase

- **Current**: Phase 8.5 - Command Output Compaction (complete)
- **Next**: Phase 8.6 - MCP Tool Profiles + Manifest Slimming

## Upcoming Roadmap

### Phase 8.6 - MCP Tool Profiles + Manifest Slimming

### Phase 8.7 - Agent Install Helper + Hook Integration

### Phase 8.8 - Multi-repo Registry

### Phase 9.0 - Language Pack Architecture

Include:

- LanguagePack trait
- LanguageRegistry
- extension-to-language resolution
- capability discovery
- unsupported language fallback

### Phase 9.1 - LSP Backend MVP
Priority:
- C#
- TypeScript
- JavaScript
- Angular where possible

### Phase 9.2 - Backend Language Packs and Domain Intelligence

#### 9.2.1 – Node.js / REST API Intelligence
Support:
- Express
- NestJS
- Fastify basic
- REST handlers
- API routes
- middleware
- controllers
- services

Graph nodes:
- ApiRoute
- Controller
- Middleware
- Service
- Handler

Edges:
- ROUTES_TO
- HANDLES_ROUTE
- USES_MIDDLEWARE
- CALLS_SERVICE

#### 9.2.2 – Messaging / Event‑driven Intelligence
Support:
- Kafka
- ksqlDB
- RabbitMQ

Kafka detection:
- topics, producers, consumers, consumer groups, ksql streams/tables, joins, windows

RabbitMQ detection:
- queues, exchanges, bindings, routing keys, publishers, consumers, dead‑letter queues

Graph nodes:
- Topic, Queue, Exchange, RoutingKey, ConsumerGroup, Stream, Table, MessageContract, Producer, Consumer

Edges:
- PRODUCES_TO, CONSUMES_FROM, PUBLISHES_TO, BINDS_TO, ROUTES_TO, JOINS_STREAM, READS_TOPIC, WRITES_TOPIC, USES_CONTRACT

#### 9.2.3 – Docker / Runtime Infrastructure Intelligence
Support:
- Dockerfile, docker‑compose.yml, .env, deployment configs

Detect:
- services, images, ports, volumes, env vars, networks, depends_on, healthchecks, build contexts

Graph nodes:
- DockerService, DockerImage, Port, Volume, Network, EnvironmentVariable, Container

Edges:
- DEPENDS_ON, EXPOSES_PORT, USES_VOLUME, USES_NETWORK, BUILDS_FROM, USES_ENV, RUNS_SERVICE

#### 9.2.4 – SignalR / Real‑time Communication Intelligence
Support:
- ASP.NET SignalR hubs, hub methods, client events, groups, frontend SignalR client handlers

Detect:
- Hub classes, Hub methods, Clients.All/Caller/Group, Groups.AddToGroupAsync, SendAsync event names, frontend .on handlers, frontend .invoke calls

Graph nodes:
- SignalRHub, HubMethod, ClientEvent, Group, RealtimeConnection

Edges:
- EMITS_EVENT, HANDLES_EVENT, JOINS_GROUP, SENDS_TO_CLIENT, CALLS_HUB, USES_CONNECTION

#### 9.2.5 – C# WPF Desktop App Intelligence
Support:
- WPF XAML, Windows, Pages, UserControls, ViewModels, Commands, Bindings, Resources, Event handlers

Detect:
- Window, Page, UserControl, ViewModel, ICommand/RelayCommand, Binding paths, ResourceDictionary, XAML event handlers, navigation targets

Graph nodes:
- WpfWindow, WpfPage, WpfUserControl, ViewModel, Command, Binding, Resource, EventHandler

Edges:
- BINDS_TO, USES_VIEWMODEL, HANDLES_EVENT, USES_RESOURCE, NAVIGATES_TO, COMMAND_EXECUTES

#### 9.2.6 – Three.js / WebGL Graphics Intelligence (React Integration)
Support:
- Three.js scenes, meshes, cameras, render loops within React components

Detect:
- Canvas elements, three.js imports, scene construction, animation loops, shader usage

Graph nodes:
- ThreeScene, Mesh, Camera, Light, Shader, ReactComponent

Edges:
- RENDERS_IN, CONTAINS_MESH, ATTACHES_CAMERA, USES_SHADER, EMBEDDED_IN_COMPONENT

### Phase 9.3 – Symbolic Editing MVP

### Phase 9.4 – Rename / Refactor MVP

### Phase 10 – Local Embeddings + Vector Search
Include:
- local embedding provider abstraction
- Ollama / fastembed / Candle support
- local Qdrant only
- no cloud embeddings required
- semantic signal remains secondary to AST/graph/FTS

### Phase 10.2 – Session Memory + Context Virtualization

### Phase 11 – Architecture Intelligence
Include:
- community detection
- module boundary detection
- dependency cluster map
- circular dependency reports
- architecture layer detection

### Phase 12 – Git Intelligence
Include:
- recent churn score
- last modified commit
- commit frequency
- hotspot files
- author count
- ranking/risk integration

### Phase 13 – Duplicate / Similarity Detection
Include:
- AST fingerprinting
- normalized AST hash
- MinHash
- SimHash
- duplicate function detection
- similar code search

### Phase 14 – Real Plugin System
Include:
- plugin registry runtime
- plugin lifecycle
- capability discovery
- language plugin loading
- ranking plugin loading
- embedding plugin loading
- external providers optional and disabled by default

### Phase 15 – Packaging + Installers
Include:
- mcp install command
- Cursor config helper
- Codex config helper
- doctor command
- uninstall command
- optional Tauri app

## Refactor Rules

- refactor only after verified feature milestones
- prefer small targeted refactors
- do not rewrite architecture unnecessarily
- do not genericize too early
- keep MCP runtime thin
- preserve offline-first architecture
- preserve public DTO compatibility where possible

## Optimization Rules

- benchmark first
- optimize measured bottlenecks only
- preserve regression benchmarks
- avoid speculative optimization
- no performance work without before/after measurement

## Benchmark Strategy

Benchmarks should establish a baseline before optimization work starts.

Track:

- cold startup time
- MCP tools/list latency
- common query latencies
- graph traversal latency
- context pack latency
- indexing speed
- changed‑file reindex latency
- watcher debounce latency
- SQLite query latency
- approximate memory use

Benchmark results should be compared before and after every optimization pass.

## Multi‑Language Strategy

Target: around 20 languages.

Do not implement all languages in one phase.

Use support levels:

### Basic:

- file detection
- top‑level symbols
- imports
- FTS support

### Good:

- methods
- classes
- tests
- basic calls
- references

### Advanced:

- framework routes
- DI relationships
- cross‑file references
- inheritance/interface edges
- component relationships

## Priority:

- F&B project first: C#, TypeScript, React, Angular
- broad basic support later
- advanced support only for important languages

## Documentation Sync Rule

After every phase, update relevant markdown files before marking the phase complete.

Review:

- README.md
- PLAN.md
- REQUIREMENTS.md
- ALGORITHM_ANALYSIS.md
- DEVELOPMENT.md
- MCP_TOOLS.md
- CONTROL_SERVER.md
- WEB_UI.md
- OFFLINE_REQUIREMENTS_PATCH.md
- AGENTS.md
- .skills/*/SKILL.md

Only update affected files.

## When can we use it?

- **Rust repo**: usable now
- **C# Web API**: Phase 9.1 / 9.2
- **React / Angular**: Phase 9.2
- **Node.js REST API**: Phase 9.2.1
- **Kafka / ksqlDB**: Phase 9.2.2
- **RabbitMQ**: Phase 9.2.2
- **Docker / docker‑compose**: Phase 9.2.3
- **SignalR**: Phase 9.2.4
- **C# WPF**: Phase 9.2.5
- **Three.js / WebGL**: Phase 9.2.6
- **Multi‑repo workspace**: Phase 8.8
- **Refactor assistant**: Phase 9.3 / 9.4
- **Full memory/context platform**: Phase 10.2+

## Reference Models and Borrowed Ideas

**Note**: B3 can be run today as a local MCP/runtime/control/UI platform, but full app‑stack intelligence depends on Phase 9.x.

- **codebase-memory-mcp** – tree‑sitter graph, persistent local code memory, MCP code intelligence.
  *Learned*: incremental AST graph, persistent storage, offline graph queries.
  *Phase*: 8.1‑8.6 (graph foundation) and 9.0‑9.2 (language pack integration).
  *Difference*: B3 adds multi‑repo support, token‑saving, and tool‑profile manifest slimming.

- **TokenSave** – token‑saving retrieval, context packing, avoiding repeated grep/read.
  *Learned*: compact context packs, cache token usage.
  *Phase*: 8.5 (Command Output Compaction) and 9.1‑9.2 (context packing).
  *Difference*: B3 integrates token‑saving at the graph level and ties it to benchmark‑driven development.

- **RTK** – command output compaction for git/test/build/lint outputs.
  *Learned*: unified output handling, compact transcript.
  *Phase*: 8.5 (Command Output Compaction).
  *Difference*: B3 extends compaction to all MCP tool outputs and adds manifest slimming.

- **Context Mode** – session continuity, context virtualization, compact/resume memory.
  *Learned*: session memory, virtualization of context.
  *Phase*: 10.2 (Session Memory + Context Virtualization).
  *Difference*: B3 builds on this with offline‑first storage and optional plugins.

- **Token Savior** – tool profiles, manifest slimming, command compaction, transcript discovery, benchmark‑driven development.
  *Learned*: profile‑based tool selection, manifest reduction.
  *Phase*: 8.6 (MCP Tool Profiles + Manifest Slimming) and 10.2+.
  *Difference*: B3 keeps profiles local and auto‑generates them from graph analysis.

- **CodeGraph** – multi‑language graph MCP, framework‑aware routes, with/without MCP benchmarks.
  *Learned*: language‑agnostic graph, framework route detection.
  *Phase*: 9.0‑9.2 (Language Backend Architecture & Domain Intelligence).
  *Difference*: B3 adds offline‑first persistent storage and token‑saving layers.

- **GitNexus** – multi‑repo registry, setup/install UX, bridge mode, repo groups/multi‑service analysis.
  *Learned*: registry of many repositories, cross‑repo analysis.
  *Phase*: 8.8 (Multi‑repo Registry).
  *Difference*: B3 integrates registry directly into MCP runtime with optional UI.

- **Serena** – LSP backend, symbolic editing, rename/refactor, IDE‑grade semantic operations.
  *Learned*: LSP integration, symbolic edits.
  *Phase*: 9.1 (LSP Backend MVP) and 9.3‑9.4 (Symbolic Editing & Refactor MVP).
  *Difference*: B3 focuses on offline‑first, token‑saving, and benchmark‑driven quality.

- **Neo4j Browser** – graph explorer UX, node/edge inspector, path/cycle visualization.
  *Learned*: interactive graph UI, inspection features.
  *Phase*: 7.2‑7.3 (Graph Explorer UI & Query Trace UI).
  *Difference*: B3 uses a lightweight React Flow UI without external graph DB dependencies.

- **Sourcegraph / Cursor‑style systems** – code intelligence, impact analysis, context retrieval.
  *Learned*: global impact analysis, fast context retrieval.
  *Phase*: 6‑9 (MCP Tools, Language Packs, Impact Intelligence).
  *Difference*: B3 stays fully offline, stores everything locally, and avoids SaaS APIs.

---

**Notes**

- Do not implement all domains at once.
- Each domain must have tests and benchmark fixtures.
- Framework detection should be conservative and explainable.
- No cloud APIs are required.
- Offline‑first and free‑by‑default remain hard requirements.
