use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::event_bus::{EventHub, WorkspaceUpdateEvent};
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
    event_hub: Option<EventHub>,
}

impl WorkspaceService {
    /// Create a new WorkspaceService
    pub fn new(config_path: PathBuf) -> Self {
        let config = Self::load_config(&config_path).unwrap_or_default();

        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            event_hub: None,
        }
    }

    /// Set the event hub (called after AppState initialization)
    pub fn set_event_hub(&mut self, hub: EventHub) {
        self.event_hub = Some(hub);
    }

    /// Publish a workspace update event if hub is available
    fn publish_event(&self, event: WorkspaceUpdateEvent) {
        if let Some(hub) = &self.event_hub {
            log::debug!("[WorkspaceService] Publishing event: {:?}", &event);
            hub.publish_workspace_update(event);
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
