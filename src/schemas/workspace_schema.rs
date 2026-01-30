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

#[cfg(test)]
mod tests {
    use super::*;

    // ============== Workspace tests ==============

    #[test]
    fn test_workspace_new() {
        let path = PathBuf::from("/home/user/my-project");
        let workspace = Workspace::new(path.clone());

        assert_eq!(workspace.name, "my-project");
        assert_eq!(workspace.path, path);
        assert!(!workspace.id.is_empty());
        assert!(workspace.tasks.is_empty());
    }

    #[test]
    fn test_workspace_new_unnamed() {
        // Root path should use "Unnamed Project"
        let path = PathBuf::from("/");
        let workspace = Workspace::new(path);

        assert_eq!(workspace.name, "Unnamed Project");
    }

    #[test]
    fn test_workspace_touch() {
        let path = PathBuf::from("/home/user/project");
        let mut workspace = Workspace::new(path);

        let original_last_accessed = workspace.last_accessed;
        std::thread::sleep(std::time::Duration::from_millis(50));
        workspace.touch();

        assert!(workspace.last_accessed > original_last_accessed);
    }

    // ============== WorkspaceTask tests ==============

    #[test]
    fn test_workspace_task_new() {
        let task = WorkspaceTask::new(
            "workspace-1".to_string(),
            "Fix bug".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        );

        assert_eq!(task.workspace_id, "workspace-1");
        assert_eq!(task.name, "Fix bug");
        assert_eq!(task.agent_name, "claude");
        assert_eq!(task.mode, "Auto");
        assert!(task.session_id.is_none());
        assert!(matches!(task.status, SessionStatus::Pending));
    }

    #[test]
    fn test_workspace_task_set_session() {
        let mut task = WorkspaceTask::new(
            "workspace-1".to_string(),
            "Task".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        );

        task.set_session("session-123".to_string());

        assert_eq!(task.session_id, Some("session-123".to_string()));
        assert!(matches!(task.status, SessionStatus::InProgress));
    }

    // ============== WorkspaceConfig tests ==============

    #[test]
    fn test_workspace_config_add_remove() {
        let mut config = WorkspaceConfig::default();

        let workspace = Workspace::new(PathBuf::from("/test/project"));
        let workspace_id = workspace.id.clone();

        config.add_workspace(workspace);
        assert_eq!(config.workspaces.len(), 1);

        config.remove_workspace(&workspace_id);
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn test_workspace_config_remove_cascades_tasks() {
        let mut config = WorkspaceConfig::default();

        let workspace = Workspace::new(PathBuf::from("/test/project"));
        let workspace_id = workspace.id.clone();
        config.add_workspace(workspace);

        let task = WorkspaceTask::new(
            workspace_id.clone(),
            "Task 1".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        );
        config.add_task(task);

        assert_eq!(config.tasks.len(), 1);

        config.remove_workspace(&workspace_id);

        assert!(config.workspaces.is_empty());
        assert!(config.tasks.is_empty()); // cascaded delete
    }

    #[test]
    fn test_workspace_config_tasks_for_workspace() {
        let mut config = WorkspaceConfig::default();

        let workspace1 = Workspace::new(PathBuf::from("/project1"));
        let workspace2 = Workspace::new(PathBuf::from("/project2"));
        let ws1_id = workspace1.id.clone();
        let ws2_id = workspace2.id.clone();

        config.add_workspace(workspace1);
        config.add_workspace(workspace2);

        config.add_task(WorkspaceTask::new(
            ws1_id.clone(),
            "Task A".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        ));
        config.add_task(WorkspaceTask::new(
            ws1_id.clone(),
            "Task B".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        ));
        config.add_task(WorkspaceTask::new(
            ws2_id.clone(),
            "Task C".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        ));

        let ws1_tasks = config.tasks_for_workspace(&ws1_id);
        let ws2_tasks = config.tasks_for_workspace(&ws2_id);

        assert_eq!(ws1_tasks.len(), 2);
        assert_eq!(ws2_tasks.len(), 1);
    }

    #[test]
    fn test_workspace_config_find_task_by_session() {
        let mut config = WorkspaceConfig::default();

        let workspace = Workspace::new(PathBuf::from("/project"));
        let ws_id = workspace.id.clone();
        config.add_workspace(workspace);

        let mut task = WorkspaceTask::new(
            ws_id,
            "Task".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        );
        task.set_session("session-xyz".to_string());
        config.add_task(task);

        let found = config.find_task_by_session("session-xyz");
        assert!(found.is_some());
        assert_eq!(found.unwrap().session_id, Some("session-xyz".to_string()));

        let not_found = config.find_task_by_session("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_workspace_config_serialization_roundtrip() {
        let mut config = WorkspaceConfig::default();

        let workspace = Workspace::new(PathBuf::from("/test/project"));
        let ws_id = workspace.id.clone();
        config.add_workspace(workspace);
        config.active_workspace_id = Some(ws_id.clone());

        let task = WorkspaceTask::new(
            ws_id,
            "Test task".to_string(),
            "claude".to_string(),
            "Auto".to_string(),
        );
        config.add_task(task);

        let json = serde_json::to_string(&config).unwrap();
        let restored: WorkspaceConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.workspaces.len(), 1);
        assert_eq!(restored.tasks.len(), 1);
        assert!(restored.active_workspace_id.is_some());
    }
}
