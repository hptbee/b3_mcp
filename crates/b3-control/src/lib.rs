//! Control server and UI boundary.
//!
//! This crate will host the optional localhost control server and provide APIs
//! for the Next.js/React Flow UI. It must stay separate from the MCP hot path;
//! UI features observe and control background services through contracts and
//! events instead of calling parser, storage, or embedding internals directly.

use b3_core::PRODUCT_NAME;

pub use b3_core::{ConfigProvider, DomainEvent, EventBus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPlaneInfo {
    pub product: &'static str,
    pub ui_path: &'static str,
    pub websocket_path: &'static str,
    pub enabled_by_default: bool,
}

pub fn control_plane_info() -> ControlPlaneInfo {
    ControlPlaneInfo {
        product: PRODUCT_NAME,
        ui_path: "/",
        websocket_path: "/ws",
        enabled_by_default: false,
    }
}
