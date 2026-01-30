use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::event_bus::{WorkspaceUpdateBusContainer, WorkspaceUpdateEvent};
use crate::core::services::SessionStatus;
use crate::schemas::workspace_schema::{Workspace, WorkspaceConfig, WorkspaceTask};

/// Service for managing workspaces and tasks
///
/// This service provides the business logic for:
/// - Adding/removing workspaces (project folders)
/// - Creating tasks within workspaces
/// - Managing task-session associations
/// - Persisting workspace configuration
/// - Publishing workspace update events
#[derive(Clone)]
pub struct WorkspaceService {
    config: Arc<RwLock<WorkspaceConfig>>,
    config_path: PathBuf,
    workspace_bus: Option<WorkspaceUpdateBusContainer>,
}

impl WorkspaceService {
    /// Create a new WorkspaceService
    pub fn new(config_path: PathBuf) -> Self {
        let config = Self::load_config(&config_path).unwrap_or_default();

        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            workspace_bus: None,
        }
    }

    /// Set the workspace event bus (called after AppState initialization)
    pub fn set_workspace_bus(&mut self, bus: WorkspaceUpdateBusContainer) {
        self.workspace_bus = Some(bus);
    }

    /// Publish a workspace update event if bus is available
    fn publish_event(&self, event: WorkspaceUpdateEvent) {
        if let Some(bus) = &self.workspace_bus {
            log::debug!("[WorkspaceService] Publishing event: {:?}", &event);
            bus.publish(event);
        }
    }

    /// Load workspace configuration from disk
    fn load_config(path: &PathBuf) -> Result<WorkspaceConfig> {
        if !path.exists() {
            return Ok(WorkspaceConfig::default());
        }

        let content = std::fs::read_to_string(path).context("Failed to read workspace config")?;

        let config: WorkspaceConfig =
            serde_json::from_str(&content).context("Failed to parse workspace config")?;

        Ok(config)
    }

    /// Save workspace configuration to disk
    async fn save_config(&self) -> Result<()> {
        let config = self.config.read().await;
        let content = serde_json::to_string_pretty(&*config)
            .context("Failed to serialize workspace config")?;

        std::fs::write(&self.config_path, content).context("Failed to write workspace config")?;

        Ok(())
    }

    /// Add a new workspace from a folder path
    pub async fn add_workspace(&self, path: PathBuf) -> Result<Workspace> {
        // Validate that the path exists and is a directory
        if !path.exists() {
            anyhow::bail!("Path does not exist: {:?}", path);
        }
        if !path.is_dir() {
            anyhow::bail!("Path is not a directory: {:?}", path);
        }

        // Check if workspace with this path already exists
        {
            let config = self.config.read().await;
            if config.workspaces.iter().any(|w| w.path == path) {
                anyhow::bail!("Workspace already exists for path: {:?}", path);
            }
        }

        let workspace = Workspace::new(path);
        let workspace_clone = workspace.clone();

        {
            let mut config = self.config.write().await;
            config.add_workspace(workspace);
            // Set as active workspace if it's the first one
            if config.active_workspace_id.is_none() {
                config.active_workspace_id = Some(workspace_clone.id.clone());
            }
        }

        self.save_config().await?;

        // Publish WorkspaceAdded event
        self.publish_event(WorkspaceUpdateEvent::WorkspaceAdded {
            workspace_id: workspace_clone.id.clone(),
        });

        log::info!(
            "Added workspace: {} at {:?}",
            workspace_clone.name,
            workspace_clone.path
        );
        Ok(workspace_clone)
    }

    /// Remove a workspace by ID
    pub async fn remove_workspace(&self, workspace_id: &str) -> Result<()> {
        {
            let mut config = self.config.write().await;
            config.remove_workspace(workspace_id);

            // Clear active workspace if it was removed
            if config.active_workspace_id.as_ref() == Some(&workspace_id.to_string()) {
                config.active_workspace_id = config.workspaces.first().map(|w| w.id.clone());
            }
        }

        self.save_config().await?;

        log::info!("Removed workspace: {}", workspace_id);
        Ok(())
    }

    /// List all workspaces
    pub async fn list_workspaces(&self) -> Vec<Workspace> {
        let config = self.config.read().await;
        config.workspaces.clone()
    }

    /// Get the entire workspace configuration
    pub async fn get_config(&self) -> WorkspaceConfig {
        let config = self.config.read().await;
        config.clone()
    }

    /// Get the active workspace
    pub async fn get_active_workspace(&self) -> Option<Workspace> {
        let config = self.config.read().await;
        let workspace_id = config.active_workspace_id.as_ref()?;
        config.get_workspace(workspace_id).cloned()
    }

    /// Get a specific workspace by ID
    pub async fn get_workspace(&self, workspace_id: &str) -> Option<Workspace> {
        let config = self.config.read().await;
        config.get_workspace(workspace_id).cloned()
    }

    /// Set the active workspace
    pub async fn set_active_workspace(&self, workspace_id: &str) -> Result<()> {
        {
            let mut config = self.config.write().await;

            // Verify workspace exists
            if config.get_workspace(workspace_id).is_none() {
                anyhow::bail!("Workspace not found: {}", workspace_id);
            }

            config.active_workspace_id = Some(workspace_id.to_string());

            // Update last accessed time
            if let Some(workspace) = config.get_workspace_mut(workspace_id) {
                workspace.touch();
            }
        }

        self.save_config().await?;

        log::info!("Set active workspace: {}", workspace_id);
        Ok(())
    }

    /// Create a new task in a workspace
    pub async fn create_task(
        &self,
        workspace_id: &str,
        name: String,
        agent_name: String,
        mode: String,
    ) -> Result<WorkspaceTask> {
        let task = WorkspaceTask::new(workspace_id.to_string(), name, agent_name, mode);
        let task_clone = task.clone();

        {
            let mut config = self.config.write().await;

            // Verify workspace exists
            if config.get_workspace(workspace_id).is_none() {
                anyhow::bail!("Workspace not found: {}", workspace_id);
            }

            config.add_task(task);
        }

        self.save_config().await?;

        // Publish TaskCreated event
        self.publish_event(WorkspaceUpdateEvent::TaskCreated {
            workspace_id: workspace_id.to_string(),
            task_id: task_clone.id.clone(),
        });

        log::info!(
            "Created task '{}' in workspace {}",
            task_clone.name,
            workspace_id
        );
        Ok(task_clone)
    }

    /// Associate a session with a task
    pub async fn set_task_session(&self, task_id: &str, session_id: String) -> Result<()> {
        {
            let mut config = self.config.write().await;

            let task = config
                .tasks
                .iter_mut()
                .find(|t| t.id == task_id)
                .context("Task not found")?;

            task.set_session(session_id);
        }

        self.save_config().await?;

        Ok(())
    }

    /// Get all tasks for a workspace
    pub async fn get_workspace_tasks(&self, workspace_id: &str) -> Vec<WorkspaceTask> {
        let config = self.config.read().await;
        config
            .tasks_for_workspace(workspace_id)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Update task status
    pub async fn update_task_status(&self, task_id: &str, status: SessionStatus) -> Result<()> {
        {
            let mut config = self.config.write().await;

            let task = config
                .tasks
                .iter_mut()
                .find(|t| t.id == task_id)
                .context("Task not found")?;

            task.status = status;
        }

        self.save_config().await?;

        Ok(())
    }

    /// Update task's last message
    pub async fn update_task_message(&self, session_id: &str, message: String) -> Result<()> {
        {
            let mut config = self.config.write().await;

            if let Some(task) = config.find_task_by_session(session_id) {
                task.update_last_message(message);
            }
        }

        // Note: We don't save config for message updates to avoid excessive I/O
        // Messages are transient and will be lost on restart

        Ok(())
    }

    /// Get a task by its session ID
    pub async fn get_task_by_session(&self, session_id: &str) -> Option<WorkspaceTask> {
        let config = self.config.read().await;
        config
            .tasks
            .iter()
            .find(|t| t.session_id.as_ref() == Some(&session_id.to_string()))
            .cloned()
    }

    /// Get all tasks across all workspaces
    pub async fn get_all_tasks(&self) -> Vec<WorkspaceTask> {
        let config = self.config.read().await;
        config.tasks.clone()
    }

    /// Get a specific task by ID
    pub async fn get_task(&self, task_id: &str) -> Option<WorkspaceTask> {
        let config = self.config.read().await;
        config.tasks.iter().find(|t| t.id == task_id).cloned()
    }

    /// Remove a task by ID
    pub async fn remove_task(&self, task_id: &str) -> Result<()> {
        let workspace_id = {
            let mut config = self.config.write().await;

            let task = config.remove_task(task_id).context("Task not found")?;

            task.workspace_id.clone()
        };

        self.save_config().await?;

        // Publish TaskRemoved event
        self.publish_event(WorkspaceUpdateEvent::TaskRemoved {
            workspace_id: workspace_id.clone(),
            task_id: task_id.to_string(),
        });

        log::info!("Removed task: {}", task_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test workspace service with temporary directory
    fn create_test_service(temp_dir: &std::path::Path) -> WorkspaceService {
        let config_path = temp_dir.join("workspace-config.json");
        WorkspaceService::new(config_path)
    }

    // ============== Constructor tests ==============

    #[tokio::test]
    async fn test_new_with_nonexistent_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("nonexistent-config.json");

        let service = WorkspaceService::new(config_path);

        // Should create with default empty config
        let config = service.get_config().await;
        assert!(config.workspaces.is_empty());
        assert!(config.tasks.is_empty());
        assert!(config.active_workspace_id.is_none());
    }

    // ============== Workspace CRUD tests ==============

    #[tokio::test]
    async fn test_add_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        // Create a project directory to add
        let project_dir = temp_dir.path().join("my-project");
        std::fs::create_dir(&project_dir).unwrap();

        let workspace = service.add_workspace(project_dir.clone()).await.unwrap();

        assert_eq!(workspace.name, "my-project");
        assert_eq!(workspace.path, project_dir);
        assert!(!workspace.id.is_empty());
    }

    #[tokio::test]
    async fn test_add_workspace_sets_first_as_active() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("first-project");
        std::fs::create_dir(&project_dir).unwrap();

        let workspace = service.add_workspace(project_dir).await.unwrap();
        let active = service.get_active_workspace().await;

        assert!(active.is_some());
        assert_eq!(active.unwrap().id, workspace.id);
    }

    #[tokio::test]
    async fn test_add_workspace_nonexistent_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let result = service
            .add_workspace(PathBuf::from("/nonexistent/path/12345"))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_add_workspace_duplicate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("duplicate-test");
        std::fs::create_dir(&project_dir).unwrap();

        // Add first time
        service.add_workspace(project_dir.clone()).await.unwrap();

        // Try to add again
        let result = service.add_workspace(project_dir).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_remove_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("to-remove");
        std::fs::create_dir(&project_dir).unwrap();

        let workspace = service.add_workspace(project_dir).await.unwrap();
        let workspace_id = workspace.id.clone();

        // Remove it
        service.remove_workspace(&workspace_id).await.unwrap();

        // Should no longer be found
        let result = service.get_workspace(&workspace_id).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_remove_active_workspace_resets_active() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        // Add two workspaces
        let project1 = temp_dir.path().join("project1");
        let project2 = temp_dir.path().join("project2");
        std::fs::create_dir(&project1).unwrap();
        std::fs::create_dir(&project2).unwrap();

        let ws1 = service.add_workspace(project1).await.unwrap();
        let ws2 = service.add_workspace(project2).await.unwrap();

        // ws1 is active (first added)
        assert_eq!(
            service.get_active_workspace().await.unwrap().id,
            ws1.id.clone()
        );

        // Remove active workspace
        service.remove_workspace(&ws1.id).await.unwrap();

        // Should switch to ws2
        let active = service.get_active_workspace().await;
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, ws2.id);
    }

    #[tokio::test]
    async fn test_list_workspaces() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        // Add multiple workspaces
        for name in ["proj-a", "proj-b", "proj-c"] {
            let dir = temp_dir.path().join(name);
            std::fs::create_dir(&dir).unwrap();
            service.add_workspace(dir).await.unwrap();
        }

        let workspaces = service.list_workspaces().await;
        assert_eq!(workspaces.len(), 3);
    }

    #[tokio::test]
    async fn test_get_active_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        // No workspaces yet
        assert!(service.get_active_workspace().await.is_none());

        // Add a workspace
        let project_dir = temp_dir.path().join("active-test");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        // Should be active
        let active = service.get_active_workspace().await.unwrap();
        assert_eq!(active.id, ws.id);
    }

    #[tokio::test]
    async fn test_set_active_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        // Add two workspaces
        let project1 = temp_dir.path().join("set-active-1");
        let project2 = temp_dir.path().join("set-active-2");
        std::fs::create_dir(&project1).unwrap();
        std::fs::create_dir(&project2).unwrap();

        let ws1 = service.add_workspace(project1).await.unwrap();
        let ws2 = service.add_workspace(project2).await.unwrap();

        // ws1 is active by default
        assert_eq!(
            service.get_active_workspace().await.unwrap().id,
            ws1.id.clone()
        );

        // Switch to ws2
        service.set_active_workspace(&ws2.id).await.unwrap();
        assert_eq!(
            service.get_active_workspace().await.unwrap().id,
            ws2.id.clone()
        );
    }

    #[tokio::test]
    async fn test_set_active_workspace_nonexistent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let result = service.set_active_workspace("nonexistent-id").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    // ============== Task tests ==============

    #[tokio::test]
    async fn test_create_task() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("task-test");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        let task = service
            .create_task(
                &ws.id,
                "Fix bug".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(task.name, "Fix bug");
        assert_eq!(task.agent_name, "claude");
        assert_eq!(task.workspace_id, ws.id);
        assert!(matches!(task.status, SessionStatus::Pending));
    }

    #[tokio::test]
    async fn test_create_task_nonexistent_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let result = service
            .create_task(
                "nonexistent-ws",
                "Task".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_set_task_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("session-test");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        let task = service
            .create_task(
                &ws.id,
                "Task".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();

        // Set session
        service
            .set_task_session(&task.id, "session-123".to_string())
            .await
            .unwrap();

        // Verify
        let updated_task = service.get_task(&task.id).await.unwrap();
        assert_eq!(updated_task.session_id, Some("session-123".to_string()));
        assert!(matches!(updated_task.status, SessionStatus::InProgress));
    }

    #[tokio::test]
    async fn test_get_workspace_tasks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("tasks-list");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        // Create multiple tasks
        service
            .create_task(
                &ws.id,
                "Task 1".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();
        service
            .create_task(
                &ws.id,
                "Task 2".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();

        let tasks = service.get_workspace_tasks(&ws.id).await;
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_get_task_by_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("by-session");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        let task = service
            .create_task(
                &ws.id,
                "Task".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();
        service
            .set_task_session(&task.id, "find-me-session".to_string())
            .await
            .unwrap();

        // Find by session
        let found = service.get_task_by_session("find-me-session").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, task.id);

        // Not found
        let not_found = service.get_task_by_session("nonexistent").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_remove_task() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("remove-task");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        let task = service
            .create_task(
                &ws.id,
                "To Remove".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();

        // Remove
        service.remove_task(&task.id).await.unwrap();

        // Should be gone
        assert!(service.get_task(&task.id).await.is_none());
    }

    #[tokio::test]
    async fn test_remove_workspace_cascades_tasks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("cascade-test");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        let task = service
            .create_task(
                &ws.id,
                "Task".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();

        // Remove workspace
        service.remove_workspace(&ws.id).await.unwrap();

        // Task should be gone too
        assert!(service.get_task(&task.id).await.is_none());
    }

    #[tokio::test]
    async fn test_update_task_status() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = create_test_service(temp_dir.path());

        let project_dir = temp_dir.path().join("status-test");
        std::fs::create_dir(&project_dir).unwrap();
        let ws = service.add_workspace(project_dir).await.unwrap();

        let task = service
            .create_task(
                &ws.id,
                "Task".to_string(),
                "claude".to_string(),
                "Auto".to_string(),
            )
            .await
            .unwrap();

        // Update status
        service
            .update_task_status(&task.id, SessionStatus::Completed)
            .await
            .unwrap();

        let updated = service.get_task(&task.id).await.unwrap();
        assert!(matches!(updated.status, SessionStatus::Completed));
    }

    #[tokio::test]
    async fn test_config_persistence_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("persist-test.json");

        // Create service and add data
        {
            let service = WorkspaceService::new(config_path.clone());

            let project_dir = temp_dir.path().join("persist-project");
            std::fs::create_dir(&project_dir).unwrap();
            let ws = service.add_workspace(project_dir).await.unwrap();

            service
                .create_task(
                    &ws.id,
                    "Persist Task".to_string(),
                    "claude".to_string(),
                    "Auto".to_string(),
                )
                .await
                .unwrap();
        }

        // Create new service instance with same config path
        let service2 = WorkspaceService::new(config_path);
        let workspaces = service2.list_workspaces().await;
        let tasks = service2.get_all_tasks().await;

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].name, "persist-project");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "Persist Task");
    }
}
