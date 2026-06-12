//! In-memory port fakes for unit-testing the use-cases.
//!
//! These let the application layer be exercised with no real agent process and
//! no filesystem — the orchestration logic is what we want to pin down, so the
//! ports are stubbed with configurable, inspectable behavior. Test-only.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;

use agentx_domain::{
    AgentError, AgentGateway, AgentId, AgentStatus, Config, ConfigStore, ContentBlock, FileEntry,
    McpServerConfig, PermissionOutcome, PersistedEvent, SessionId, SessionInit, SessionRepository,
    SessionStatus, StopReason, StoreError, Task, TaskId, Workspace, WorkspaceFiles, WorkspaceId,
    WorkspaceRepository,
};

/// A configurable, recording [`AgentGateway`].
#[derive(Default)]
pub(crate) struct FakeAgentGateway {
    state: Mutex<GatewayState>,
}

#[derive(Default)]
struct GatewayState {
    /// Session id handed back by create/resume/load (default `"fake-session"`).
    session_id: Option<SessionId>,
    /// Reason returned by `prompt` (default [`StopReason::EndTurn`]).
    prompt_reason: Option<StopReason>,
    /// When set, `prompt` fails with a transport error instead of returning.
    prompt_fails: bool,
    /// Every `(session, content)` passed to `prompt`, in order.
    prompts: Vec<(SessionId, Vec<ContentBlock>)>,
    /// Every agent passed to `create_session`, in order.
    created: Vec<AgentId>,
    /// Every session passed to `cancel`, in order.
    cancelled: Vec<SessionId>,
}

impl FakeAgentGateway {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_session_id(self, id: impl Into<SessionId>) -> Self {
        self.state.lock().unwrap().session_id = Some(id.into());
        self
    }

    pub(crate) fn with_prompt_reason(self, reason: StopReason) -> Self {
        self.state.lock().unwrap().prompt_reason = Some(reason);
        self
    }

    pub(crate) fn failing_prompt(self) -> Self {
        self.state.lock().unwrap().prompt_fails = true;
        self
    }

    pub(crate) fn prompts(&self) -> Vec<(SessionId, Vec<ContentBlock>)> {
        self.state.lock().unwrap().prompts.clone()
    }

    pub(crate) fn created(&self) -> Vec<AgentId> {
        self.state.lock().unwrap().created.clone()
    }

    pub(crate) fn cancelled(&self) -> Vec<SessionId> {
        self.state.lock().unwrap().cancelled.clone()
    }

    fn session_init(&self) -> SessionInit {
        let session_id = self
            .state
            .lock()
            .unwrap()
            .session_id
            .clone()
            .unwrap_or_else(|| SessionId::from("fake-session"));
        SessionInit {
            session_id,
            config_options: Vec::new(),
            modes: Vec::new(),
            current_mode: None,
            commands: Vec::new(),
        }
    }
}

#[async_trait]
impl AgentGateway for FakeAgentGateway {
    fn status(&self, _agent: &AgentId) -> AgentStatus {
        AgentStatus::Ready
    }

    async fn create_session(
        &self,
        agent: &AgentId,
        _cwd: &Path,
        _mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        self.state.lock().unwrap().created.push(agent.clone());
        Ok(self.session_init())
    }

    async fn resume_session(
        &self,
        _agent: &AgentId,
        _session: &SessionId,
        _cwd: &Path,
        _mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        Ok(self.session_init())
    }

    async fn load_session(
        &self,
        _agent: &AgentId,
        _session: &SessionId,
        _cwd: &Path,
        _mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        Ok(self.session_init())
    }

