//! Reusable GPUI presentation pieces for AgentX panels.
//!
//! Components in this module are intentionally small and data-driven. They do
//! not subscribe to services, read stores, or mutate application state; panels
//! and view-models pass in domain data plus callbacks.

mod diff;
mod message;
mod permission;
mod plan;
mod status_badge;
mod tool_call;

pub use diff::render_diff;
pub use message::{
    render_agent_message, render_agent_thought, render_session_end, render_timeline_error,
    render_timeline_note, render_user_message,
};
pub use permission::{
    permission_is_allow, permission_option_button, permission_option_kind_to_icon,
    render_permission_request_card,
};
pub use plan::render_plan;
pub use status_badge::{StatusTone, status_badge, status_dot};
pub use tool_call::{
    render_tool_call_body, render_tool_call_content, render_tool_call_item, tool_status_label,
    tool_status_tone,
};
