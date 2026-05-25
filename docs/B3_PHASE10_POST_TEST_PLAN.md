# B3 Post-Phase-10 Test Plan

> Mục tiêu: dùng file này để test lại toàn bộ B3 sau khi hoàn tất **Phase 10 — Local Embeddings + Vector Search**.
>
> Phạm vi test bao gồm: regression test, integration test, smoke test, API test, MCP test, CLI test, indexing test, language/framework intelligence test, offline/free compliance test, và benchmark test.

---

## 1. Expected Project State After Phase 10

Sau Phase 10, B3 dự kiến đã có các nhóm capability sau:

- Core local indexing and graph storage
- SQLite/FTS storage
- Rust indexing
- JavaScript / TypeScript / JSX / TSX basic indexing
- Node.js REST route intelligence
- React / TSX component intelligence
- Next.js intelligence, nếu Phase 9.2.3 đã hoàn tất trước đó
- Angular / C# / ORM / Realtime / Messaging / Cloud / Go tùy theo phase đã hoàn tất trước Phase 10
- LSP backend foundation, disabled by default
- MCP runtime with tool profiles
- CLI helper and registry
- Control server APIs
- Web UI basic local console
- Local embeddings and vector search
- No required cloud/API/telemetry/paid dependency

---

## 2. Hard Requirements

B3 phải luôn giữ các yêu cầu sau:

- Offline-first
- Free-by-default
- Local-only by default
- No required external APIs
- No required cloud services
- No hosted vector database required
- No SaaS authentication required
- No telemetry
- No paid dependency
- No required internet access at runtime
- External/cloud/paid integrations chỉ được là optional plugin, disabled by default

---

## 3. Test Environment

### 3.1 Recommended Environment

```powershell
# Windows example
cd D:\Project\b3_mcp
```

### 3.2 Required Local Commands

```powershell
cargo --version
rustc --version
```

Optional nếu frontend cần verify:

```powershell
node --version
npm --version
```

### 3.3 Important Ports

```text
Control server: http://127.0.0.1:7777
Web UI:        http://127.0.0.1:8888
```

### 3.4 Database Path

Default local DB:

```text
.b3/b3.db
```

### 3.5 Registry Path

Default local registry:

```text
~/.b3/registry.json
```

Use temporary registry for tests when possible:

```powershell
$env:B3_HOME="D:\Temp\b3-test-home"
```

---

## 4. Global Verification Commands

Run these after Phase 10 before deeper testing.

```powershell
rg -n "<<<<<<<|=======|>>>>>>>" .
cargo fmt
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo run -p b3-bench -- baseline
```

Expected result:

- No conflict markers
- Formatting passes
- Workspace check passes
- All tests pass
- Benchmark baseline runs successfully
- Benchmark JSON is written to `target/benchmarks/baseline.json`

---

## 5. Core Regression Test Cases

| ID | Area | Test Case | Steps | Expected Result |
|---|---|---|---|---|
| CORE-001 | Conflict markers | Scan unresolved conflict markers | `rg -n "<<<<<<<|=======|>>>>>>>" .` | No results |
| CORE-002 | Formatting | Rust format check | `cargo fmt --check` | Pass |
| CORE-003 | Build | Workspace build check | `cargo check --workspace` | Pass |
| CORE-004 | Tests | Full workspace tests | `cargo test --workspace` | Pass |
| CORE-005 | Benchmark | Baseline benchmark | `cargo run -p b3-bench -- baseline` | Pass and writes JSON |
| CORE-006 | Offline/free | Dependency policy check | Inspect new dependencies and docs | No required cloud/API/telemetry/paid dependency |
| CORE-007 | Runtime safety | No command execution added by indexing/querying | Inspect code paths and run smoke | No shell/npm/node/tsc/eslint/next execution required |

---

## 6. Storage / Database Test Cases

