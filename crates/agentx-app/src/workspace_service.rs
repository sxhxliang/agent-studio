//! Workspace and task use-cases.
//!
//! Orchestrates the [`WorkspaceRepository`] (project folders + tasks) and derives
//! a last-message preview for each task from the [`SessionRepository`], so the
//! task list can show context without the UI touching storage. Mutations publish
//! a [`DomainEvent`] so panels refresh without being called directly.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;

use agentx_bus::EventBus;
use agentx_domain::{
    DomainEvent, SessionEvent, SessionId, SessionRepository, SessionStatus, StoreError, Task,
    TaskId, Workspace, WorkspaceId, WorkspaceRepository,
};

/// A task plus a one-line preview of its session's last message.
pub struct TaskView {
    pub task: Task,
    pub last_message: Option<String>,
}

pub struct WorkspaceService {
    repo: Arc<dyn WorkspaceRepository>,
    sessions: Arc<dyn SessionRepository>,
    bus: EventBus,
}

impl WorkspaceService {
    pub fn new(
        repo: Arc<dyn WorkspaceRepository>,
        sessions: Arc<dyn SessionRepository>,
        bus: EventBus,
    ) -> Self {
        Self {
            repo,
            sessions,
            bus,
        }
    }

    /// Every workspace with its tasks (newest first), each carrying a derived
    /// last-message preview.
    pub async fn workspaces_with_tasks(
        &self,
    ) -> Result<Vec<(Workspace, Vec<TaskView>)>, StoreError> {
        let workspaces = self.repo.list_workspaces().await?;
        let tasks = self.repo.list_tasks().await?;

        let mut grouped = Vec::with_capacity(workspaces.len());
        for workspace in workspaces {
            let mut views = Vec::new();
            for task in tasks.iter().filter(|task| task.workspace == workspace.id) {
                let last_message = match &task.session {
                    Some(session) => self.last_message(session).await,
                    None => None,
                };
                views.push(TaskView {
                    task: task.clone(),
                    last_message,
                });
            }
            views.sort_by_key(|view| std::cmp::Reverse(view.task.created_at));
            grouped.push((workspace, views));
        }
        Ok(grouped)
    }

    /// The first line of a session's most recent message, if any.
    async fn last_message(&self, session: &SessionId) -> Option<String> {
        let history = self.sessions.load(session).await.ok()?;
        history
            .iter()
            .rev()
            .find_map(|persisted| match &persisted.event {
                SessionEvent::AgentMessage { content } | SessionEvent::UserMessage { content } => {
                    let text: String = content
                        .iter()
                        .filter_map(|block| block.as_plain_text())
                        .collect();
                    let line = text.lines().next().unwrap_or("").trim().to_string();
                    (!line.is_empty()).then_some(line)
                }
                _ => None,
            })
    }

    /// Add a workspace for `path`, deriving its name from the folder.
    pub async fn add_workspace(&self, path: PathBuf) -> Result<Workspace, StoreError> {
        let workspace = Workspace::new(WorkspaceId::generate(), path, Utc::now());
        self.repo.add_workspace(workspace.clone()).await?;
        self.bus.publish(DomainEvent::WorkspaceAdded {
            workspace: workspace.clone(),
        });
        Ok(workspace)
    }

    pub async fn remove_workspace(&self, id: &WorkspaceId) -> Result<(), StoreError> {
        self.repo.remove_workspace(id).await?;
        self.bus.publish(DomainEvent::WorkspaceRemoved {
            workspace: id.clone(),
        });
        Ok(())
    }

    pub async fn add_task(&self, task: Task) -> Result<(), StoreError> {
        self.repo.add_task(task.clone()).await?;
        self.bus.publish(DomainEvent::TaskAdded { task });
        Ok(())
    }

    pub async fn remove_task(&self, id: &TaskId) -> Result<(), StoreError> {
        self.repo.remove_task(id).await?;
        self.bus
            .publish(DomainEvent::TaskRemoved { task: id.clone() });
        Ok(())
    }

    pub async fn update_task_status(
        &self,
        id: &TaskId,
        status: SessionStatus,
    ) -> Result<(), StoreError> {
        self.repo.update_task_status(id, status).await?;
        self.bus.publish(DomainEvent::TaskStatusChanged {
            task: id.clone(),
            status,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeSessionRepository, FakeWorkspaceRepository};
    use agentx_domain::AgentId;

    fn service() -> (WorkspaceService, EventBus) {
        let bus = EventBus::new();
        let service = WorkspaceService::new(
            Arc::new(FakeWorkspaceRepository::new()),
            Arc::new(FakeSessionRepository::new()),
            bus.clone(),
        );
        (service, bus)
    }

    #[tokio::test]
    async fn add_workspace_with_a_task_groups_them() {
        let (service, _bus) = service();
        let workspace = service
            .add_workspace(PathBuf::from("/tmp/demo"))
            .await
            .unwrap();

        let task = Task::new(
            TaskId::generate(),
            workspace.id.clone(),
            "Fix the bug".into(),
            AgentId::from("claude"),
            "code".into(),
            Utc::now(),
        );
        service.add_task(task.clone()).await.unwrap();

        let grouped = service.workspaces_with_tasks().await.unwrap();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].0.id, workspace.id);
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[0].1[0].task.id, task.id);
    }

    #[tokio::test]
    async fn update_task_status_publishes_and_persists() {
        let (service, bus) = service();
        let mut rx = bus.subscribe::<DomainEvent>();
        let workspace = service
            .add_workspace(PathBuf::from("/tmp/demo"))
            .await
            .unwrap();
        let task = Task::new(
            TaskId::generate(),
            workspace.id,
            "Task".into(),
            AgentId::from("claude"),
            "code".into(),
            Utc::now(),
        );
        let task_id = task.id.clone();
        service.add_task(task).await.unwrap();

        service
            .update_task_status(&task_id, SessionStatus::Completed)
            .await
            .unwrap();

        let grouped = service.workspaces_with_tasks().await.unwrap();
        assert_eq!(grouped[0].1[0].task.status, SessionStatus::Completed);

        let mut saw_status_change = false;
        while let Ok(event) = rx.try_recv() {
            if matches!(
                event,
                DomainEvent::TaskStatusChanged {
                    status: SessionStatus::Completed,
                    ..
                }
            ) {
                saw_status_change = true;
            }
        }
        assert!(saw_status_change);
    }
}
