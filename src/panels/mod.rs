// Panel-related modules

pub mod code_editor;
pub mod conversation;
pub mod dock_panel;
mod session_manager;
mod settings_panel;
mod task_panel;
mod welcome_panel;


// Re-export panel types
pub use code_editor::CodeEditorPanel;
pub use conversation::ConversationPanel;
pub use dock_panel::{DockPanel, DockPanelContainer, DockPanelState};
pub use session_manager::SessionManagerPanel;
pub use settings_panel::SettingsPanel;
pub use task_panel::TaskPanel;
pub use welcome_panel::WelcomePanel;