| ID | Area | Test Case | Steps | Expected Result |
|---|---|---|---|---|
| DB-001 | Init | Init project DB | Run project init command | `.b3/b3.db` created |
| DB-002 | Index | Index project | Run index command | files/symbols/edges > 0 |
| DB-003 | Reindex | Reindex idempotency | Run reindex twice | No duplicate symbols/routes/components/vectors |
| DB-004 | Delete cleanup | Delete file and reindex/watch | Remove fixture file, reindex | symbols/routes/components/vectors for deleted file removed |
| DB-005 | Corruption handling | Invalid/missing DB path | Use invalid DB parent/path | Clear error, no panic |
| DB-006 | Migration | Schema migration compatibility | Open old DB if fixture exists | Migration succeeds or clear error |
| DB-007 | WAL files | SQLite local artifacts | Inspect `.b3/` | `.db-wal`/`.db-shm` allowed locally and ignored by git |

---

## 7. Indexer Test Cases

| ID | Area | Test Case | Steps | Expected Result |
|---|---|---|---|---|
| IDX-001 | Rust | Index Rust fixture | Index Rust test project | Rust symbols/imports/relationships extracted |
| IDX-002 | JS | Index JavaScript fixture | Index JS project | JS files/symbols/imports > 0 |
| IDX-003 | TS | Index TypeScript fixture | Index TS project | TS functions/classes/interfaces/types/enums extracted |
| IDX-004 | JSX | Index JSX fixture | Index JSX project | JSX component-like symbols extracted |
| IDX-005 | TSX | Index TSX fixture | Index TSX project | TSX component-like symbols extracted |
| IDX-006 | Invalid syntax | Parse invalid JS/TS/Rust file | Add invalid file and index | No panic; parse failure recorded if supported |
| IDX-007 | Generated folders | Ignore heavy folders | Include `node_modules`, `.next`, `target` | Ignored / not indexed |
| IDX-008 | Parser worker | Parser isolation smoke | Enable subprocess parser mode if supported | Worker handles parse without crashing parent |
| IDX-009 | File watcher | Changed file reindex | Run watcher, edit file | Changed file reindexed |
| IDX-010 | Deleted file | Deleted file cleanup | Delete file under watcher | stale symbols/routes/components/vectors removed |

---

## 8. Language Capability Test Cases

| ID | Area | Test Case | Endpoint/Command | Expected Result |
|---|---|---|---|---|
| LANG-001 | Rust | Rust support level | `GET /api/languages` | Rust = Good |
| LANG-002 | JavaScript | JS support level | `GET /api/languages` | JavaScript = Basic |
| LANG-003 | TypeScript | TS support level | `GET /api/languages` | TypeScript = Basic |
| LANG-004 | JSX | JSX support level | `GET /api/languages` | JSX = Basic |
| LANG-005 | TSX | TSX support level | `GET /api/languages` | TSX = Basic |
| LANG-006 | C# | C# support level | `GET /api/languages` | C# = detect-only / basic detect-only |
| LANG-007 | LSP | LSP default | `GET /api/lsp/status` | Disabled by default |
| LANG-008 | Capabilities | Capability endpoint | `GET /api/capabilities` | Truthful capabilities, no overclaim |

---

## 9. Node.js REST Test Cases

| ID | Framework | Test Case | Fixture Pattern | Expected Result |
|---|---|---|---|---|
| REST-001 | Express | app.get route | `app.get('/users', handler)` | Route GET `/users` detected |
| REST-002 | Express | app.post route | `app.post('/users', handler)` | Route POST `/users` detected |
| REST-003 | Express | router.get route | `router.get('/:id', handler)` | Route detected with path |
| REST-004 | Express | router.route chain | `router.route('/users').get(...).post(...)` | GET/POST routes detected |
| REST-005 | NestJS | controller path | `@Controller('users')` | Controller base path detected |
| REST-006 | NestJS | method route | `@Get(':id')` | Combined path `/users/:id` detected |
| REST-007 | NestJS | POST route | `@Post()` | POST route detected |
| REST-008 | Fastify | shorthand route | `fastify.get('/users', handler)` | GET route detected |
| REST-009 | Fastify | route object | `fastify.route({ method, url, handler })` | Route detected |
| REST-010 | API | Routes endpoint | `GET /api/routes` | Returns route DTOs |
| REST-011 | Filtering | Method filter | `GET /api/routes?method=GET` | Only GET routes returned |
| REST-012 | Filtering | Framework filter | `GET /api/routes?framework=express` | Only Express routes returned |
| REST-013 | Cleanup | Route cleanup | Delete route file and reindex | Deleted routes removed |
| REST-014 | Offline | No runtime execution | Run route extraction | No npm/node/tsc/eslint execution |

