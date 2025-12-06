use agent_client_protocol::{ContentChunk, Plan};
/// RenderedItem enum and message handling logic
use gpui::Entity;

use super::components::{ToolCallItemState, UserMessageView};
use crate::{AgentMessageData, PermissionRequestView};

// ============================================================================
// Rendered Item
// ============================================================================

pub enum RenderedItem {
    UserMessage(Entity<UserMessageView>),
    /// Agent message with unique ID and mutable data (supports chunk merging)
    AgentMessage(String, AgentMessageData),
    /// Agent thought with mutable text (supports chunk merging)
    AgentThought(String),
    Plan(Plan),
    ToolCall(Entity<ToolCallItemState>),
    // Simple text updates for commands and mode changes
    InfoUpdate(String),
    // Permission request
    PermissionRequest(Entity<PermissionRequestView>),
}

impl RenderedItem {
    /// Try to append an AgentMessageChunk to this item (returns true if successful)
    pub fn try_append_agent_message_chunk(&mut self, chunk: ContentChunk) -> bool {
        if let RenderedItem::AgentMessage(_id, ref mut data) = self {
            data.chunks.push(chunk);
            true
        } else {
            false
        }
    }

    /// Try to append an AgentThoughtChunk to this item (returns true if successful)
    pub fn try_append_agent_thought_chunk(&mut self, text: String) -> bool {
        if let RenderedItem::AgentThought(ref mut existing_text) = self {
            existing_text.push_str(&text);
            true
        } else {
            false
        }
    }

    /// Mark an AgentMessage as complete (no more chunks expected)
    pub fn mark_complete(&mut self) {
        if let RenderedItem::AgentMessage(_id, ref mut data) = self {
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
pub fn create_agent_message_data(chunk: ContentChunk, _index: usize) -> AgentMessageData {
    AgentMessageData::new("default-session").add_chunk(chunk)
}
