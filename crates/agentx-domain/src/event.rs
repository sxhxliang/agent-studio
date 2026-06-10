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
use crate::id::{AgentId, SessionId};
use crate::message::ContentBlock;
use crate::permission::PermissionRequest;
use crate::plan::Plan;
use crate::session::{SessionConfigOption, SessionStatus, SlashCommand};
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
    /// An agent is asking the user to allow or reject a tool call. Carries the
    /// full request so the UI can render the options without a second lookup;
    /// the user's decision goes back via
    /// [`AgentGateway::resolve_permission`](crate::ports::AgentGateway::resolve_permission).
    PermissionRequested {
        request: PermissionRequest,
    },
    /// The agent updated the slash commands available in a session. Delivered as
    /// a notification mid-session (not in the session-creation response), so it
    /// is its own event rather than part of [`SessionInit`](crate::session::SessionInit).
    SessionCommandsChanged {
        session: SessionId,
        commands: Vec<SlashCommand>,
    },
    /// The agent updated the session's config options (model/mode/…), either in
    /// response to a `set_config_option` or unilaterally mid-session. Carries the
    /// complete current set so the UI can refresh its selectors.
    SessionConfigChanged {
        session: SessionId,
        options: Vec<SessionConfigOption>,
    },
    ConfigChanged,
}
