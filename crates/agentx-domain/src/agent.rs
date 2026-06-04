//! Agents and their live connection status.

use serde::{Deserialize, Serialize};

use crate::id::AgentId;

/// Live status of an agent's subprocess connection.
///
/// This type is why the rewrite has no two-phase initialization: the agent
/// gateway exists from the first moment, and an agent that is still starting up
/// simply reports [`AgentStatus::Connecting`]. "Not ready yet" is a domain
/// state, not a missing service (`Option<Service>`) and not a null check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// The subprocess is starting up or performing the ACP handshake.
    Connecting,
    /// Ready to accept sessions and prompts.
    Ready,
    /// Failed to start or has crashed; carries a human-readable reason.
    Unavailable { reason: String },
}

impl AgentStatus {
    pub fn is_ready(&self) -> bool {
        matches!(self, AgentStatus::Ready)
    }
}

/// A configured agent together with its current status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    pub id: AgentId,
    pub status: AgentStatus,
}

/// Why an agent's prompt turn finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurns,
    Refusal,
    Cancelled,
}
