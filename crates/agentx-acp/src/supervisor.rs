//! The composition-facing adapter: one [`AcpSupervisor`] fronts every agent.
//!
//! It implements the domain's [`AgentGateway`] and [`AgentRegistry`] ports by
//! owning a set of [`AgentWorker`] actors (one per agent) plus the routing they
//! need: which agent owns which session, and the shared [`PermissionStore`] the
//! workers park requests in. Nothing here speaks ACP directly — the workers and
//! [`crate::mapping`] do — so the application sees only domain types and errors.
//!
//! Locks are held only to read or swap a handle, never across an `.await`: each
//! method clones the worker (or the routing entry) out from under the lock and
//! then awaits it. The `async_trait` futures are `Send`, which the standard-
//! library guards would otherwise prevent — a useful compile-time check that we
//! are not holding a lock across suspension.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, RwLock};
use std::sync::Arc;

use async_trait::async_trait;

use agentx_bus::EventBus;
use agentx_domain::{
    AgentConfig, AgentDescriptor, AgentError, AgentGateway, AgentId, AgentRegistry, AgentStatus,
    ContentBlock, DomainEvent, McpServerConfig, PermissionOutcome, ProxyConfig, SessionId,
    SessionInit, StopReason,
};

use crate::worker::{AgentWorker, PermissionStore};

/// Per-session routing: which agent owns the session, plus the id of its model
/// selector config option (if the agent advertised one), so `set_model` can
/// target the right option later.
struct Route {
    agent: AgentId,
    model_config_id: Option<String>,
}

/// Supervises the live agent subprocesses and routes domain calls to them.
pub struct AcpSupervisor {
    bus: EventBus,
    permissions: Arc<PermissionStore>,
    agents: RwLock<HashMap<AgentId, AgentWorker>>,
    /// Routing for each open session, so session-scoped calls (prompt, cancel,
    /// set_mode, set_model) reach the right worker without the caller naming the
    /// agent.
    sessions: Mutex<HashMap<SessionId, Route>>,
    /// Applied to every agent started after it is set.
    proxy: Mutex<ProxyConfig>,
}

impl AcpSupervisor {
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            permissions: Arc::new(PermissionStore::default()),
            agents: RwLock::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            proxy: Mutex::new(ProxyConfig::default()),
        }
    }

    fn worker(&self, agent: &AgentId) -> Result<AgentWorker, AgentError> {
        self.agents
            .read()
            .expect("agents poisoned")
            .get(agent)
            .cloned()
            .ok_or_else(|| AgentError::Unavailable(agent.clone()))
    }

    fn worker_for_session(&self, session: &SessionId) -> Result<AgentWorker, AgentError> {
        let agent = self
            .sessions
            .lock()
            .expect("sessions poisoned")
            .get(session)
            .map(|route| route.agent.clone())
            .ok_or_else(|| AgentError::SessionNotFound(session.clone()))?;
        self.worker(&agent)
    }

    /// Record which agent owns a freshly opened session and its model selector.
    fn route_session(&self, agent: &AgentId, start_session: SessionId, model_config_id: Option<String>) {
        self.sessions.lock().expect("sessions poisoned").insert(
            start_session,
            Route {
                agent: agent.clone(),
                model_config_id,
            },
        );
    }

    /// The model selector config id captured when the session opened, if any.
    fn model_config_for(&self, session: &SessionId) -> Option<String> {
        self.sessions
            .lock()
            .expect("sessions poisoned")
            .get(session)
            .and_then(|route| route.model_config_id.clone())
    }
}

#[async_trait]
impl AgentGateway for AcpSupervisor {
    fn status(&self, agent: &AgentId) -> AgentStatus {
        self.agents
            .read()
            .expect("agents poisoned")
            .get(agent)
            .map(AgentWorker::status)
            .unwrap_or(AgentStatus::Unavailable {
                reason: "agent is not configured".into(),
            })
    }

    async fn create_session(
        &self,
        agent: &AgentId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        let worker = self.worker(agent)?;
        let start = worker
            .create_session(cwd.to_path_buf(), mcp_servers.to_vec())
            .await?;
        self.route_session(agent, start.init.session_id.clone(), start.model_config_id);
        Ok(start.init)
    }