---

## 10. React / TSX Component Test Cases

| ID | Area | Test Case | Fixture Pattern | Expected Result |
|---|---|---|---|---|
| REACT-001 | Detection | React package detection | package.json has `react` | React detected as technology metadata if supported |
| REACT-002 | Function component | PascalCase function returning JSX | `function ProductCard(){ return <div/> }` | Component detected |
| REACT-003 | Arrow component | Arrow function returning JSX | `const ProductCard = () => <div/>` | Component detected |
| REACT-004 | Default export | Default function component | `export default function ProductCard(){}` | Component export detected |
| REACT-005 | Named export | Named component export | `export function ProductCard(){}` | Named export detected |
| REACT-006 | Props interface | Props interface link | `interface ProductCardProps {}` | Props type linked if resolvable |
| REACT-007 | Props type | Props type alias link | `type ProductCardProps = {}` | Props type linked if resolvable |
| REACT-008 | Hooks | Built-in hooks | `useState`, `useEffect` | Hooks detected |
| REACT-009 | Custom hooks | Custom hook | `useProduct()` | Custom hook detected |
| REACT-010 | JSX usage | Component usage | `<ProductCard />` | Usage metadata/relationship detected where safe |
| REACT-011 | Non-component | Utility function | `function formatPrice(){ return '' }` | Not classified as component |
| REACT-012 | API | Components endpoint | `GET /api/components` | Returns component DTOs |
| REACT-013 | Filtering | Name filter | `GET /api/components?name=ProductCard` | Filtered results |
| REACT-014 | Cleanup | Delete component file | Delete file and reindex | Component metadata removed |
| REACT-015 | Offline | No runtime execution | Run extraction | No npm/node/react/next execution |

---

## 11. Next.js Test Cases

Use this section after Phase 9.2.3 is completed.

| ID | Area | Test Case | Fixture Path | Expected Result |
|---|---|---|---|---|
| NEXT-001 | Detection | package.json Next.js | `next` dependency | Next.js detected |
| NEXT-002 | Config | next.config detection | `next.config.js` | Config detected |
| NEXT-003 | App Router | Root page | `app/page.tsx` | Route `/` detected |
| NEXT-004 | App Router | Nested page | `app/users/page.tsx` | Route `/users` detected |
| NEXT-005 | Dynamic route | Single segment | `app/users/[id]/page.tsx` | Route `/users/:id` detected |
| NEXT-006 | Catch-all | Catch-all segment | `app/blog/[...slug]/page.tsx` | Catch-all route detected |
| NEXT-007 | Optional catch-all | Optional segment | `app/docs/[[...slug]]/page.tsx` | Optional catch-all route detected |
| NEXT-008 | Route groups | Group exclusion | `app/(marketing)/page.tsx` | Group omitted from URL path |
| NEXT-009 | Layout | Layout file | `app/layout.tsx` | Layout metadata detected |
| NEXT-010 | Loading | Loading file | `app/loading.tsx` | Loading metadata detected |
| NEXT-011 | Error | Error file | `app/error.tsx` | Error metadata detected |
| NEXT-012 | Not found | Not found file | `app/not-found.tsx` | Not found metadata detected |
| NEXT-013 | App API | Route handler | `app/api/users/route.ts` | API route `/api/users` detected |
| NEXT-014 | HTTP methods | Exported methods | `export function GET/POST` | Methods detected |
| NEXT-015 | Pages Router | Index page | `pages/index.tsx` | Route `/` detected |
| NEXT-016 | Pages Router | Nested page | `pages/users/index.tsx` | Route `/users` detected |
| NEXT-017 | Pages API | API page | `pages/api/users.ts` | API route `/api/users` detected |
| NEXT-018 | Client boundary | use client | top-level `'use client'` | Client boundary detected |
| NEXT-019 | Server boundary | use server | top-level `'use server'` | Server boundary detected |
| NEXT-020 | API | Routes endpoint | `GET /api/routes?framework=nextjs` | Next.js routes returned |
| NEXT-021 | Offline | No Next runtime | Run indexing | No `next dev`, `next build`, npm, node execution |

