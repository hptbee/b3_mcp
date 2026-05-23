$ErrorActionPreference = "Stop"

cargo fmt --check
cargo check --workspace
cargo test --workspace
