//! Panel-facing API for the AgentX GPUI shell.
//!
//! This module is the target shape for migrating legacy `src/panels`: panels own
//! UI state and intents, while concrete adapters are supplied by the shell.

mod tool_call_detail;
mod types;

pub use tool_call_detail::{ToolCallDetailPanel, open_tool_call_detail_window};
pub use types::{PanelCommand, PanelDescriptor, PanelKind, PanelPlacement};
