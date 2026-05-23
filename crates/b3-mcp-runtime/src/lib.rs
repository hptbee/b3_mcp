//! Thin MCP runtime boundary.
//!
//! This crate owns protocol-facing concerns only. Heavy indexing, embeddings,
//! graph traversal, blocking IO, and filesystem scans belong in core services
//! behind this boundary.

use b3_core::PRODUCT_NAME;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInfo {
    pub name: &'static str,
    pub protocol: &'static str,
    pub boundary: RuntimeBoundary,
}

pub fn runtime_info() -> RuntimeInfo {
    RuntimeInfo {
        name: PRODUCT_NAME,
        protocol: "mcp",
        boundary: RuntimeBoundary::ProtocolOnly,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBoundary {
    ProtocolOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponsibility {
    StdioTransport,
    JsonRpc,
    ToolRouting,
    Streaming,
    Cancellation,
    SessionLifecycle,
}