---

## 12. Angular Test Cases

Use this section after Angular phase is completed.

| ID | Area | Test Case | Fixture Pattern | Expected Result |
|---|---|---|---|---|
| ANG-001 | Detection | Angular package detection | `@angular/core` | Angular detected |
| ANG-002 | Component | Component decorator | `@Component({...})` | Component metadata detected |
| ANG-003 | Service | Injectable service | `@Injectable()` | Service metadata detected |
| ANG-004 | Module | NgModule | `@NgModule()` | Module metadata detected |
| ANG-005 | Routes | Routes array | `{ path: 'users', component: UsersComponent }` | Route metadata detected |
| ANG-006 | Template | templateUrl | `templateUrl` | Template link detected where safe |
| ANG-007 | DI | Constructor injection | `constructor(svc: UserService)` | Basic DI relationship detected |
| ANG-008 | Offline | No Angular CLI | Run indexing | No `ng`, npm, node execution |

---

## 13. ASP.NET Core / C# Test Cases

Use this section after C# / ASP.NET Core phase is completed.

| ID | Area | Test Case | Fixture Pattern | Expected Result |
|---|---|---|---|---|
| CS-001 | Detection | .cs file detection | `.cs` | C# detected |
| CS-002 | Controller | Controller class | `class UsersController` | Controller metadata detected |
| CS-003 | Route attribute | Class route | `[Route("api/[controller]")]` | Base route detected |
| CS-004 | HTTP method | Action route | `[HttpGet("{id}")]` | GET route detected |
| CS-005 | DI | Constructor injection | `IUserService` | Basic DI relationship detected |
| CS-006 | Service | Service class | `UserService` | Service symbol detected |
| CS-007 | LSP | LSP disabled default | config default | Missing server non-fatal |
| CS-008 | Offline | No dotnet execution unless explicitly allowed | Run indexing | No required dotnet command execution |

---

## 14. ORM / Database Intelligence Test Cases

Use this section after ORM phase is completed.

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| ORM-001 | EF Core | DbContext detection | DbContext metadata detected |
| ORM-002 | EF Core | DbSet detection | Entity set metadata detected |
| ORM-003 | Dapper | SQL call detection | Query callsite detected |
| ORM-004 | Prisma | schema.prisma detection | Models detected if implemented |
| ORM-005 | TypeORM | Entity decorator | Entity metadata detected |
| ORM-006 | Sequelize | Model init | Model metadata detected |
| ORM-007 | Impact | Service -> repository -> DB metadata | Relationship detected where safe |
| ORM-008 | Offline | No DB connection | No live DB required |

---

## 15. Realtime / Socket Test Cases

Use this section after realtime/socket phase is completed.

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| RT-001 | WebSocket | Server handler detection | WebSocket endpoint/event detected |
| RT-002 | Socket.IO | `io.on`, `socket.on` | Events detected |
| RT-003 | SignalR | Hub detection | Hub/methods detected |
| RT-004 | RSocket | Route/message mapping | RSocket metadata detected |
| RT-005 | Flow | Client/server event name match | Relationship detected where safe |
| RT-006 | Offline | No server execution | Static analysis only |

---

## 16. Messaging / Event-Driven Test Cases

