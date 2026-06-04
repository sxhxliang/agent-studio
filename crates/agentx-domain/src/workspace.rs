//! Workspaces (project folders) and the tasks the user runs within them.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{AgentId, SessionId, TaskId, WorkspaceId};
use crate::session::SessionStatus;

/// A local project folder the user works in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}

impl Workspace {
    /// Create a workspace, deriving the display name from the folder name. The
    /// caller supplies `now` to keep the domain pure.
    pub fn new(id: WorkspaceId, path: PathBuf, now: DateTime<Utc>) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unnamed Project")
            .to_string();
        Self {
            id,
            name,
            path,
            created_at: now,
            last_accessed: now,
        }
    }

    pub fn touch(&mut self, now: DateTime<Utc>) {
        self.last_accessed = now;
    }
}

/// A unit of work started against an agent, optionally bound to a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub workspace: WorkspaceId,
    pub name: String,
    pub agent: AgentId,
    pub mode: String,
    pub session: Option<SessionId>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub fn new(
        id: TaskId,
        workspace: WorkspaceId,
        name: String,
        agent: AgentId,
        mode: String,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            workspace,
            name,
            agent,
            mode,
            session: None,
            status: SessionStatus::Pending,
            created_at: now,
        }
    }

    /// Associate a session with this task and mark it as running.
    pub fn bind_session(&mut self, session: SessionId) {
        self.session = Some(session);
        self.status = SessionStatus::Running;
    }
}
