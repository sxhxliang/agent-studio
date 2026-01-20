use agent_client_protocol::{ContentChunk, Plan, SessionUpdate, ToolCall, ToolCallUpdate};
use gpui::{App, AppContext, Context, Entity};
/// Optimized state manager for ConversationPanel updates
///
/// This module provides fast O(1) lookups for ToolCall and message updates
/// using HashMap indices, avoiding expensive O(n) linear searches.
use std::collections::HashMap;

use super::components::{AgentThoughtItemState, ResourceItemState, UserMessageView};
use super::helpers::{extract_text_from_content, session_update_type_name};
use super::rendered_item::{RenderedItem, create_agent_message_data};
use super::types::ResourceInfo;
use crate::components::ToolCallItem;
use crate::{AppState, UserMessageData};

/// Fast index for locating items in the rendered list
#[derive(Default)]
pub struct UpdateStateIndex {
    /// Maps tool_call_id -> index in rendered_items
    tool_call_positions: HashMap<String, usize>,
    /// Track the index of the last message item (for fast appending)
    last_message_index: Option<usize>,
    /// Track the index of the last thought item (for fast appending)
    last_thought_index: Option<usize>,
}

impl UpdateStateIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new ToolCall at the given index
    pub fn register_tool_call(&mut self, tool_call_id: String, index: usize) {
        self.tool_call_positions.insert(tool_call_id, index);
    }

    /// Update a ToolCall's position after list modifications
    pub fn update_tool_call_position(&mut self, tool_call_id: &str, new_index: usize) {
        self.tool_call_positions
            .insert(tool_call_id.to_string(), new_index);
    }

    /// Find ToolCall index by ID (O(1) lookup)
    pub fn find_tool_call(&self, tool_call_id: &str) -> Option<usize> {
        self.tool_call_positions.get(tool_call_id).copied()
    }

    /// Remove a ToolCall from index
    pub fn remove_tool_call(&mut self, tool_call_id: &str) {
        self.tool_call_positions.remove(tool_call_id);
    }

    /// Set the index of the last message
    pub fn set_last_message(&mut self, index: usize) {
        self.last_message_index = Some(index);
    }

    /// Get the index of the last message
    pub fn last_message(&self) -> Option<usize> {
        self.last_message_index
    }

    /// Set the index of the last thought
    pub fn set_last_thought(&mut self, index: usize) {
        self.last_thought_index = Some(index);
    }

    /// Get the index of the last thought
    pub fn last_thought(&self) -> Option<usize> {
        self.last_thought_index
    }

    /// Clear the last message/thought tracking when type changes
    pub fn clear_streaming_state(&mut self) {
        self.last_message_index = None;
        self.last_thought_index = None;
    }

    /// Rebuild index from rendered items (call after bulk operations)
    pub fn rebuild(&mut self, items: &[RenderedItem], cx: &App) {
        self.tool_call_positions.clear();
        self.last_message_index = None;
        self.last_thought_index = None;

        for (idx, item) in items.iter().enumerate() {
            match item {
                RenderedItem::ToolCall(entity) => {
                    let tool_call_id = entity.read(cx).tool_call_id().to_string();
                    self.tool_call_positions.insert(tool_call_id, idx);
                }
                RenderedItem::AgentMessage(..) => {
                    self.last_message_index = Some(idx);
                }
                RenderedItem::AgentThought(..) => {
                    self.last_thought_index = Some(idx);
                }
                _ => {}
            }
        }
    }
}

