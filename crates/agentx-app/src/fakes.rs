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
    AgentError, AgentGateway, AgentId, AgentStatus, ContentBlock, McpServerConfig,
    PermissionOutcome, PersistedEvent, SessionId, SessionInit, SessionRepository, StopReason,
    StoreError,
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
            modes: Vec::new(),
            current_mode: None,
            models: Vec::new(),
            current_model: None,
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
        Ok(())
    }

    async fn set_mode(&self, _session: &SessionId, _mode_id: &str) -> Result<(), AgentError> {
        Ok(())
    }

    async fn set_model(&self, _session: &SessionId, _model_id: &str) -> Result<(), AgentError> {
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