    async fn resume_session(
        &self,
        agent: &AgentId,
        session: &SessionId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        let worker = self.worker(agent)?;
        let start = worker
            .resume_session(session.clone(), cwd.to_path_buf(), mcp_servers.to_vec())
            .await?;
        self.route_session(agent, start.init.session_id.clone(), start.model_config_id);
        Ok(start.init)
    }

    async fn load_session(
        &self,
        agent: &AgentId,
        session: &SessionId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError> {
        let worker = self.worker(agent)?;
        let start = worker
            .load_session(session.clone(), cwd.to_path_buf(), mcp_servers.to_vec())
            .await?;
        self.route_session(agent, start.init.session_id.clone(), start.model_config_id);
        Ok(start.init)
    }

    async fn prompt(
        &self,
        session: &SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<StopReason, AgentError> {
        self.worker_for_session(session)?
            .prompt(session.clone(), content)
            .await
    }

    async fn cancel(&self, session: &SessionId) -> Result<(), AgentError> {
        self.worker_for_session(session)?
            .cancel(session.clone())
            .await
    }

    async fn set_mode(&self, session: &SessionId, mode_id: &str) -> Result<(), AgentError> {
        self.worker_for_session(session)?
            .set_mode(session.clone(), mode_id.to_string())
            .await
    }

    async fn set_model(&self, session: &SessionId, model_id: &str) -> Result<(), AgentError> {
        let config_id = self.model_config_for(session).ok_or_else(|| {
            AgentError::Protocol("the session's agent has no model selector".into())
        })?;
        self.worker_for_session(session)?
            .set_model(session.clone(), config_id, model_id.to_string())
            .await
    }

    async fn resolve_permission(
        &self,
        _session: &SessionId,
        permission_id: &str,
        outcome: PermissionOutcome,
    ) -> Result<(), AgentError> {
        self.permissions.answer(permission_id, outcome)
    }
}

#[async_trait]
impl AgentRegistry for AcpSupervisor {
    fn agents(&self) -> Vec<AgentDescriptor> {
        self.agents
            .read()
            .expect("agents poisoned")
            .iter()
            .map(|(id, worker)| AgentDescriptor {
                id: id.clone(),
                status: worker.status(),
            })
            .collect()
    }

    async fn add_agent(&self, agent: AgentId, config: AgentConfig) -> Result<(), AgentError> {
        if self
            .agents
            .read()
            .expect("agents poisoned")
            .contains_key(&agent)
        {
            return Err(AgentError::Protocol(format!(
                "agent `{agent}` already exists"
            )));
        }
        let proxy = self.proxy.lock().expect("proxy poisoned").clone();
        let worker =
            AgentWorker::spawn(agent.clone(), config, proxy, self.bus.clone(), self.permissions.clone())
                .await?;
        let status = worker.status();
        self.agents
            .write()
            .expect("agents poisoned")
            .insert(agent.clone(), worker);
        self.bus
            .publish(DomainEvent::AgentStatusChanged { agent, status });
        Ok(())
    }

    async fn remove_agent(&self, agent: &AgentId) -> Result<(), AgentError> {
        let worker = self
            .agents
            .write()
            .expect("agents poisoned")
            .remove(agent);
        let Some(worker) = worker else {
            return Err(AgentError::Unavailable(agent.clone()));
        };
        worker.shutdown().await;
        self.sessions
            .lock()
            .expect("sessions poisoned")
            .retain(|_, route| route.agent != *agent);
        self.bus.publish(DomainEvent::AgentStatusChanged {
            agent: agent.clone(),
            status: AgentStatus::Unavailable {
                reason: "agent was removed".into(),
            },
        });
        Ok(())
    }

    async fn restart_agent(&self, agent: &AgentId, config: AgentConfig) -> Result<(), AgentError> {
        // Best-effort removal: a not-yet-running agent is fine to (re)start.
        let _ = self.remove_agent(agent).await;
        self.add_agent(agent.clone(), config).await
    }

    async fn set_proxy(&self, proxy: ProxyConfig) -> Result<(), AgentError> {
        // Takes effect for agents started afterward; running agents keep theirs
        // until restarted.
        *self.proxy.lock().expect("proxy poisoned") = proxy;
        Ok(())
    }
}
