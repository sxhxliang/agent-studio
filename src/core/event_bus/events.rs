use std::sync::Arc;

use agent_client_protocol as acp;
use agent_client_protocol::SessionUpdate;
use chrono::{DateTime, Utc};

use crate::app::actions::AddCodeSelection;
use crate::core::config::{
    AgentProcessConfig, CommandConfig, Config, McpServerConfig, ModelConfig,
};
use crate::core::services::SessionStatus;

/// Events published when agent configuration changes
#[derive(Clone, Debug)]
pub enum AgentConfigEvent {
    // ========== Agent Events ==========
    /// A new agent was added
    AgentAdded {
        name: String,
        config: AgentProcessConfig,
    },
    /// An existing agent's configuration was updated
    AgentUpdated {
        name: String,
        config: AgentProcessConfig,
    },
    /// An agent was removed
    AgentRemoved { name: String },

    // ========== Model Events ==========
    /// A new model was added
    ModelAdded { name: String, config: ModelConfig },
    /// An existing model's configuration was updated
    ModelUpdated { name: String, config: ModelConfig },
    /// A model was removed
    ModelRemoved { name: String },

    // ========== MCP Server Events ==========
    /// A new MCP server was added
    McpServerAdded {
        name: String,
        config: McpServerConfig,
    },
    /// An existing MCP server's configuration was updated
    McpServerUpdated {
        name: String,
        config: McpServerConfig,
    },
    /// An MCP server was removed
    McpServerRemoved { name: String },

    // ========== Command Events ==========
    /// A new command was added
    CommandAdded { name: String, config: CommandConfig },
    /// An existing command's configuration was updated
    CommandUpdated { name: String, config: CommandConfig },
    /// A command was removed
    CommandRemoved { name: String },

    // ========== Full Reload ==========
    /// The entire configuration was reloaded from file
    ConfigReloaded { config: Box<Config> },
}

/// Session update event that can be broadcast to subscribers
#[derive(Clone, Debug)]
pub struct SessionUpdateEvent {
    pub session_id: String,
    pub agent_name: Option<String>,
    pub update: Arc<SessionUpdate>,
}

/// Permission request event that can be broadcast to subscribers
#[derive(Clone, Debug)]
pub struct PermissionRequestEvent {
    /// Unique permission request ID from PermissionStore
    pub permission_id: String,
    /// Session ID for this permission request
    pub session_id: String,
    /// Agent name requesting permission
    pub agent_name: String,
    /// Tool call details
    pub tool_call: acp::ToolCallUpdate,
    /// Available permission options
    pub options: Vec<acp::PermissionOption>,
}

/// Workspace update events
#[derive(Clone, Debug)]
pub enum WorkspaceUpdateEvent {
    /// A new task was created
    TaskCreated {
        workspace_id: String,
        task_id: String,
    },
    /// A task was updated
    TaskUpdated { task_id: String },
    /// A task was removed
    TaskRemoved {
        workspace_id: String,
        task_id: String,
    },
    /// A new workspace was added
    WorkspaceAdded { workspace_id: String },
    /// A workspace was removed
    WorkspaceRemoved { workspace_id: String },
    /// A session status was updated
    SessionStatusUpdated {
        session_id: String,
        agent_name: String,
        status: SessionStatus,
        last_active: DateTime<Utc>,
        message_count: usize,
    },
}

/// Event published when code is selected in the editor
#[derive(Clone, Debug)]
pub struct CodeSelectionEvent {
    pub selection: AddCodeSelection,
}
