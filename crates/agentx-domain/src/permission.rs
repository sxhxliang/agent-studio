//! Permission requests raised by an agent before performing a sensitive action.

use serde::{Deserialize, Serialize};

use crate::id::{PermissionId, SessionId};
use crate::tool_call::ToolCall;

/// What choosing a permission option does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

/// One choice presented to the user for a permission request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    pub kind: PermissionOptionKind,
}

/// A pending request for the user to allow or reject a tool call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: PermissionId,
    pub session: SessionId,
    pub tool_call: ToolCall,
    pub options: Vec<PermissionOption>,
}

/// The user's response to a [`PermissionRequest`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionOutcome {
    Selected { option_id: String },
    Cancelled,
}
