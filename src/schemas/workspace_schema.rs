use gpui::SharedString;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::core::services::SessionStatus;

/// Workspace represents a local project folder
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique identifier for the workspace
    pub id: String,
    /// Display name for the workspace
    pub name: String,
    /// Absolute path to the project folder
    pub path: PathBuf,
    /// When the workspace was added
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last accessed time
    #[serde(with = "chrono::serde::ts_seconds")]
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    /// Tasks associated with this workspace
    #[serde(skip)]
    pub tasks: Vec<WorkspaceTask>,
}

impl Workspace {
    /// Create a new workspace from a folder path
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unnamed Project")
            .to_string();

        let now = chrono::Utc::now();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            path,
            created_at: now,
            last_accessed: now,
            tasks: Vec::new(),
        }
    }

    /// Update last accessed time
    pub fn touch(&mut self) {
        self.last_accessed = chrono::Utc::now();
    }
}

/// Task within a workspace
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceTask {
    /// Unique identifier for the task
    pub id: String,
    /// Workspace this task belongs to
    pub workspace_id: String,
    /// Task name/description
    pub name: String,
    /// Agent used for this task
    pub agent_name: String,
    /// Task mode (Auto, Ask, Plan, Code, Explain)
    pub mode: String,
    /// Session ID if a session has been created
    pub session_id: Option<String>,
    /// Task status
    pub status: SessionStatus,
    /// When the task was created
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last message preview
    #[serde(skip)]
    pub last_message: Option<SharedString>,
}

impl WorkspaceTask {
    /// Create a new task for a workspace
    pub fn new(workspace_id: String, name: String, agent_name: String, mode: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id,
            name,
            agent_name,
            mode,
            session_id: None,
            status: SessionStatus::Pending,
            created_at: chrono::Utc::now(),
            last_message: None,
        }
    }

    /// Associate a session with this task
    pub fn set_session(&mut self, session_id: String) {
        self.session_id = Some(session_id);
        self.status = SessionStatus::InProgress;
    }

    /// Update the last message preview
    pub fn update_last_message(&mut self, text: impl Into<SharedString>) {
        self.last_message = Some(text.into());
    }
}

/// Persistent workspace configuration
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct WorkspaceConfig {
    /// All workspaces
    pub workspaces: Vec<Workspace>,
    /// All tasks across workspaces
    pub tasks: Vec<WorkspaceTask>,
    /// Currently active workspace ID
    pub active_workspace_id: Option<String>,
}

impl WorkspaceConfig {
    /// Add a new workspace
    pub fn add_workspace(&mut self, workspace: Workspace) {
        self.workspaces.push(workspace);
    }

    /// Remove a workspace by ID
    pub fn remove_workspace(&mut self, workspace_id: &str) {
        self.workspaces.retain(|w| w.id != workspace_id);
        // Also remove all tasks for this workspace
        self.tasks.retain(|t| t.workspace_id != workspace_id);
    }

    /// Add a task to a workspace
    pub fn add_task(&mut self, task: WorkspaceTask) {
        self.tasks.push(task);
    }

    /// Remove a task by ID
    pub fn remove_task(&mut self, task_id: &str) -> Option<WorkspaceTask> {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == task_id) {
            Some(self.tasks.remove(pos))
        } else {
            None
        }
    }

    /// Get tasks for a specific workspace
    pub fn tasks_for_workspace(&self, workspace_id: &str) -> Vec<&WorkspaceTask> {
        self.tasks
            .iter()
            .filter(|t| t.workspace_id == workspace_id)
            .collect()
    }

    /// Get mutable tasks for a specific workspace
    pub fn tasks_for_workspace_mut(&mut self, workspace_id: &str) -> Vec<&mut WorkspaceTask> {
        self.tasks
            .iter_mut()
            .filter(|t| t.workspace_id == workspace_id)
            .collect()
    }

    /// Find a task by session ID
    pub fn find_task_by_session(&mut self, session_id: &str) -> Option<&mut WorkspaceTask> {
        self.tasks
            .iter_mut()
            .find(|t| t.session_id.as_ref() == Some(&session_id.to_string()))
    }

    /// Get workspace by ID
    pub fn get_workspace(&self, workspace_id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == workspace_id)
    }

    /// Get mutable workspace by ID
    pub fn get_workspace_mut(&mut self, workspace_id: &str) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.id == workspace_id)
    }
}