    async fn prompt(
        &self,
        session: &SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<StopReason, AgentError> {
        let mut state = self.state.lock().unwrap();
        state.prompts.push((session.clone(), content));
        if state.prompt_fails {
            Err(AgentError::Transport("fake prompt failure".into()))
        } else {
            Ok(state.prompt_reason.unwrap_or(StopReason::EndTurn))
        }
    }

    async fn cancel(&self, _session: &SessionId) -> Result<(), AgentError> {
        self.state.lock().unwrap().cancelled.push(_session.clone());
        Ok(())
    }

    async fn set_mode(&self, _session: &SessionId, _mode_id: &str) -> Result<(), AgentError> {
        Ok(())
    }

    async fn set_config_option(
        &self,
        _session: &SessionId,
        _config_id: &str,
        _value: &str,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    async fn resolve_permission(
        &self,
        _session: &SessionId,
        _permission_id: &str,
        _outcome: PermissionOutcome,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

/// An in-memory [`SessionRepository`].
#[derive(Default)]
pub(crate) struct FakeSessionRepository {
    sessions: Mutex<HashMap<SessionId, Vec<PersistedEvent>>>,
}

impl FakeSessionRepository {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionRepository for FakeSessionRepository {
    async fn append(&self, session: &SessionId, event: PersistedEvent) -> Result<(), StoreError> {
        self.sessions
            .lock()
            .unwrap()
            .entry(session.clone())
            .or_default()
            .push(event);
        Ok(())
    }

    async fn load(&self, session: &SessionId) -> Result<Vec<PersistedEvent>, StoreError> {
        self.sessions
            .lock()
            .unwrap()
            .get(session)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(session.to_string()))
    }

    async fn delete(&self, session: &SessionId) -> Result<(), StoreError> {
        self.sessions.lock().unwrap().remove(session);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<SessionId>, StoreError> {
        Ok(self.sessions.lock().unwrap().keys().cloned().collect())
    }

    async fn exists(&self, session: &SessionId) -> bool {
        self.sessions.lock().unwrap().contains_key(session)
    }

    async fn flush(&self, _session: &SessionId) -> Result<(), StoreError> {
        Ok(())
    }
}

/// An in-memory [`ConfigStore`] that records every saved config.
#[derive(Default)]
pub(crate) struct FakeConfigStore {
    config: Mutex<Config>,
    saved: Mutex<Vec<Config>>,
}

impl FakeConfigStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn saved(&self) -> Vec<Config> {
        self.saved.lock().unwrap().clone()
    }
}

#[async_trait]
impl ConfigStore for FakeConfigStore {
    async fn load(&self) -> Result<Config, StoreError> {
        Ok(self.config.lock().unwrap().clone())
    }

    async fn save(&self, config: &Config) -> Result<(), StoreError> {
        self.saved.lock().unwrap().push(config.clone());
        *self.config.lock().unwrap() = config.clone();
        Ok(())
    }
}

/// An in-memory [`WorkspaceRepository`].
#[derive(Default)]
pub(crate) struct FakeWorkspaceRepository {
    workspaces: Mutex<Vec<Workspace>>,
    tasks: Mutex<Vec<Task>>,
}

impl FakeWorkspaceRepository {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkspaceRepository for FakeWorkspaceRepository {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
        Ok(self.workspaces.lock().unwrap().clone())
    }

    async fn list_tasks(&self) -> Result<Vec<Task>, StoreError> {
        Ok(self.tasks.lock().unwrap().clone())
    }

    async fn add_workspace(&self, workspace: Workspace) -> Result<(), StoreError> {
        let mut workspaces = self.workspaces.lock().unwrap();
        workspaces.retain(|existing| existing.id != workspace.id);
        workspaces.push(workspace);
        Ok(())
    }

    async fn remove_workspace(&self, id: &WorkspaceId) -> Result<(), StoreError> {
        self.workspaces.lock().unwrap().retain(|w| &w.id != id);
        self.tasks.lock().unwrap().retain(|t| &t.workspace != id);
        Ok(())
    }

    async fn add_task(&self, task: Task) -> Result<(), StoreError> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|existing| existing.id != task.id);
        tasks.push(task);
        Ok(())
    }

    async fn remove_task(&self, id: &TaskId) -> Result<(), StoreError> {
        self.tasks.lock().unwrap().retain(|t| &t.id != id);
        Ok(())
    }

    async fn update_task_status(
        &self,
        id: &TaskId,
        status: SessionStatus,
    ) -> Result<(), StoreError> {
        if let Some(task) = self.tasks.lock().unwrap().iter_mut().find(|t| &t.id == id) {
            task.status = status;
        }
        Ok(())
    }
}

/// An in-memory [`WorkspaceFiles`] returning a fixed set, filtered by query.
#[derive(Default)]
pub(crate) struct FakeWorkspaceFiles {
    entries: Vec<FileEntry>,
}

impl FakeWorkspaceFiles {
    pub(crate) fn with_entries(entries: Vec<FileEntry>) -> Self {
        Self { entries }
    }
}

#[async_trait]
impl WorkspaceFiles for FakeWorkspaceFiles {
    async fn list_files(&self, _root: &Path, query: &str) -> Result<Vec<FileEntry>, StoreError> {
        let query = query.to_lowercase();
        Ok(self
            .entries
            .iter()
            .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
            .cloned()
            .collect())
    }
}