Use this section after messaging phase is completed.

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| MSG-001 | AMQP | Queue publish/consume | Queue metadata detected |
| MSG-002 | RabbitMQ | Exchange/routing key | Exchange/key metadata detected |
| MSG-003 | Kafka | Producer/consumer | Topic metadata detected |
| MSG-004 | Google Pub/Sub | Topic/subscription | Pub/Sub metadata detected |
| MSG-005 | ksqlDB | .ksql file | Streams/tables detected if implemented |
| MSG-006 | Impact | Event contract impact | Producers/consumers linked where safe |
| MSG-007 | Offline | No broker required | Static analysis only |

---

## 17. Cloud / Infrastructure Test Cases

Use this section after cloud/infrastructure phase is completed.

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| INFRA-001 | Terraform | `.tf` resources | Resources detected |
| INFRA-002 | GCP | GCP resource names/types | GCP metadata detected |
| INFRA-003 | GKE | Kubernetes workloads | GKE/K8s workload metadata detected |
| INFRA-004 | Docker | Dockerfile | Docker metadata detected |
| INFRA-005 | Compose | docker-compose.yml | Services detected |
| INFRA-006 | K8s | deployment/service yaml | Workloads/services detected |
| INFRA-007 | Relationship | app service -> infra | Link detected where safe |
| INFRA-008 | Offline | No cloud auth | No gcloud/kubectl/terraform execution required |

---

## 18. Go Test Cases

Use this section after Go phase is completed.

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| GO-001 | Detection | `.go` file | Go detected |
| GO-002 | Package | package declaration | Package metadata detected |
| GO-003 | Imports | import block | Imports extracted |
| GO-004 | Function | function declaration | Function symbol extracted |
| GO-005 | Struct | struct declaration | Struct symbol extracted |
| GO-006 | Interface | interface declaration | Interface symbol extracted |
| GO-007 | Method | receiver method | Method symbol extracted |
| GO-008 | net/http | basic handler if implemented | Handler metadata detected |
| GO-009 | Offline | No `go` execution required | Static analysis only |

---

## 19. LSP Backend Test Cases

| ID | Area | Test Case | Steps | Expected Result |
|---|---|---|---|---|
| LSP-001 | Defaults | LSP disabled | Check config/status | Disabled by default |
| LSP-002 | Status | LSP status endpoint | `GET /api/lsp/status` | HTTP 200, disabled |
| LSP-003 | Servers | LSP servers endpoint | `GET /api/lsp/servers` | HTTP 200, zero or configured servers |
| LSP-004 | Missing server | Missing binary | Configure missing server | Clear non-fatal error |
| LSP-005 | Mock server | Mock LSP integration | Run tests | initialize/definition/references work against mock |
| LSP-006 | Shutdown | Process cleanup | Start/stop mock | No zombie process |
| LSP-007 | Offline | Local-only | Inspect behavior | No cloud/API/download |

---

## 20. MCP Runtime Test Cases

| ID | Area | Test Case | Steps | Expected Result |
|---|---|---|---|---|
| MCP-001 | Initialize | MCP initialize | Send initialize | OK |
| MCP-002 | Shutdown | MCP shutdown | Send shutdown | OK |
| MCP-003 | Profile optimized | tools/list default | Run default profile | 7 tools |
| MCP-004 | Profile tiny | tools/list tiny | `--profile tiny` | 5 tools |
| MCP-005 | Profile full | tools/list full | `--profile full` | 11 tools |
| MCP-006 | Profile debug | tools/list debug | `--profile debug` | 11 tools |
| MCP-007 | Profile enterprise | tools/list enterprise | `--profile enterprise` | 9 tools |
| MCP-008 | Hidden tool | Call hidden tool in tiny | Call excluded tool | Structured `tool_not_enabled` error |
| MCP-009 | Compaction | compact_command_output | Call in optimized | Non-error compacted result |
| MCP-010 | Compatibility | No new profile count drift | Run profile test suite | Counts stable unless intentionally changed |
| MCP-011 | Runtime boundary | MCP runtime thin | Inspect changes | No indexing/query/storage/UI logic added to runtime |

