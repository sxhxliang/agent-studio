//! Agent Service - Manages agents and their sessions
//!
//! This service acts as a facade for agent operations and session management.
//! It follows the Aggregate Root pattern where Agent is the aggregate root
//! and Session is a child entity.

use std::{
    collections::{HashMap, hash_map::Entry},
    sync::{Arc, RwLock},
    time::Duration,
};

use agent_client_protocol::{self as acp, AvailableCommand, PromptResponse};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::agent::{AgentHandle, AgentManager};
use crate::core::event_bus::workspace_bus::{WorkspaceUpdateBusContainer, WorkspaceUpdateEvent};

/// Agent service - manages agents and their sessions
pub struct AgentService {
    agent_manager: Arc<AgentManager>,
    /// Stores agent -> (session_id -> session_info) mapping (multiple sessions per agent)
    sessions: Arc<RwLock<HashMap<String, HashMap<String, AgentSessionInfo>>>>,
    /// Workspace event bus for publishing status updates
    workspace_bus: Option<WorkspaceUpdateBusContainer>,
}

/// Agent session information
#[derive(Clone, Debug)]
pub struct AgentSessionInfo {
    pub session_id: String,
    pub agent_name: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub status: SessionStatus,
    /// Session metadata returned on creation (modes/models/etc.)
    pub new_session_response: Option<acp::NewSessionResponse>,
    /// Available commands for this session (slash commands, etc.)
    pub available_commands: Vec<AvailableCommand>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    #[default]
    Active,
    Idle,
    InProgress,
    Pending,
    Completed,
    Closed,
    Failed,
}

impl AgentService {
    pub fn new(agent_manager: Arc<AgentManager>) -> Self {
        Self {
            agent_manager,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            workspace_bus: None,
        }
    }

    /// Set the workspace event bus for publishing status updates
    pub fn set_workspace_bus(&mut self, bus: WorkspaceUpdateBusContainer) {
        log::info!("AgentService: Setting workspace event bus");
        self.workspace_bus = Some(bus);
    }

    // ========== Agent Operations ==========

    /// List all available agents
    pub async fn list_agents(&self) -> Vec<String> {
        self.agent_manager.list_agents().await
    }

    /// Get the initialize response for a specific agent
    pub async fn get_agent_init_response(
        &self,
        agent_name: &str,
    ) -> Option<acp::InitializeResponse> {
        self.agent_manager.get_agent_init_response(agent_name).await
    }

    /// Get all agents with their initialize responses
    pub async fn list_agents_with_info(&self) -> Vec<(String, Option<acp::InitializeResponse>)> {
        self.agent_manager.list_agents_with_info().await
    }

    /// Get agent handle (internal use)
    async fn get_agent_handle(&self, name: &str) -> Result<Arc<AgentHandle>> {
        self.agent_manager
            .get(name)
            .await
            .ok_or_else(|| anyhow!("Agent not found: {}", name))
    }

    // ========== Session Operations ==========

    /// Create a new session for the agent
    pub async fn create_session(&self, agent_name: &str) -> Result<String> {
        self.create_session_with_mcp(agent_name, Vec::new()).await
    }

    /// Create a new session with MCP servers configured
    pub async fn create_session_with_mcp(
        &self,
        agent_name: &str,
        mcp_servers: Vec<acp::McpServer>,
    ) -> Result<String> {
        self.create_session_with_mcp_and_cwd(
            agent_name,
            mcp_servers,
            std::env::current_dir().unwrap_or_default(),
        )
        .await
    }

