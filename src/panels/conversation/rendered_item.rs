use agent_client_protocol::{ContentChunk, Plan};
/// RenderedItem enum and message handling logic
use gpui::{App, Entity};

use super::components::{AgentThoughtItemState, UserMessageView};
use crate::components::ToolCallItem;
use crate::{AgentMessageData, DiffSummary, PermissionRequestView};

// ============================================================================
// Rendered Item
// ============================================================================

pub enum RenderedItem {
    UserMessage(Entity<UserMessageView>),
    /// Agent message with unique ID and mutable data (supports chunk merging)
    AgentMessage(String, AgentMessageData),
    /// Agent thought with unique ID and entity (supports chunk merging and expand/collapse)
    AgentThought(Entity<AgentThoughtItemState>),
    Plan(Plan),
    ToolCall(Entity<ToolCallItem>),
    // Simple text updates for commands and mode changes
    InfoUpdate(String),
    // Permission request
    PermissionRequest(Entity<PermissionRequestView>),
    // Diff summary for file changes
    DiffSummary(Entity<DiffSummary>),
}

impl RenderedItem {
    /// Try to append an AgentMessageChunk to this item (returns true if successful)
    pub fn try_append_agent_message_chunk(&mut self, chunk: ContentChunk) -> bool {
        if let RenderedItem::AgentMessage(_id, data) = self {
            data.push_chunk(chunk);
            true
        } else {
            false
        }
    }

    /// Try to append an AgentThoughtChunk to this item (returns true if successful)
    pub fn try_append_agent_thought_chunk(&mut self, text: String, cx: &mut App) -> bool {
        if let RenderedItem::AgentThought(entity) = self {
            entity.update(cx, |state, cx| {
                state.append_text(text, cx);
            });
            true
        } else {
            false
        }
    }

    /// Mark an AgentMessage as complete (no more chunks expected)
    pub fn mark_complete(&mut self) {
        if let RenderedItem::AgentMessage(_id, data) = self {
            data.meta.is_complete = true;
        }
    }

    /// Check if this item can accept chunks of a given type
    pub fn can_accept_agent_message_chunk(&self) -> bool {
        matches!(self, RenderedItem::AgentMessage(..))
    }

    pub fn can_accept_agent_thought_chunk(&self) -> bool {
        matches!(self, RenderedItem::AgentThought(..))
    }
}

// ============================================================================
// Message Creation Functions
// ============================================================================

/// Create AgentMessageData from a ContentChunk
pub fn create_agent_message_data(
    chunk: ContentChunk,
    session_id: Option<&str>,
    agent_name: Option<&str>,
) -> AgentMessageData {
    let session_id = session_id.unwrap_or("default-session").to_string();
    let mut data = AgentMessageData::new(session_id).add_chunk(chunk);
    if let Some(agent_name) = agent_name {
        data = data.with_agent_name(agent_name);
    }
    data
}