/// Optimized update processor with fast lookups
pub struct UpdateProcessor<'a, T> {
    items: &'a mut Vec<RenderedItem>,
    index: &'a mut UpdateStateIndex,
    session_id: Option<&'a str>,
    agent_name: Option<&'a str>,
    next_index: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T> UpdateProcessor<'a, T> {
    pub fn new(
        items: &'a mut Vec<RenderedItem>,
        index: &'a mut UpdateStateIndex,
        session_id: Option<&'a str>,
        agent_name: Option<&'a str>,
        next_index: usize,
    ) -> Self {
        Self {
            items,
            index,
            session_id,
            agent_name,
            next_index,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Process a SessionUpdate and add/update items
    pub fn process_update(&mut self, update: SessionUpdate, cx: &mut Context<T>) {
        let update_type = session_update_type_name(&update);
        log::debug!(
            "Processing SessionUpdate[{}]: {}",
            self.next_index,
            update_type
        );

        match update {
            SessionUpdate::UserMessageChunk(chunk) => {
                self.process_user_message_chunk(chunk, cx);
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                self.process_agent_message_chunk(chunk, cx);
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                self.process_agent_thought_chunk(chunk, cx);
            }
            SessionUpdate::ToolCall(tool_call) => {
                self.process_tool_call(tool_call, cx);
            }
            SessionUpdate::ToolCallUpdate(tool_call_update) => {
                self.process_tool_call_update(tool_call_update, cx);
            }
            SessionUpdate::Plan(plan) => {
                self.process_plan(plan);
            }
            SessionUpdate::AvailableCommandsUpdate(commands_update) => {
                self.complete_last_item();
                self.index.clear_streaming_state();
                log::debug!(
                    "  └─ Commands update: {} available",
                    commands_update.available_commands.len()
                );
                self.items.push(RenderedItem::InfoUpdate(format!(
                    "📋 Available Commands: {} commands",
                    commands_update.available_commands.len()
                )));
            }
            SessionUpdate::CurrentModeUpdate(mode_update) => {
                self.complete_last_item();
                self.index.clear_streaming_state();
                log::debug!("  └─ Mode changed to: {}", mode_update.current_mode_id);
                self.items.push(RenderedItem::InfoUpdate(format!(
                    "🔄 Mode: {}",
                    mode_update.current_mode_id
                )));
            }
            _ => {
                log::warn!(
                    "⚠️  UNHANDLED SessionUpdate type: {}\n\
                     This update will be ignored. Consider implementing support for this type.\n\
                     Update details: {:?}",
                    update_type,
                    update
                );
            }
        }
    }

    /// Process UserMessageChunk
    fn process_user_message_chunk(&mut self, chunk: ContentChunk, cx: &mut Context<T>) {
        // Mark last message as complete if it was streaming
        if self.index.last_message().is_some() || self.index.last_thought().is_some() {
            self.complete_last_item();
            self.index.clear_streaming_state();
        }

        log::debug!("  └─ Creating UserMessage");
        let item = create_user_message(chunk, self.next_index, cx);
        self.items.push(item);
    }

    /// Process AgentMessageChunk with optimized merging
    fn process_agent_message_chunk(&mut self, chunk: ContentChunk, cx: &mut Context<T>) {
        let resolved_agent_name = self.agent_name.map(str::to_string).or_else(|| {
            self.session_id.and_then(|session_id| {
                AppState::global(cx)
                    .agent_service()
                    .and_then(|service| service.get_agent_for_session(session_id))
            })
        });

        // Fast path: Try to merge with tracked last message
        if let Some(last_idx) = self.index.last_message() {
            if last_idx < self.items.len() {
                if let Some(last_item) = self.items.get_mut(last_idx) {
                    if last_item.can_accept_agent_message_chunk() {
                        if last_item.try_append_agent_message_chunk(chunk.clone()) {
                            // Update agent name if needed
                            if let (Some(name), RenderedItem::AgentMessage(_, data)) =
                                (resolved_agent_name.as_deref(), last_item)
                            {
                                if data.meta.agent_name.is_none() {
                                    data.meta.agent_name = Some(name.to_string());
                                }
                            }
                            log::debug!(
                                "  └─ Merged AgentMessageChunk into existing message (fast path)"
                            );
                            return;
                        }
                    }
                }
            }
        }

        // Slow path: Last item is not a message, complete it and create new
        if self.index.last_thought().is_some() {
            self.complete_last_item();
        }

        log::debug!("  └─ Creating new AgentMessage");
        let data =
            create_agent_message_data(chunk, self.session_id, resolved_agent_name.as_deref());
        let new_index = self.items.len();
        self.items.push(RenderedItem::AgentMessage(
            format!("agent-msg-{}", self.next_index),
            data,
        ));
        self.index.set_last_message(new_index);
        self.index.set_last_thought(new_index); // Clear thought tracking
    }

    /// Process AgentThoughtChunk with optimized merging
    fn process_agent_thought_chunk(&mut self, chunk: ContentChunk, cx: &mut Context<T>) {
        let text = extract_text_from_content(&chunk.content);

        // Fast path: Try to merge with tracked last thought
        if let Some(last_idx) = self.index.last_thought() {
            if last_idx < self.items.len() {
                if let Some(last_item) = self.items.get_mut(last_idx) {
                    if last_item.can_accept_agent_thought_chunk() {
                        if last_item.try_append_agent_thought_chunk(text.clone(), cx) {
                            log::debug!(
                                "  └─ Merged AgentThoughtChunk into existing thought (fast path)"
                            );
                            return;
                        }
                    }
                }
            }
        }

        // Slow path: Last item is not a thought, complete it and create new
        if self.index.last_message().is_some() {
            self.complete_last_item();
        }

        log::debug!("  └─ Creating new AgentThought");
        let entity = cx.new(|_| AgentThoughtItemState::new(text));
        let new_index = self.items.len();
        self.items.push(RenderedItem::AgentThought(entity));
        self.index.set_last_thought(new_index);
        self.index.set_last_message(new_index); // Clear message tracking
    }

    /// Process ToolCall with O(1) lookup
    fn process_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<T>) {
        // Fast O(1) lookup using index
        if let Some(idx) = self
            .index
            .find_tool_call(&tool_call.tool_call_id.to_string())
        {
            if idx < self.items.len() {
                if let Some(RenderedItem::ToolCall(entity)) = self.items.get_mut(idx) {
                    entity.update(cx, |state, cx| {
                        log::debug!(
                            "  └─ Updating existing ToolCall: {} (title: {:?} -> {:?}) [fast O(1) lookup]",
                            tool_call.tool_call_id,
                            state.tool_call().title,
                            tool_call.title
                        );
                        state.update_tool_call(tool_call.clone(), cx);
                    });
                    return;
                }
            }
        }

        // Not found, create new
        self.complete_last_item();
        self.index.clear_streaming_state();

        log::debug!("  └─ Creating new ToolCall: {}", tool_call.tool_call_id);
        let tool_call_id = tool_call.tool_call_id.to_string();
        let entity = cx.new(|_| ToolCallItem::new(tool_call));
        let new_index = self.items.len();
        self.items.push(RenderedItem::ToolCall(entity));
        self.index.register_tool_call(tool_call_id, new_index);
    }

    /// Process ToolCallUpdate with O(1) lookup
    fn process_tool_call_update(&mut self, tool_call_update: ToolCallUpdate, cx: &mut Context<T>) {
        log::debug!("  └─ Updating ToolCall: {}", tool_call_update.tool_call_id);

        // Fast O(1) lookup using index
        if let Some(idx) = self
            .index
            .find_tool_call(&tool_call_update.tool_call_id.to_string())
        {
            if idx < self.items.len() {
                if let Some(RenderedItem::ToolCall(entity)) = self.items.get_mut(idx) {
                    entity.update(cx, |state, cx| {
                        log::debug!(
                            "     ✓ Found and updating ToolCall {} (status: {:?}) [fast O(1) lookup]",
                            tool_call_update.tool_call_id,
                            tool_call_update.fields.status
                        );
                        state.apply_update(tool_call_update.fields.clone(), cx);
                    });
                    return;
                }
            }
        }

        // Not found, try to create from update
        log::warn!(
            "     ⚠ ToolCallUpdate for non-existent ID: {}. Attempting to create.",
            tool_call_update.tool_call_id
        );

        match ToolCall::try_from(tool_call_update) {
            Ok(tool_call) => {
                log::debug!("     ✓ Successfully created ToolCall from update");
                let tool_call_id = tool_call.tool_call_id.to_string();
                let entity = cx.new(|_| ToolCallItem::new(tool_call));
                let new_index = self.items.len();
                self.items.push(RenderedItem::ToolCall(entity));
                self.index.register_tool_call(tool_call_id, new_index);
            }
            Err(e) => {
                log::error!("     ✗ Failed to create ToolCall from update: {:?}", e);
            }
        }
    }

    /// Process Plan
    fn process_plan(&mut self, plan: Plan) {
        self.complete_last_item();
        self.index.clear_streaming_state();
        log::debug!("  └─ Creating Plan with {} entries", plan.entries.len());
        self.items.push(RenderedItem::Plan(plan));
    }

    /// Mark the last item as complete
    fn complete_last_item(&mut self) {
        if let Some(last_item) = self.items.last_mut() {
            last_item.mark_complete();
        }
    }
}

