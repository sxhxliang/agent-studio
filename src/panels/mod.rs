// Panel-related modules

pub mod code_editor;
pub mod conversation;
pub mod dock_panel;
mod session_manager;
mod settings_panel;
mod task_panel;
mod terminal_panel;
mod tool_call_detail_panel;
mod welcome_panel;

// Re-export panel types
pub use code_editor::CodeEditorPanel;
pub use conversation::ConversationPanel;
pub use dock_panel::{DockPanel, DockPanelContainer, DockPanelState};
pub use session_manager::SessionManagerPanel;
pub use settings_panel::{AppSettings, SettingsPanel};
pub use task_panel::TaskPanel;
pub use terminal_panel::TerminalPanel;
pub use tool_call_detail_panel::ToolCallDetailPanel;
pub use welcome_panel::WelcomePanel;