---

## 21. Control Server API Test Cases

Start control server:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

Test endpoints:

| ID | Endpoint | Expected Result |
|---|---|---|
| API-001 | `GET /health` | HTTP 200, healthy, offline/local metadata |
| API-002 | `GET /api/status` | HTTP 200, indexed counts |
| API-003 | `GET /api/project` | HTTP 200, project path/db/counts |
| API-004 | `GET /api/capabilities` | HTTP 200, capabilities truthful |
| API-005 | `GET /api/languages` | HTTP 200, language support levels |
| API-006 | `GET /api/lsp/status` | HTTP 200, disabled by default |
| API-007 | `GET /api/lsp/servers` | HTTP 200 |
| API-008 | `GET /api/routes` | HTTP 200, list or empty list |
| API-009 | `GET /api/components` | HTTP 200, list or empty list |
| API-010 | `POST /api/index/run` | Starts index if implemented |
| API-011 | `GET /api/index/status` | Shows idle/running/error |
| API-012 | `POST /api/index/reindex` | Reindexes if safe/implemented |

---

## 22. CLI Test Cases

| ID | Command | Expected Result |
|---|---|---|
| CLI-001 | `b3 doctor --project "." --database ".b3/b3.db" --profile optimized` | OK, local-only diagnostics |
| CLI-002 | `b3 install --agent codex --project "." --database ".b3/b3.db" --profile optimized --dry-run` | Prints config, writes nothing |
| CLI-003 | `b3 install --agent cursor --project "." --database ".b3/b3.db" --profile optimized --dry-run` | Prints config, writes nothing |
| CLI-004 | `b3 install --agent codex --apply --backup` against temp config | Writes config and backup |
| CLI-005 | Repeated install | No duplicate MCP server entry |
| CLI-006 | `b3 uninstall --agent codex --dry-run` | Prints planned removal, writes nothing |
| CLI-007 | `b3 register <temp-project>` with temp B3_HOME | Registry entry created |
| CLI-008 | `b3 list` with temp B3_HOME | Lists registered project |
| CLI-009 | `b3 status <project-id>` | Shows path/db status |
| CLI-010 | `b3 group create <name>` | Group created |
| CLI-011 | `b3 group add <group> <project>` | Membership added |
| CLI-012 | `b3 group status <group>` | Group members shown |
| CLI-013 | Invalid profile | Clear error |
| CLI-014 | Offline | No network/cloud/telemetry | Local file operations only |

---

## 23. Web UI Smoke Test Cases

Run only if frontend source/package files changed, or as manual release verification.

```powershell
cd apps/web-ui
npm run typecheck
npm run lint
npm run build
npm audit --json
npm run dev
```

Open:

```text
http://127.0.0.1:8888
```

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| UI-001 | Port | Web UI dev port | Runs on 8888 |
| UI-002 | API base | Control server URL | Uses 127.0.0.1:7777 |
| UI-003 | Unavailable API | Control server stopped | Clear unavailable message |
| UI-004 | Dashboard | Project counts | Files/symbols/edges shown |
| UI-005 | Languages | Language data if displayed | Truthful support levels |
| UI-006 | Routes | Route UI if present | Does not fake data |
| UI-007 | Components | Component UI if present | Does not fake data |
| UI-008 | Registry | Registry UI if present | Shows deferred if API unavailable |
| UI-009 | Offline | No telemetry/cloud | No external required runtime calls |

---

## 24. Phase 10 — Local Embeddings + Vector Search Test Cases

These are the main tests after Phase 10.

### 24.1 Embedding Backend Tests

