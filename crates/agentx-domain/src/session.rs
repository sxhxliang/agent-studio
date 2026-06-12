//! Sessions and their lifecycle state machine.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{AgentId, SessionId};

/// Lifecycle state of a session.
///
/// Replaces the legacy seven-state mix (`Active`/`Idle`/`InProgress`/`Pending`/
/// ...) with a minimal set whose transitions are explicit and tested.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Created, but no prompt has been sent yet.
    #[default]
    Pending,
    /// The agent is actively processing a prompt.
    Running,
    /// Waiting for the user; the agent is ready for the next prompt.
    Idle,
    /// The work finished successfully.
    Completed,
    /// The session ended with an error.
    Failed,
    /// The session was closed and can no longer be used (terminal).
    Closed,
}

impl SessionStatus {
    /// Whether a transition to `next` is allowed by the lifecycle rules.
    pub fn can_transition_to(self, next: SessionStatus) -> bool {
        use SessionStatus::*;
        match (self, next) {
            // Idempotent transitions are always fine.
            (a, b) if a == b => true,
            // `Closed` is terminal.
            (Closed, _) => false,
            // Anything that is not already closed may be closed.
            (_, Closed) => true,
            // A turn starts.
            (Pending | Idle | Completed | Failed, Running) => true,
            // A turn ends.
            (Running, Idle | Completed | Failed) => true,
            _ => false,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, SessionStatus::Closed)
    }

    pub fn is_busy(self) -> bool {
        matches!(self, SessionStatus::Running)
    }
}

/// A conversation session bound to exactly one agent and one working directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub agent: AgentId,
    pub cwd: PathBuf,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub message_count: usize,
}

impl Session {
    /// Create a new session in the [`SessionStatus::Pending`] state. The caller
    /// supplies `now` so the domain stays pure and tests stay deterministic.
    pub fn new(id: SessionId, agent: AgentId, cwd: PathBuf, now: DateTime<Utc>) -> Self {
        Self {
            id,
            agent,
            cwd,
            status: SessionStatus::Pending,
            created_at: now,
            last_active: now,
            message_count: 0,
        }
    }

    /// Attempt a status transition, refreshing the activity timestamp on success.
    pub fn transition(
        &mut self,
        next: SessionStatus,
        now: DateTime<Utc>,
    ) -> Result<(), InvalidTransition> {
        if self.status.can_transition_to(next) {
            self.status = next;
            self.last_active = now;
            Ok(())
        } else {
            Err(InvalidTransition {
                from: self.status,
                to: next,
            })
        }
    }
}

/// Returned when a [`SessionStatus`] transition would violate the lifecycle rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid session status transition: {from:?} -> {to:?}")]
pub struct InvalidTransition {
    pub from: SessionStatus,
    pub to: SessionStatus,
}

/// A mode advertised by the agent for a session (e.g. "ask", "code", "plan").
/// Legacy mechanism; superseded by [`SessionConfigOption`] but kept as a
/// fallback for agents that only advertise modes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMode {
    pub id: String,
    pub name: String,
}

/// A configuration option the agent advertises for a session — a single-select
/// over [`values`](Self::values) with a [`current_value`](Self::current_value).
/// Categories like `"model"`, `"mode"`, `"thought_level"` are UX hints only.
/// This is ACP's unified mechanism (it supersedes session modes); the user
/// changes it via [`AgentGateway::set_config_option`](crate::ports::AgentGateway::set_config_option).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConfigOption {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub current_value: String,
    pub values: Vec<ConfigOptionValue>,
}

/// One selectable value of a [`SessionConfigOption`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOptionValue {
    pub value: String,
    pub name: String,
}

/// A slash command advertised by the agent for a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
}

/// Everything the agent gateway returns when a session is created, resumed, or
/// loaded: the id plus the capabilities the UI offers.
///
/// `config_options` is the modern, unified selector list (model / mode /
/// thought-level / …). `modes` is the legacy mode list, populated only as a
/// fallback for agents that advertise modes but not config options; the UI
/// prefers `config_options` when present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInit {
    pub session_id: SessionId,
    pub config_options: Vec<SessionConfigOption>,
    pub modes: Vec<SessionMode>,
    pub current_mode: Option<String>,
    pub commands: Vec<SlashCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    #[test]
    fn pending_session_can_start_a_turn() {
        assert!(SessionStatus::Pending.can_transition_to(SessionStatus::Running));
    }

    #[test]
    fn closed_is_terminal() {
        assert!(SessionStatus::Closed.is_terminal());
        assert!(!SessionStatus::Closed.can_transition_to(SessionStatus::Running));
    }

    #[test]
    fn a_turn_in_progress_cannot_jump_back_to_pending() {
        assert!(!SessionStatus::Running.can_transition_to(SessionStatus::Pending));
    }

    #[test]
    fn any_non_closed_state_can_close() {
        for status in [
            SessionStatus::Pending,
            SessionStatus::Running,
            SessionStatus::Idle,
            SessionStatus::Completed,
            SessionStatus::Failed,
        ] {
            assert!(status.can_transition_to(SessionStatus::Closed));
        }
    }

    #[test]
    fn transition_refreshes_activity_and_rejects_invalid() {
        let mut session = Session::new(
            SessionId::from("s1"),
            AgentId::from("claude"),
            PathBuf::from("."),
            at(0),
        );

        session.transition(SessionStatus::Running, at(5)).unwrap();
        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.last_active, at(5));

        let err = session
            .transition(SessionStatus::Pending, at(9))
            .unwrap_err();
        assert_eq!(err.from, SessionStatus::Running);
        assert_eq!(err.to, SessionStatus::Pending);
        // A rejected transition leaves the session untouched.
        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.last_active, at(5));
    }
}