    /// Create a new session with MCP servers and custom working directory
    pub async fn create_session_with_mcp_and_cwd(
        &self,
        agent_name: &str,
        mcp_servers: Vec<acp::McpServer>,
        cwd: std::path::PathBuf,
    ) -> Result<String> {
        let agent_handle = self.get_agent_handle(agent_name).await?;

        let mut request = acp::NewSessionRequest::new(cwd.clone());
        request.cwd = cwd;
        request.mcp_servers = mcp_servers;
        request.meta = None;

        let new_session_response: acp::NewSessionResponse = agent_handle
            .new_session(request)
            .await
            .map_err(|e| anyhow!("Failed to create session: {}", e))?;

        let session_id = new_session_response.session_id.to_string();

        let now = Utc::now();

        // Insert into nested HashMap structure
        let mut sessions = self.sessions.write().unwrap();
        let agent_sessions = sessions
            .entry(agent_name.to_string())
            .or_insert_with(HashMap::new);

        match agent_sessions.entry(session_id.clone()) {
            Entry::Occupied(mut entry) => {
                let info = entry.get_mut();
                info.agent_name = agent_name.to_string();
                info.created_at = now;
                info.last_active = now;
                info.status = SessionStatus::Active;
                info.new_session_response = Some(new_session_response);
                log::info!(
                    "Session {} for agent {} already exists; refreshed metadata",
                    session_id,
                    agent_name
                );
            }
            Entry::Vacant(entry) => {
                entry.insert(AgentSessionInfo {
                    session_id: session_id.clone(),
                    agent_name: agent_name.to_string(),
                    created_at: now,
                    last_active: now,
                    status: SessionStatus::Active,
                    new_session_response: Some(new_session_response),
                    available_commands: Vec::new(), // Will be populated by AvailableCommandsUpdate
                });
                log::info!("Created session {} for agent {}", session_id, agent_name);
            }
        }
        Ok(session_id)
    }

    /// Resume an existing session with specified session_id
    pub async fn resume_session(&self, agent_name: &str, session_id: &str) -> Result<String> {
        self.resume_session_with_mcp(agent_name, session_id, Vec::new())
            .await
    }

    /// Resume an existing session with MCP servers configured
    pub async fn resume_session_with_mcp(
        &self,
        agent_name: &str,
        session_id: &str,
        mcp_servers: Vec<acp::McpServer>,
    ) -> Result<String> {
        self.resume_session_with_mcp_and_cwd(
            agent_name,
            session_id,
            mcp_servers,
            std::env::current_dir().unwrap_or_default(),
        )
        .await
    }

