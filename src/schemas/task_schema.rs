use gpui::SharedString;
use serde::Deserialize;

use crate::core::services::SessionStatus;

#[derive(Clone, Default, Deserialize)]
pub struct AgentTask {
    pub name: String,
    pub task_type: String,
    pub add_new_code_lines: i16,
    pub delete_code_lines: i16,
    pub status: SessionStatus,

    /// Optional session ID for ACP-enabled tasks
    #[serde(skip)]
    pub session_id: Option<String>,

    /// Message preview/subtitle for the task
    #[serde(skip)]
    pub subtitle: Option<SharedString>,

    #[serde(skip)]
    pub change_timestamp: i16,
    #[serde(skip)]
    pub change_timestamp_str: SharedString,
    #[serde(skip)]
    pub add_new_code_lines_str: SharedString,
    #[serde(skip)]
    pub delete_code_lines_str: SharedString,
}

impl AgentTask {
    pub fn prepare(mut self) -> Self {
        self.add_new_code_lines_str = format!("+{}", self.add_new_code_lines).into();
        self.delete_code_lines_str = format!("-{}", self.delete_code_lines).into();
        self
    }

    /// Create a new task for a session
    pub fn new_for_session(name: String, session_id: String) -> Self {
        Self {
            name,
            task_type: "Default".to_string(),
            add_new_code_lines: 0,
            delete_code_lines: 0,
            status: SessionStatus::InProgress,
            session_id: Some(session_id),
            subtitle: None,
            change_timestamp: 0,
            change_timestamp_str: "".into(),
            add_new_code_lines_str: "+0".into(),
            delete_code_lines_str: "-0".into(),
        }
    }

    /// Update the subtitle with a message preview
    pub fn update_subtitle(&mut self, text: impl Into<SharedString>) {
        self.subtitle = Some(text.into());
    }
}
