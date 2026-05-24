# Development

## Required Local Tools

- Rust stable toolchain with `cargo`
- Git
- PowerShell on Windows

Future phases may also require:
- Node.js for the Next.js UI
- local Qdrant for vector search
- local embedding providers such as Ollama, GGUF models, sentence-transformers, Candle, or fastembed

All required dependencies should be available locally for normal development.
Do not make verification depend on live network access, hosted databases,
cloud authentication, or external telemetry endpoints.

## Rust Installation

Install Rust from:

```text
https://rustup.rs/
```

After installation, confirm:

```powershell
cargo --version
rustc --version
```

## Verification Commands

Run from the repository root:

```powershell
cargo fmt
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

For a stricter local pre-commit pass:

```powershell
.\scripts\verify.ps1
```

## Manual Project Indexing

Initialize and index a local project database:

```powershell
cargo run -p b3-control --bin b3-control-server -- init --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- index --project "." --database ".b3/b3.db"
cargo run -p b3-control --bin b3-control-server -- reindex --project "." --database ".b3/b3.db"
```

Then run the control server:

```powershell
cargo run -p b3-control --bin b3-control-server -- serve --project "." --database ".b3/b3.db" --port 7777
```

The control server uses `http://127.0.0.1:7777` by default. The Web UI dev
server uses `http://127.0.0.1:8888` by default and calls the local control
server through `NEXT_PUBLIC_B3_API_BASE_URL`.

`reindex` is currently a safe incremental reindex. It skips unchanged files,
cleans deleted files for the current branch, and does not delete unrelated
project data. The workflow is single-project only; multi-repo registry support
is deferred to Phase 8.8.

The CI skeleton mirrors these commands with:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace`

## Benchmark Commands

Run the local benchmark baseline from the repository root:

```powershell
cargo run -p b3-bench -- baseline
```

The runner uses deterministic local fixtures from `benchmarks/fixtures` and
writes JSON output to:

```text
target/benchmarks/baseline.json
```

The baseline includes local timings for MCP tool listing, simple MCP tool calls,
control-server handlers, query operations, indexing, changed-file reindexing,
watcher debounce behavior, SQLite graph summary queries, and parser worker
request handling. The `memory_kb` field may be `null` on platforms where a
rough process-memory snapshot is not available yet.

The watcher debounce benchmark measures event coalescing overhead. It does not
include an intentional sleep for the configured debounce wait, because that
would benchmark the configured delay rather than processing cost.

Regression thresholds live in:

```text
benchmarks/benchmark-thresholds.json
```

Thresholds are advisory by default. They do not fail CI unless
`fail_on_regression` is explicitly enabled.

## Offline-First Expectations

The default project must work without internet access after dependencies are available locally.

Core features must not require:
- cloud APIs
- hosted vector databases
- SaaS authentication
- remote telemetry
- OpenAI, Anthropic, or Gemini APIs

External integrations must remain:
- optional
- plugin-based
- disabled by default

## Boundary Expectations

- MCP runtime remains protocol-only.
- Indexing runs outside the MCP hot path.
- Storage exposes repository contracts instead of leaking SQLite details across crates.
- Thread-safe SQLite index-store adapters belong in `b3-storage`, not HTTP or MCP adapters.
- Command output compaction only transforms provided stdout/stderr; it must not execute commands.
- Embeddings run in background workers in later phases.
- UI/control plane stays separate from MCP runtime.

## Command Output Compaction

Phase 8.5 adds deterministic local compaction for noisy command output. Use the
MCP tool `compact_command_output` when a client already has stdout/stderr and
wants a smaller summary.

The compactor supports conservative string-based detection for `git`, `cargo`,
`dotnet`, `npm`, `pnpm`, `yarn`, `ng`, `tsc`, `eslint`, `docker`,
`docker compose`, `rg`, `grep`, `cat`, `tree`, and unknown commands. It does not
run commands, open shells, call an LLM, upload output, or emit telemetry.

## Parser Worker Development

Phase 8.1 adds a local parser isolation worker binary:

```powershell
cargo build -p b3-indexer --bin b3-parser-worker
```

The worker uses stdin/stdout JSON lines, parses locally, never opens network
sockets, and never emits telemetry. `ParserIsolation::InProcess` remains the
default compatibility mode; `ParserIsolation::SubprocessWorker` is available for
crash/timeout isolation. Defaults are:

- `parser_timeout_ms = 10000`
- `parser_max_retries = 1`
- `parser_worker_path = None` (resolved next to the current executable when subprocess mode is used)

Parse failures are stored locally in SQLite table `parse_failures` and surfaced
through `GET /api/diagnostics`.
