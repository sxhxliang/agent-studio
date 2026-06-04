//! Domain events.
//!
//! There are two distinct layers:
//! - [`SessionEvent`] is the semantic stream *within* a session. It is what the
//!   [`SessionRepository`](crate::ports::SessionRepository) persists and what
//!   the UI renders as a conversation timeline.
//! - [`DomainEvent`] is a cross-cutting notification carried on the event bus so
//!   adapters and the UI can react without calling each other directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentStatus, StopReason};
use crate::id::{AgentId, PermissionId, SessionId};
use crate::message::ContentBlock;
use crate::plan::Plan;
use crate::session::SessionStatus;
use crate::tool_call::ToolCall;

/// One semantic event in a session's timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionEvent {
    UserMessage { content: Vec<ContentBlock> },
    AgentMessage { content: Vec<ContentBlock> },
    AgentThought { text: String },
    ToolCall(ToolCall),
    Plan(Plan),
    Stopped { reason: StopReason },
}

/// A [`SessionEvent`] stamped with the time it occurred — the unit the
/// repository persists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedEvent {
    pub at: DateTime<Utc>,
    pub event: SessionEvent,
}

impl PersistedEvent {
    pub fn new(event: SessionEvent, at: DateTime<Utc>) -> Self {
        Self { at, event }
    }
}

/// A cross-cutting notification published on the event bus.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DomainEvent {
    AgentStatusChanged {
        agent: AgentId,
        status: AgentStatus,
    },
    SessionStatusChanged {
        session: SessionId,
        status: SessionStatus,
    },
    SessionAppended {
        session: SessionId,
        event: SessionEvent,
    },
    PermissionRequested {
        permission: PermissionId,
        session: SessionId,
    },
    ConfigChanged,
}
