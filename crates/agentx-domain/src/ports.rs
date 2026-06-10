//! Ports — the contracts the application depends on, implemented by adapters.
//!
//! The application layer ([`agentx-app`]) depends on these traits, never on a
//! concrete adapter. The composition root injects implementations at startup.
//! Each trait is `#[async_trait]` so it stays object-safe and can be held as
//! `Arc<dyn Port>` for dependency injection.

use std::path::Path;

use async_trait::async_trait;

use crate::agent::{AgentDescriptor, AgentStatus};
use crate::config::{AgentConfig, Config, McpServerConfig, ProxyConfig};
use crate::error::{AgentError, StoreError};
use crate::event::PersistedEvent;
use crate::id::{AgentId, SessionId};
use crate::message::ContentBlock;
use crate::permission::PermissionOutcome;
use crate::session::SessionInit;
use crate::agent::StopReason;

/// Talks to agents: session lifecycle, prompts, and turn control.
#[async_trait]
pub trait AgentGateway: Send + Sync {
    /// Current status of an agent. Always answerable — a starting agent reports
    /// [`AgentStatus::Connecting`] rather than being absent.
    fn status(&self, agent: &AgentId) -> AgentStatus;

    async fn create_session(
        &self,
        agent: &AgentId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError>;

    async fn resume_session(
        &self,
        agent: &AgentId,
        session: &SessionId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError>;

    async fn load_session(
        &self,
        agent: &AgentId,
        session: &SessionId,
        cwd: &Path,
        mcp_servers: &[McpServerConfig],
    ) -> Result<SessionInit, AgentError>;

    /// Send a user prompt. Streamed output is published as
    /// [`DomainEvent::SessionAppended`](crate::event::DomainEvent); this call
    /// resolves with the reason the turn stopped.
    async fn prompt(
        &self,
        session: &SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<StopReason, AgentError>;

    async fn cancel(&self, session: &SessionId) -> Result<(), AgentError>;

    async fn set_mode(&self, session: &SessionId, mode_id: &str) -> Result<(), AgentError>;

    /// Change one of the session's config options (model/mode/thought-level/…)
    /// by id. The agent's updated option set is published as
    /// [`DomainEvent::SessionConfigChanged`](crate::event::DomainEvent).
    async fn set_config_option(
        &self,
        session: &SessionId,
        config_id: &str,
        value: &str,
    ) -> Result<(), AgentError>;

    async fn resolve_permission(
        &self,
        session: &SessionId,
        permission_id: &str,
        outcome: PermissionOutcome,
    ) -> Result<(), AgentError>;
}

/// Manages the set of running agents and process-level configuration.
#[async_trait]
pub trait AgentRegistry: Send + Sync {
    fn agents(&self) -> Vec<AgentDescriptor>;
    async fn add_agent(&self, agent: AgentId, config: AgentConfig) -> Result<(), AgentError>;
    async fn remove_agent(&self, agent: &AgentId) -> Result<(), AgentError>;
    async fn restart_agent(&self, agent: &AgentId, config: AgentConfig) -> Result<(), AgentError>;
    async fn set_proxy(&self, proxy: ProxyConfig) -> Result<(), AgentError>;
}

/// Persists session timelines as an append-only stream.
#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn append(&self, session: &SessionId, event: PersistedEvent) -> Result<(), StoreError>;
    async fn load(&self, session: &SessionId) -> Result<Vec<PersistedEvent>, StoreError>;
    async fn delete(&self, session: &SessionId) -> Result<(), StoreError>;
    async fn list(&self) -> Result<Vec<SessionId>, StoreError>;
    async fn exists(&self, session: &SessionId) -> bool;
    async fn flush(&self, session: &SessionId) -> Result<(), StoreError>;
}

/// Loads and saves application configuration.
#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn load(&self) -> Result<Config, StoreError>;
    async fn save(&self, config: &Config) -> Result<(), StoreError>;
}
