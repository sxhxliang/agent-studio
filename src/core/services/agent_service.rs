//! Agent Service - Manages agents and their sessions
//!
//! This service acts as a facade for agent operations and session management.
//! It follows the Aggregate Root pattern where Agent is the aggregate root
//! and Session is a child entity.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use agent_client_protocol as acp;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

use crate::core::agent::{AgentHandle, AgentManager};

/// Agent service - manages agents and their sessions
pub struct AgentService {
    agent_manager: Arc<AgentManager>,
    /// Stores agent -> (session_id -> session_info) mapping (multiple sessions per agent)
    sessions: Arc<RwLock<HashMap<String, HashMap<String, AgentSessionInfo>>>>,
}

/// Agent session information
#[derive(Clone, Debug)]
pub struct AgentSessionInfo {
    pub session_id: String,
    pub agent_name: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub status: SessionStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Active,
    Idle,
    Closed,
}

impl AgentService {
    pub fn new(agent_manager: Arc<AgentManager>) -> Self {
        Self {
            agent_manager,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ========== Agent Operations ==========

    /// List all available agents
    pub fn list_agents(&self) -> Vec<String> {
        self.agent_manager.list_agents()
    }

    /// Get agent handle (internal use)
    fn get_agent_handle(&self, name: &str) -> Result<Arc<AgentHandle>> {
        self.agent_manager
            .get(name)
            .ok_or_else(|| anyhow!("Agent not found: {}", name))
    }

    // ========== Session Operations ==========

    /// Create a new session for the agent
    pub async fn create_session(&self, agent_name: &str) -> Result<String> {
        let agent_handle = self.get_agent_handle(agent_name)?;

        let mut request = acp::NewSessionRequest::new(std::env::current_dir().unwrap_or_default());
        request.cwd = std::env::current_dir().unwrap_or_default();
        request.mcp_servers = vec![];
        request.meta = None;

        let session_id = agent_handle
            .new_session(request)
            .await
            .map_err(|e| anyhow!("Failed to create session: {}", e))?
            .session_id
            .to_string();

        // Store session information
        let session_info = AgentSessionInfo {
            session_id: session_id.clone(),
            agent_name: agent_name.to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            status: SessionStatus::Active,
        };

        // Insert into nested HashMap structure
        let mut sessions = self.sessions.write().unwrap();
        sessions
            .entry(agent_name.to_string())
            .or_insert_with(HashMap::new)
            .insert(session_id.clone(), session_info);

        log::info!("Created session {} for agent {}", session_id, agent_name);
        Ok(session_id)
    }

    /// Get session information
    pub fn get_session_info(&self, agent_name: &str, session_id: &str) -> Option<AgentSessionInfo> {
        self.sessions
            .read()
            .unwrap()
            .get(agent_name)?
            .get(session_id)
            .cloned()
    }

    /// Close an agent's session
    pub async fn close_session(&self, agent_name: &str, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(agent_sessions) = sessions.get_mut(agent_name) {
            if let Some(info) = agent_sessions.get_mut(session_id) {
                info.status = SessionStatus::Closed;
                log::info!("Closed session {} for agent {}", session_id, agent_name);
            }
        }
        Ok(())
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<AgentSessionInfo> {
        self.sessions
            .read()
            .unwrap()
            .values()
            .flat_map(|agent_sessions| agent_sessions.values().cloned())
            .collect()
    }

    /// Update session's last active time
    pub fn update_session_activity(&self, agent_name: &str, session_id: &str) {
        if let Some(agent_sessions) = self.sessions.write().unwrap().get_mut(agent_name) {
            if let Some(info) = agent_sessions.get_mut(session_id) {
                info.last_active = Utc::now();
            }
        }
    }

    // ========== Prompt Operations ==========

    /// Send a prompt to an agent's session
    pub async fn send_prompt(
        &self,
        agent_name: &str,
        session_id: &str,
        prompt: Vec<acp::ContentBlock>,
    ) -> Result<()> {
        let agent_handle = self.get_agent_handle(agent_name)?;

        let request = acp::PromptRequest::new(acp::SessionId::from(session_id.to_string()), prompt);

        agent_handle
            .prompt(request)
            .await
            .map_err(|e| anyhow!("Failed to send prompt: {}", e))?;

        // Update activity time
        self.update_session_activity(agent_name, session_id);

        log::debug!("Sent prompt to agent {} session {}", agent_name, session_id);
        Ok(())
    }

    // ========== Cleanup Operations ==========

    /// Clean up idle sessions
    pub async fn cleanup_idle_sessions(&self, idle_duration: Duration) {
        let now = Utc::now();
        let mut sessions = self.sessions.write().unwrap();

        for (agent_name, agent_sessions) in sessions.iter_mut() {
            agent_sessions.retain(|session_id, info| {
                let idle_time = now.signed_duration_since(info.last_active);
                let should_keep = idle_time.num_seconds() < idle_duration.as_secs() as i64;

                if !should_keep {
                    log::info!(
                        "Cleaning up idle session {} for agent {} (idle for {}s)",
                        session_id,
                        agent_name,
                        idle_time.num_seconds()
                    );
                }

                should_keep
            });
        }
    }

    // ========== Multi-Session Query Methods ==========

    /// List all sessions for a specific agent
    pub fn list_sessions_for_agent(&self, agent_name: &str) -> Vec<AgentSessionInfo> {
        self.sessions
            .read()
            .unwrap()
            .get(agent_name)
            .map(|agent_sessions| agent_sessions.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Find a session by ID across all agents
    pub fn get_session_by_id(&self, session_id: &str) -> Option<AgentSessionInfo> {
        self.sessions
            .read()
            .unwrap()
            .values()
            .flat_map(|agent_sessions| agent_sessions.values())
            .find(|info| info.session_id == session_id)
            .cloned()
    }

    /// Get the agent name for a given session ID
    pub fn get_agent_for_session(&self, session_id: &str) -> Option<String> {
        self.get_session_by_id(session_id)
            .map(|info| info.agent_name)
    }
}
