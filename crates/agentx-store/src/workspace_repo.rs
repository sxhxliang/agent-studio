//! Filesystem implementation of [`WorkspaceRepository`].
//!
//! Persists the whole workspace/task set as one JSON file. The domain entities
//! derive `Serialize`/`Deserialize`, so no DTO layer is needed here. The set is
//! small and edited interactively, so each mutation does a load-modify-save.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use agentx_domain::{
    SessionStatus, StoreError, Task, TaskId, Workspace, WorkspaceId, WorkspaceRepository,
};

use crate::{io_err, serde_err};

#[derive(Default, Serialize, Deserialize)]
struct WorkspaceData {
    #[serde(default)]
    workspaces: Vec<Workspace>,
    #[serde(default)]
    tasks: Vec<Task>,
}

/// Reads and writes workspaces + tasks as a single JSON file.
pub struct FsWorkspaceRepository {
    path: PathBuf,
}

impl FsWorkspaceRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Load the data, treating a missing file as an empty set (the file only
    /// appears once the user adds their first workspace).
    fn read(&self) -> Result<WorkspaceData, StoreError> {
        match std::fs::read_to_string(&self.path) {
            Ok(data) => serde_json::from_str(&data).map_err(serde_err),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(WorkspaceData::default())
            }
            Err(error) => Err(io_err(error)),
        }
    }

    fn write(&self, data: &WorkspaceData) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(io_err)?;
        }
        let json = serde_json::to_string_pretty(data).map_err(serde_err)?;
        std::fs::write(&self.path, json).map_err(io_err)
    }
}

#[async_trait]
impl WorkspaceRepository for FsWorkspaceRepository {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
        Ok(self.read()?.workspaces)
    }

    async fn list_tasks(&self) -> Result<Vec<Task>, StoreError> {
        Ok(self.read()?.tasks)
    }

    async fn add_workspace(&self, workspace: Workspace) -> Result<(), StoreError> {
        let mut data = self.read()?;
        data.workspaces
            .retain(|existing| existing.id != workspace.id);
        data.workspaces.push(workspace);
        self.write(&data)
    }

    async fn remove_workspace(&self, id: &WorkspaceId) -> Result<(), StoreError> {
        let mut data = self.read()?;
        data.workspaces.retain(|workspace| &workspace.id != id);
        data.tasks.retain(|task| &task.workspace != id);
        self.write(&data)
    }

    async fn add_task(&self, task: Task) -> Result<(), StoreError> {
        let mut data = self.read()?;
        data.tasks.retain(|existing| existing.id != task.id);
        data.tasks.push(task);
        self.write(&data)
    }

    async fn remove_task(&self, id: &TaskId) -> Result<(), StoreError> {
        let mut data = self.read()?;
        data.tasks.retain(|task| &task.id != id);
        self.write(&data)
    }

    async fn update_task_status(
        &self,
        id: &TaskId,
        status: SessionStatus,
    ) -> Result<(), StoreError> {
        let mut data = self.read()?;
        if let Some(task) = data.tasks.iter_mut().find(|task| &task.id == id) {
            task.status = status;
        }
        self.write(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentx_domain::AgentId;
    use chrono::Utc;

    #[tokio::test]
    async fn add_list_update_remove_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        // Nested path exercises create_dir_all.
        let repo = FsWorkspaceRepository::new(dir.path().join("nested").join("workspaces.json"));

        let workspace = Workspace::new(
            WorkspaceId::generate(),
            PathBuf::from("/tmp/project"),
            Utc::now(),
        );
        repo.add_workspace(workspace.clone()).await.unwrap();

        let task = Task::new(
            TaskId::generate(),
            workspace.id.clone(),
            "Task".into(),
            AgentId::from("claude"),
            "code".into(),
            Utc::now(),
        );
        let task_id = task.id.clone();
        repo.add_task(task).await.unwrap();
        repo.update_task_status(&task_id, SessionStatus::Completed)
            .await
            .unwrap();

        assert_eq!(repo.list_workspaces().await.unwrap().len(), 1);
        let tasks = repo.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, SessionStatus::Completed);

        // Removing the workspace cascades to its tasks.
        repo.remove_workspace(&workspace.id).await.unwrap();
        assert!(repo.list_workspaces().await.unwrap().is_empty());
        assert!(repo.list_tasks().await.unwrap().is_empty());
    }
}