/// Create a UserMessage RenderedItem from a ContentChunk
fn create_user_message<T>(chunk: ContentChunk, _index: usize, cx: &mut Context<T>) -> RenderedItem {
    let content_vec = vec![chunk.content.clone()];
    let user_data = UserMessageData::new("default-session").with_contents(content_vec.clone());

    let entity = cx.new(|cx| {
        let data_entity = cx.new(|_| user_data);

        let resource_items: Vec<Entity<ResourceItemState>> = content_vec
            .iter()
            .filter_map(|content| ResourceInfo::from_content_block(content))
            .map(|resource_info| cx.new(|_| ResourceItemState::new(resource_info)))
            .collect();

        UserMessageView {
            data: data_entity,
            resource_items,
        }
    });

    RenderedItem::UserMessage(entity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_tool_call_operations() {
        let mut index = UpdateStateIndex::new();

        // Register tool calls
        index.register_tool_call("tc-1".to_string(), 0);
        index.register_tool_call("tc-2".to_string(), 5);

        // O(1) lookups
        assert_eq!(index.find_tool_call("tc-1"), Some(0));
        assert_eq!(index.find_tool_call("tc-2"), Some(5));
        assert_eq!(index.find_tool_call("tc-3"), None);

        // Update position
        index.update_tool_call_position("tc-1", 10);
        assert_eq!(index.find_tool_call("tc-1"), Some(10));

        // Remove
        index.remove_tool_call("tc-1");
        assert_eq!(index.find_tool_call("tc-1"), None);
    }

    #[test]
    fn test_index_streaming_state() {
        let mut index = UpdateStateIndex::new();

        // Track message
        index.set_last_message(5);
        assert_eq!(index.last_message(), Some(5));

        // Track thought
        index.set_last_thought(8);
        assert_eq!(index.last_thought(), Some(8));

        // Clear
        index.clear_streaming_state();
        assert_eq!(index.last_message(), None);
        assert_eq!(index.last_thought(), None);
    }
}