| ID | Area | Test Case | Steps | Expected Result |
|---|---|---|---|---|
| EMB-001 | Defaults | Embeddings disabled or local-only default | Inspect config/status | No cloud embedding API required |
| EMB-002 | Local model | Local embedding provider config | Configure local provider | Uses local model/path only |
| EMB-003 | Missing model | Missing local model path | Run embedding/index | Clear non-fatal error or disabled status |
| EMB-004 | No network | Runtime offline | Disable network / inspect calls | No external API call |
| EMB-005 | Chunking | Code chunk generation | Index fixture | Stable chunks with file/symbol metadata |
| EMB-006 | Determinism | Same input same chunks | Run twice | Stable chunk ids/content hashes |
| EMB-007 | Incremental | Changed file re-embeds only affected chunks | Edit one file | Only changed chunks updated where supported |
| EMB-008 | Cleanup | Deleted file removes vectors | Delete file and reindex | Vectors removed |
| EMB-009 | Large file | Large generated file ignored/handled | Add large file | No runaway memory/time |
| EMB-010 | Privacy | Local-only data | Inspect logs/config | No code uploaded anywhere |

### 24.2 Vector Storage Tests

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| VEC-001 | Storage | Vector table/index created | Local DB/vector store initialized |
| VEC-002 | Insert | Insert vector records | Records stored with file/symbol/chunk refs |
| VEC-003 | Query | Nearest neighbor query | Relevant chunks returned |
| VEC-004 | Delete | Delete stale vectors | No stale results for deleted file |
| VEC-005 | Dimension mismatch | Wrong vector dimension | Clear error |
| VEC-006 | Empty index | Query empty vector index | Empty result, no panic |
| VEC-007 | Corruption | Invalid vector metadata | Clear error |
| VEC-008 | Local-only | No hosted vector DB required | Works without Pinecone/Qdrant Cloud/Weaviate Cloud |

### 24.3 Hybrid Search Tests

| ID | Area | Test Case | Expected Result |
|---|---|---|---|
| HYB-001 | FTS only | Keyword query | FTS results returned |
| HYB-002 | Vector only | Semantic query | Semantically relevant chunks returned |
| HYB-003 | Hybrid | FTS + vector + graph | Blended ranked results returned |
| HYB-004 | Symbol boost | Query symbol name | Symbol/file context boosted |
| HYB-005 | Graph boost | Related symbol query | Graph-near results boosted |
| HYB-006 | Explainability | Search explanation | Shows FTS/vector/graph contribution if supported |
| HYB-007 | Context pack | Hybrid context pack | Useful concise context returned |
| HYB-008 | No overrun | Token budget | Context pack respects configured budget |

### 24.4 Semantic Search Quality Tests

| ID | Area | Query | Expected Result |
|---|---|---|---|
| SEM-001 | REST | “user route handler” | Express/Nest/Fastify user route files returned |
| SEM-002 | React | “product card component props” | React component + props type returned |
| SEM-003 | Next.js | “dynamic user page route” | Next dynamic route file returned if implemented |
| SEM-004 | Rust | “function that builds context pack” | Relevant Rust symbols returned |
| SEM-005 | Cross wording | “where is the login screen UI” | Related component/page returned if indexed |
| SEM-006 | Negative | nonsense query | Low-confidence/empty result, no panic |

---

## 25. Benchmark Test Plan

### 25.1 Baseline Command

```powershell
cargo run -p b3-bench -- baseline
```

Expected output:

- Human-readable table
- JSON output at `target/benchmarks/baseline.json`
- No upload
- No telemetry

### 25.2 Benchmark Categories

