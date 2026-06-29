//! Panel-facing API for the AgentX GPUI shell.
//!
//! This module is the target shape for migrating legacy `src/panels`: panels own
//! UI state and intents, while concrete adapters are supplied by the shell.

mod session_manager;
mod settings;
mod task_panel;
mod tool_call_detail;
mod types;
mod welcome_panel;

pub use session_manager::SessionManagerPanel;
pub use settings::{SettingsPanel, open_settings_window};
pub use task_panel::TaskPanel;
pub use tool_call_detail::{ToolCallDetailPanel, open_tool_call_detail_window};
pub use types::{PanelCommand, PanelDescriptor, PanelKind, PanelPlacement};
pub(crate) use welcome_panel::WelcomeLaunch;
pub use welcome_panel::WelcomePanel;
