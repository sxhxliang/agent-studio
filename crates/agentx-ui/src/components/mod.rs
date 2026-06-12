//! Reusable GPUI presentation pieces for AgentX panels.
//!
//! Components in this module are intentionally small and data-driven. They do
//! not subscribe to services, read stores, or mutate application state; panels
//! and view-models pass in domain data plus callbacks.

mod chat_input_box;
mod diff;
mod file_picker;
mod input_suggestion;
mod message;
mod permission;
mod plan;
mod select_items;
mod status_badge;
mod tool_call;

pub use chat_input_box::{ChatInputBox, CodeSelection, ImageAttachment};
pub use diff::render_diff;
pub use file_picker::{FileItem, render_file_item};
pub use input_suggestion::{
    InputSuggestion, InputSuggestionEvent, InputSuggestionItem, InputSuggestionState,
};
pub use message::{
    render_agent_message, render_agent_thought, render_session_end, render_timeline_error,
    render_timeline_note, render_user_message,
};
pub use permission::{
    permission_is_allow, permission_option_button, permission_option_kind_to_icon,
    render_permission_request_card,
};
pub use plan::render_plan;
pub use select_items::{AgentItem, ModeSelectItem, ModelSelectItem};
pub use status_badge::{StatusTone, session_status_tone, status_badge, status_dot};
pub use tool_call::{
    render_tool_call_body, render_tool_call_content, render_tool_call_item, tool_status_label,
    tool_status_tone,
};
