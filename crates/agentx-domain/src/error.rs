//! Domain error types.
//!
//! Application and UI code matches on these variants rather than parsing
//! `anyhow` strings — a fragile pattern that was pervasive in the legacy code.
//! Adapters convert their library-specific errors into these at the boundary.

use crate::id::{AgentId, SessionId};

/// Failures surfaced by the agent gateway / registry ports.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agent `{0}` is not available")]
    Unavailable(AgentId),
    #[error("session `{0}` was not found")]
    SessionNotFound(SessionId),
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("agent transport error: {0}")]
    Transport(String),
    #[error("agent protocol error: {0}")]
    Protocol(String),
}

/// Failures surfaced by the persistence / configuration ports.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("`{0}` was not found")]
    NotFound(String),
    #[error("i/o error: {0}")]
    Io(String),
    #[error("(de)serialization error: {0}")]
    Serde(String),
}
