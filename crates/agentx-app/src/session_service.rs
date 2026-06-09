//! Session lifecycle and message-sending use-cases.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::Mutex;

use agentx_bus::EventBus;
use agentx_domain::{
    AgentError, AgentGateway, AgentId, ContentBlock, DomainEvent, McpServerConfig,
    PermissionOutcome, PersistedEvent, Session, SessionEvent, SessionId, SessionInit,
    SessionRepository, SessionStatus, StopReason, StoreError,
};

/// Orchestrates the session lifecycle and the send-a-message use-case.
///
/// Owns the in-memory set of live sessions — the single piece of mutable state
/// these use-cases share — and the ports they drive. It is deliberately *not* a
/// god-object: it holds only the agent gateway, the event bus, and the session
/// repository, nothing about agent administration, config, or workspaces.
pub struct SessionService {
    gateway: Arc<dyn AgentGateway>,
    bus: EventBus,
    repository: Arc<dyn SessionRepository>,
    sessions: Mutex<Registry>,
}

#[derive(Default)]
struct Registry {
    /// The current session per agent, so repeated sends reuse one session.
    by_agent: HashMap<AgentId, SessionId>,
    /// Live session state, by id.
    by_id: HashMap<SessionId, Live>,
}

/// A live session: its lifecycle state plus the capabilities it was created
/// with (modes/models/commands), which the UI reads to build its selectors.
struct Live {
    session: Session,
    init: SessionInit,
}

impl SessionService {
    pub fn new(
        gateway: Arc<dyn AgentGateway>,
        bus: EventBus,
        repository: Arc<dyn SessionRepository>,
    ) -> Self {
        Self {
            gateway,
            bus,
            repository,
            sessions: Mutex::new(Registry::default()),
        }
    }