    /// Resume an existing session with MCP servers and custom working directory
    pub async fn resume_session_with_mcp_and_cwd(
        &self,
        agent_name: &str,
        session_id: &str,
        mcp_servers: Vec<acp::McpServer>,
        cwd: std::path::PathBuf,
    ) -> Result<String> {
        let agent_handle = self.get_agent_handle(agent_name).await?;

        let mut request = acp::ResumeSessionRequest::new(
            acp::SessionId::from(session_id.to_string()),
            cwd.clone(),
        );
        request.cwd = cwd;
        request.mcp_servers = mcp_servers;
        request.meta = None;

        let resume_session_response: acp::ResumeSessionResponse = agent_handle
            .resume_session(request)
            .await
            .map_err(|e| anyhow!("Failed to resume session: {}", e))?;

        // Convert ResumeSessionResponse to NewSessionResponse for consistency
        let new_session_response = acp::NewSessionResponse::new(session_id.to_string())
            .config_options(resume_session_response.config_options)
            .models(resume_session_response.models)
            .modes(resume_session_response.modes)
            .meta(resume_session_response.meta);

        let now = Utc::now();

        // Insert into nested HashMap structure
        let mut sessions = self.sessions.write().unwrap();
        let agent_sessions = sessions
            .entry(agent_name.to_string())
            .or_insert_with(HashMap::new);

        match agent_sessions.entry(session_id.to_string()) {
            Entry::Occupied(mut entry) => {
                let info = entry.get_mut();
                info.agent_name = agent_name.to_string();
                info.last_active = now;
                info.status = SessionStatus::Active;
                info.new_session_response = Some(new_session_response);
                log::info!("Resumed session {} for agent {}", session_id, agent_name);
            }
            Entry::Vacant(entry) => {
                entry.insert(AgentSessionInfo {
                    session_id: session_id.to_string(),
                    agent_name: agent_name.to_string(),
                    created_at: now,
                    last_active: now,
                    status: SessionStatus::Active,
                    new_session_response: Some(new_session_response),
                    available_commands: Vec::new(),
                });
                log::info!(
                    "Resumed session {} for agent {} (created new entry)",
                    session_id,
                    agent_name
                );
            }
        }
        Ok(session_id.to_string())
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

    /// Cancel an ongoing session operation
    pub async fn cancel_session(&self, agent_name: &str, session_id: &str) -> Result<()> {
        log::info!(
            "AgentService: cancel_session called for agent={}, session={}",
            agent_name,
            session_id
        );

        // Get the agent handle
        let agent_handle = self.get_agent_handle(agent_name).await?;
        log::info!("AgentService: Got agent handle for {}", agent_name);

        // Send cancel request to the agent
        agent_handle.cancel(session_id.to_string()).await?;
        log::info!("AgentService: Sent cancel request to agent");

        // Update session status to Idle
        let mut sessions = self.sessions.write().unwrap();
        if let Some(agent_sessions) = sessions.get_mut(agent_name) {
            if let Some(info) = agent_sessions.get_mut(session_id) {
                info.status = SessionStatus::Idle;
                log::info!(
                    "AgentService: Updated session status to Idle for {} (agent: {})",
                    session_id,
                    agent_name
                );

                // Publish status update to workspace bus
                if let Some(ref workspace_bus) = self.workspace_bus {
                    let event = WorkspaceUpdateEvent::SessionStatusUpdated {
                        session_id: session_id.to_string(),
                        agent_name: agent_name.to_string(),
                        status: SessionStatus::Idle,
                        last_active: info.last_active,
                        message_count: 0,
                    };
                    workspace_bus.publish(event);
                    log::info!("AgentService: Published session status update to workspace bus");
                }
            } else {
                log::warn!(
                    "AgentService: Session {} not found in agent {}",
                    session_id,
                    agent_name
                );
            }
        } else {
            log::warn!("AgentService: No sessions found for agent {}", agent_name);
        }

        Ok(())
    }

    /// Cancel a session by ID without requiring the caller to know the agent name
    pub async fn cancel_session_by_id(&self, session_id: &str) -> Result<()> {
        let agent_name = self
            .get_agent_for_session(session_id)
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

        self.cancel_session(&agent_name, session_id).await
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

    /// Update session's available commands
    pub fn update_session_commands(
        &self,
        agent_name: &str,
        session_id: &str,
        commands: Vec<AvailableCommand>,
    ) {
        let now = Utc::now();
        let command_count = commands.len();
        let mut sessions = self.sessions.write().unwrap();
        let agent_sessions = sessions
            .entry(agent_name.to_string())
            .or_insert_with(HashMap::new);
        log::info!(
            "Updating available commands for {}:{} - {} commands",
            agent_name,
            session_id,
            command_count
        );

        match agent_sessions.entry(session_id.to_string()) {
            Entry::Occupied(mut entry) => {
                let info = entry.get_mut();
                info.available_commands = commands;
                info.last_active = now;
            }
            Entry::Vacant(entry) => {
                entry.insert(AgentSessionInfo {
                    session_id: session_id.to_string(),
                    agent_name: agent_name.to_string(),
                    created_at: now,
                    last_active: now,
                    status: SessionStatus::Active,
                    new_session_response: None,
                    available_commands: commands,
                });
            }
        }
    }

    /// Get available commands for a session
    pub fn get_session_commands(
        &self,
        agent_name: &str,
        session_id: &str,
    ) -> Option<Vec<AvailableCommand>> {
        self.sessions
            .read()
            .unwrap()
            .get(agent_name)?
            .get(session_id)
            .map(|info| info.available_commands.clone())
    }
    pub fn update_session_status(&self, agent_name: &str, session_id: &str, status: SessionStatus) {
        if let Some(agent_sessions) = self.sessions.write().unwrap().get_mut(agent_name) {
            if let Some(info) = agent_sessions.get_mut(session_id) {
                log::info!(
                    "Updating session status for {}:{} to {:?}",
                    agent_name,
                    session_id,
                    &status
                );
                info.status = status.clone();

                // Publish status update to workspace bus
                if let Some(ref workspace_bus) = self.workspace_bus {
                    let event = WorkspaceUpdateEvent::SessionStatusUpdated {
                        session_id: session_id.to_string(),
                        agent_name: agent_name.to_string(),
                        status,
                        last_active: info.last_active,
                        message_count: 0, // TODO: Track actual message count
                    };
                    workspace_bus.publish(event);
                    log::debug!("Published session status update to workspace bus");
                }
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
    ) -> Result<PromptResponse> {
        let agent_handle = self.get_agent_handle(agent_name).await?;
        self.update_session_status(agent_name, session_id, SessionStatus::InProgress);
        let request = acp::PromptRequest::new(acp::SessionId::from(session_id.to_string()), prompt);

        let result = agent_handle
            .prompt(request)
            .await
            .map_err(|e| anyhow!("Failed to send prompt: {}", e))?;

        self.update_session_status(agent_name, session_id, SessionStatus::Completed);
        // Update activity time
        self.update_session_activity(agent_name, session_id);

        log::debug!("Sent prompt to agent {} session {}", agent_name, session_id);
        Ok(result)
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
