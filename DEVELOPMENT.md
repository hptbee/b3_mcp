# Development

## Required Local Tools

- Rust stable toolchain with `cargo`
- Git
- PowerShell on Windows

Future phases may also require:
- Node.js for the Next.js UI
- local Qdrant for vector search
- local embedding providers such as Ollama, GGUF models, sentence-transformers, Candle, or fastembed

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
cargo check --workspace
cargo test --workspace
```

For a stricter local pre-commit pass:

```powershell
.\scripts\verify.ps1
```

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
- Embeddings run in background workers in later phases.
- UI/control plane stays separate from MCP runtime.