| ID | Benchmark | Purpose | Expected Constraint |
|---|---|---|---|
| BENCH-001 | indexing_tiny_rust_repo | Rust indexing speed | No major regression |
| BENCH-002 | indexing_js_ts_repo | JS/TS indexing speed | Stable local parse time |
| BENCH-003 | indexing_react_repo | React component extraction overhead | Acceptable overhead |
| BENCH-004 | indexing_nextjs_repo | Next.js route extraction overhead | Acceptable overhead after implemented |
| BENCH-005 | sqlite_summary_latency | Storage summary query | Low latency |
| BENCH-006 | mcp_tools_list_latency_optimized | Default MCP manifest latency | 7 tools, stable latency |
| BENCH-007 | mcp_tools_list_latency_full | Full profile manifest latency | 11 tools |
| BENCH-008 | command_compaction_latency | Output compaction speed | Stable and local |
| BENCH-009 | context_pack_latency | Context pack generation | Stable latency |
| BENCH-010 | route_query_latency | `/api/routes` query | Stable for bounded results |
| BENCH-011 | component_query_latency | `/api/components` query | Stable for bounded results |
| BENCH-012 | embedding_chunk_latency | Phase 10 chunk generation | Stable and local |
| BENCH-013 | embedding_compute_latency | Phase 10 local embedding compute | Baseline only; no cloud |
| BENCH-014 | vector_insert_latency | Phase 10 vector insert | Stable local storage |
| BENCH-015 | vector_query_latency | Phase 10 vector search | Stable local nearest-neighbor query |
| BENCH-016 | hybrid_search_latency | Phase 10 hybrid search | Stable under token budget |

### 25.3 Benchmark Metadata Requirements

Benchmark JSON should include where applicable:

```json
{
  "benchmark": "name",
  "duration_ms": 0.0,
  "input_size": 0,
  "result_count": 0,
  "metadata": {
    "profile": "optimized",
    "tool_count": 7,
    "language": "typescript",
    "framework": "react",
    "embedding_provider": "local",
    "vector_index": "local"
  }
}
```

Do not remove existing JSON fields unless intentionally versioned.

### 25.4 Regression Gates

Suggested initial advisory gates:

| Area | Gate |
|---|---|
| Workspace tests | Must pass |
| Benchmark command | Must pass |
| MCP optimized count | Must remain 7 unless intentionally changed |
| Full MCP count | Must remain 11 unless new tool intentionally added |
| Local embedding | Must not call cloud |
| Vector search | Must not require hosted DB |
| Indexing | No obvious 2x regression without explanation |
| Query latency | No obvious 2x regression without explanation |
| Benchmark JSON | Backward-compatible shape |

---

## 26. Manual Release Checklist After Phase 10

```text
[ ] Conflict marker scan clean
[ ] cargo fmt passed
[ ] cargo fmt --check passed
[ ] cargo check --workspace passed
[ ] cargo test --workspace passed
[ ] cargo run -p b3-bench -- baseline passed
[ ] Control server starts on 7777
[ ] Web UI opens on 8888 if manually checked
[ ] /health returns healthy
[ ] /api/status returns project counts
[ ] /api/languages truthful
[ ] /api/lsp/status disabled by default
[ ] /api/routes works if route data exists
[ ] /api/components works if component data exists
[ ] MCP initialize/shutdown works
[ ] MCP optimized profile has expected count
[ ] CLI doctor works
[ ] Registry commands work with temp B3_HOME
[ ] Local embeddings do not call cloud
[ ] Vector search does not require hosted DB
[ ] Hybrid search returns useful context
[ ] No npm/node/next/tsc/eslint execution required for static analysis
[ ] No telemetry added
[ ] No paid dependency added
[ ] Docs updated
[ ] PLAN.md current status updated
[ ] Offline/free compliance documented
```

---

## 27. Final Phase 10 Acceptance Report Template

Use this when reporting after Phase 10 testing.

```text
A. Files changed
B. Phase 10 features verified
C. Local embedding behavior
D. Vector storage/search behavior
E. Hybrid search behavior
F. Indexing regression result
G. Language/framework regression result
H. MCP/profile compatibility result
I. Control API result
J. CLI result
K. Web UI result or skipped reason
L. Benchmark result
M. Benchmark JSON compatibility result
N. Offline/free compliance result
O. Remaining risks
P. Deferred work
Q. READY / NOT READY for next phase
```

---

## 28. Notes

- This test plan intentionally avoids requiring cloud services, hosted vector databases, telemetry, SaaS authentication, paid plugins, or runtime internet access.
- Some test sections are future-facing and should only be marked pass after the corresponding phase is actually implemented.
- Do not fake pass for deferred capabilities.
- “Skipped” must include a reason.
- “Completed” is wrong if any required verification was silently skipped.