    /// Return the agent's current session, creating one through the gateway if
    /// none exists yet. New sessions start [`SessionStatus::Pending`].
    pub async fn get_or_create_session(
        &self,
        agent: &AgentId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionId, AgentError> {
        // The registry lock is intentionally held across the gateway call so two
        // concurrent callers cannot create two sessions for the same agent.
        // Session creation is a low-frequency, UI-driven action, so the lost
        // concurrency costs nothing and the invariant stays simple.
        let mut registry = self.sessions.lock().await;
        if let Some(existing) = registry.by_agent.get(agent) {
            return Ok(existing.clone());
        }
        let init = self.gateway.create_session(agent, cwd, mcp_servers).await?;
        let session = Session::new(
            init.session_id.clone(),
            agent.clone(),
            cwd.to_path_buf(),
            Utc::now(),
        );
        let id = session.id.clone();
        registry.by_agent.insert(agent.clone(), id.clone());
        registry.by_id.insert(id.clone(), Live { session, init });
        Ok(id)
    }

    /// Start a fresh session, bypassing the per-agent reuse that
    /// [`get_or_create_session`](Self::get_or_create_session) does. Becomes the
    /// agent's current session.
    pub async fn new_session(
        &self,
        agent: &AgentId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        let init = self.gateway.create_session(agent, cwd, mcp_servers).await?;
        let mut registry = self.sessions.lock().await;
        let id = init.session_id.clone();
        let session = Session::new(id.clone(), agent.clone(), cwd.to_path_buf(), Utc::now());
        registry.by_agent.insert(agent.clone(), id.clone());
        registry.by_id.insert(
            id,
            Live {
                session,
                init: init.clone(),
            },
        );
        Ok(init)
    }

    /// Resume a previously-created session: reconnect the agent to it and make
    /// it the live session (registered like a freshly-created one), so prompts
    /// continue the conversation. The agent may not support resumption, in which
    /// case the gateway error propagates.
    pub async fn resume_session(
        &self,
        agent: &AgentId,
        session: &SessionId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        let init = self
            .gateway
            .resume_session(agent, session, cwd, mcp_servers)
            .await?;
        let mut registry = self.sessions.lock().await;
        let id = init.session_id.clone();
        let live = Session::new(id.clone(), agent.clone(), cwd.to_path_buf(), Utc::now());
        registry.by_agent.insert(agent.clone(), id.clone());
        registry.by_id.insert(
            id,
            Live {
                session: live,
                init: init.clone(),
            },
        );
        Ok(init)
    }

    /// Send a user prompt and run the turn to completion.
    ///
    /// The user message and the stop reason are published as
    /// [`DomainEvent::SessionAppended`] (consumed by the persistence projector
    /// and the UI); the agent's streamed reply is published by the gateway
    /// itself. The session moves to `Running` for the turn and back to `Idle`
    /// when it ends, or to `Failed` if the prompt errors.
    pub async fn send_message(
        &self,
        session: &SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<StopReason, AgentError> {
        self.set_status(session, SessionStatus::Running).await?;
        self.append(session, SessionEvent::UserMessage {
            content: content.clone(),
        });

        match self.gateway.prompt(session, content).await {
            Ok(reason) => {
                // The turn already succeeded; a status-update failure here (e.g.
                // the session was concurrently deleted) must not mask that.
                let _ = self.set_status(session, SessionStatus::Idle).await;
                self.append(session, SessionEvent::Stopped { reason });
                Ok(reason)
            }
            Err(error) => {
                let _ = self.set_status(session, SessionStatus::Failed).await;
                Err(error)
            }
        }
    }

    /// The capabilities (modes, models, commands) the session was created with,
    /// if it is still live. The UI reads these to populate its selectors.
    pub async fn session_init(&self, session: &SessionId) -> Option<SessionInit> {
        self.sessions
            .lock()
            .await
            .by_id
            .get(session)
            .map(|live| live.init.clone())
    }

    /// Switch the session's mode (e.g. "ask" → "code").
    pub async fn set_mode(&self, session: &SessionId, mode_id: &str) -> Result<(), AgentError> {
        self.gateway.set_mode(session, mode_id).await
    }

    /// Switch the session's model.
    pub async fn set_model(&self, session: &SessionId, model_id: &str) -> Result<(), AgentError> {
        self.gateway.set_model(session, model_id).await
    }

    /// Forward the user's decision on a permission request the agent raised
    /// mid-turn. The request itself arrives as
    /// [`DomainEvent::PermissionRequested`]; this carries the answer back.
    pub async fn resolve_permission(
        &self,
        session: &SessionId,
        permission_id: &str,
        outcome: PermissionOutcome,
    ) -> Result<(), AgentError> {
        self.gateway
            .resolve_permission(session, permission_id, outcome)
            .await
    }

    /// The persisted timeline of a session.
    pub async fn history(&self, session: &SessionId) -> Result<Vec<PersistedEvent>, StoreError> {
        self.repository.load(session).await
    }

    /// Every session that has a persisted timeline.
    pub async fn list_sessions(&self) -> Result<Vec<SessionId>, StoreError> {
        self.repository.list().await
    }

    /// Forget a session's live state and delete its persisted timeline.
    pub async fn delete_session(&self, session: &SessionId) -> Result<(), StoreError> {
        {
            let mut registry = self.sessions.lock().await;
            registry.by_id.remove(session);
            registry.by_agent.retain(|_, id| id != session);
        }
        self.repository.delete(session).await
    }

    async fn set_status(
        &self,
        session: &SessionId,
        next: SessionStatus,
    ) -> Result<(), AgentError> {
        let status = {
            let mut registry = self.sessions.lock().await;
            let entry = registry
                .by_id
                .get_mut(session)
                .ok_or_else(|| AgentError::SessionNotFound(session.clone()))?;
            // An invalid transition is a real lifecycle bug, surfaced as a
            // protocol error rather than silently ignored.
            entry
                .session
                .transition(next, Utc::now())
                .map_err(|e| AgentError::Protocol(e.to_string()))?;
            entry.session.status
        };
        self.bus.publish(DomainEvent::SessionStatusChanged {
            session: session.clone(),
            status,
        });
        Ok(())
    }

    fn append(&self, session: &SessionId, event: SessionEvent) {
        self.bus.publish(DomainEvent::SessionAppended {
            session: session.clone(),
            event,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeAgentGateway, FakeSessionRepository};

    fn drain(rx: &mut agentx_bus::Receiver<DomainEvent>) -> Vec<DomainEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    #[tokio::test]
    async fn get_or_create_creates_once_then_reuses() {
        let gateway = Arc::new(FakeAgentGateway::new().with_session_id("s1"));
        let service = SessionService::new(
            gateway.clone(),
            EventBus::new(),
            Arc::new(FakeSessionRepository::new()),
        );
        let agent = AgentId::from("claude");

        let first = service
            .get_or_create_session(&agent, Path::new("."), &[])
            .await
            .unwrap();
        let second = service
            .get_or_create_session(&agent, Path::new("."), &[])
            .await
            .unwrap();

        assert_eq!(first, SessionId::from("s1"));
        assert_eq!(first, second);
        // The gateway was asked to create exactly one session.
        assert_eq!(gateway.created(), vec![agent]);
    }

    #[tokio::test]
    async fn send_message_publishes_lifecycle_and_message_events() {
        let gateway = Arc::new(
            FakeAgentGateway::new()
                .with_session_id("s1")
                .with_prompt_reason(StopReason::EndTurn),
        );
        let bus = EventBus::new();
        let mut rx = bus.subscribe::<DomainEvent>();
        let service = SessionService::new(
            gateway.clone(),
            bus.clone(),
            Arc::new(FakeSessionRepository::new()),
        );
        let session = service
            .get_or_create_session(&AgentId::from("claude"), Path::new("."), &[])
            .await
            .unwrap();

        let reason = service
            .send_message(&session, vec![ContentBlock::text("hi")])
            .await
            .unwrap();
        assert_eq!(reason, StopReason::EndTurn);

        assert_eq!(
            drain(&mut rx),
            vec![
                DomainEvent::SessionStatusChanged {
                    session: session.clone(),
                    status: SessionStatus::Running,
                },
                DomainEvent::SessionAppended {
                    session: session.clone(),
                    event: SessionEvent::UserMessage {
                        content: vec![ContentBlock::text("hi")],
                    },
                },
                DomainEvent::SessionStatusChanged {
                    session: session.clone(),
                    status: SessionStatus::Idle,
                },
                DomainEvent::SessionAppended {
                    session: session.clone(),
                    event: SessionEvent::Stopped {
                        reason: StopReason::EndTurn,
                    },
                },
            ]
        );
        // The prompt reached the gateway with the user's content.
        assert_eq!(
            gateway.prompts(),
            vec![(session, vec![ContentBlock::text("hi")])]
        );
    }

    #[tokio::test]
    async fn failed_prompt_marks_session_failed_and_propagates_error() {
        let gateway = Arc::new(
            FakeAgentGateway::new()
                .with_session_id("s1")
                .failing_prompt(),
        );
        let bus = EventBus::new();
        let mut rx = bus.subscribe::<DomainEvent>();
        let service =
            SessionService::new(gateway, bus.clone(), Arc::new(FakeSessionRepository::new()));
        let session = service
            .get_or_create_session(&AgentId::from("claude"), Path::new("."), &[])
            .await
            .unwrap();

        let error = service
            .send_message(&session, vec![ContentBlock::text("hi")])
            .await
            .unwrap_err();
        assert!(matches!(error, AgentError::Transport(_)));

        let statuses: Vec<SessionStatus> = drain(&mut rx)
            .into_iter()
            .filter_map(|event| match event {
                DomainEvent::SessionStatusChanged { status, .. } => Some(status),
                _ => None,
            })
            .collect();
        assert_eq!(
            statuses,
            vec![SessionStatus::Running, SessionStatus::Failed]
        );
    }

    #[tokio::test]
    async fn sending_to_an_unknown_session_is_not_found() {
        let service = SessionService::new(
            Arc::new(FakeAgentGateway::new()),
            EventBus::new(),
            Arc::new(FakeSessionRepository::new()),
        );
        let error = service
            .send_message(&SessionId::from("ghost"), vec![ContentBlock::text("hi")])
            .await
            .unwrap_err();
        assert!(matches!(error, AgentError::SessionNotFound(_)));
    }

    #[tokio::test]
    async fn history_reads_the_persisted_timeline() {
        let repository = Arc::new(FakeSessionRepository::new());
        let session = SessionId::from("s1");
        repository
            .append(
                &session,
                PersistedEvent::new(
                    SessionEvent::AgentThought {
                        text: "thinking".into(),
                    },
                    Utc::now(),
                ),
            )
            .await
            .unwrap();
        let service =
            SessionService::new(Arc::new(FakeAgentGateway::new()), EventBus::new(), repository);

        let events = service.history(&session).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn session_init_is_cached_and_returned_by_id() {
        let gateway = Arc::new(FakeAgentGateway::new().with_session_id("s1"));
        let service = SessionService::new(
            gateway,
            EventBus::new(),
            Arc::new(FakeSessionRepository::new()),
        );
        let session = service
            .get_or_create_session(&AgentId::from("claude"), Path::new("."), &[])
            .await
            .unwrap();

        let init = service.session_init(&session).await.expect("caps are cached");
        assert_eq!(init.session_id, session);
        // An unknown session has no cached capabilities.
        assert!(service.session_init(&SessionId::from("ghost")).await.is_none());
    }
}
