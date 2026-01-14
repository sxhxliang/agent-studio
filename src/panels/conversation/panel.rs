use gpui::{
    App, ClipboardEntry, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, ScrollHandle, SharedString, Styled, Timer, Window, div, prelude::*, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, h_flex, input::InputState, skeleton::Skeleton, spinner::Spinner,
    v_flex,
};

// Use the published ACP schema crate
use agent_client_protocol::{ContentChunk, ImageContent, SessionUpdate, ToolCall};
use chrono::{DateTime, Utc};
use rust_i18n::t;
use std::time::Duration;

use crate::components::ToolCallItem;
use crate::{
    AgentMessage, AgentTodoList, AppState, ChatInputBox, DiffSummary, DiffSummaryData,
    SendMessageToSession, app::actions::AddCodeSelection, core::services::SessionStatus,
    panels::dock_panel::DockPanel,
};

// Import from submodules
use super::{
    components::{AgentThoughtItemState, ResourceItemState, UserMessageView},
    helpers::{extract_text_from_content, get_element_id, session_update_type_name},
    rendered_item::{RenderedItem, create_agent_message_data},
    types::ResourceInfo,
};

/// Session status information for display
#[derive(Clone, Debug)]
pub struct SessionStatusInfo {
    pub agent_name: String,
    pub status: SessionStatus,
    pub last_active: DateTime<Utc>,
    pub message_count: usize,
}

/// Conversation panel that displays SessionUpdate messages from ACP
pub struct ConversationPanel {
    focus_handle: FocusHandle,
    /// List of rendered items
    rendered_items: Vec<RenderedItem>,
    /// Counter for generating unique IDs for new items
    next_index: usize,
    /// Optional session ID to filter updates (None = all sessions)
    session_id: Option<String>,
    /// Scroll handle for auto-scrolling to bottom
    scroll_handle: ScrollHandle,
    /// Input state for the chat input box
    input_state: Entity<InputState>,
    /// List of pasted images: (ImageContent, filename)
    pasted_images: Vec<(ImageContent, String)>,
    /// List of code selections from editor
    code_selections: Vec<AddCodeSelection>,
    /// Session status information for display
    session_status: Option<SessionStatusInfo>,
    /// Workspace information
    workspace_id: Option<String>,
    workspace_name: Option<String>,
    working_directory: Option<String>,
}

impl ConversationPanel {
    /// Create a new panel with mock data (for demo purposes)
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        log::info!("🚀 Creating ConversationPanel view");
        let entity = cx.new(|cx| Self::new(window, cx));
        Self::subscribe_to_updates(&entity, None, cx);
        Self::subscribe_to_permissions(&entity, None, cx);
        Self::subscribe_to_code_selections(&entity, cx);
        log::info!("✅ ConversationPanel view created and subscribed");
        entity
    }

    /// Create a new panel for a specific session (no mock data)
    pub fn view_for_session(session_id: String, window: &mut Window, cx: &mut App) -> Entity<Self> {
        log::info!("🚀 Creating ConversationPanel for session: {}", session_id);
        let entity = cx.new(|cx| Self::new_for_session(session_id.clone(), window, cx));

        // Load historical messages before subscribing to new updates
        Self::load_history_for_session(&entity, session_id.clone(), cx);

        Self::subscribe_to_updates(&entity, Some(session_id.clone()), cx);
        Self::subscribe_to_permissions(&entity, Some(session_id.clone()), cx);
        Self::subscribe_to_code_selections(&entity, cx);
        Self::subscribe_to_status_updates(&entity, Some(session_id.clone()), cx);
        log::info!("✅ ConversationPanel created for session: {}", session_id);
        entity
    }

    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    /// Get the workspace_id (if available)
    pub fn workspace_id(&self) -> Option<String> {
        self.workspace_id.clone()
    }

    /// Get the workspace_name (if available)
    pub fn workspace_name(&self) -> Option<String> {
        self.workspace_name.clone()
    }

    /// Get the working_directory (if available)
    pub fn working_directory(&self) -> Option<String> {
        self.working_directory.clone()
    }

    fn new(window: &mut Window, cx: &mut App) -> Self {
        log::info!("🔧 Initializing ConversationPanel (new)");
        Self::new_internal(None, window, cx)
    }

    fn new_for_session(session_id: String, window: &mut Window, cx: &mut App) -> Self {
        log::info!(
            "🔧 Initializing ConversationPanel for session: {}",
            session_id
        );
        Self::new_internal(Some(session_id), window, cx)
    }

    fn new_internal(session_id: Option<String>, window: &mut Window, cx: &mut App) -> Self {
        let focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();
        let input_state = Self::create_input_state(window, cx);
        let rendered_items = Vec::new();
        let next_index = rendered_items.len();

        Self {
            focus_handle,
            rendered_items,
            next_index,
            session_id,
            scroll_handle,
            input_state,
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            session_status: None,
            workspace_id: None,
            workspace_name: None,
            working_directory: None,
        }
    }

    fn create_input_state(window: &mut Window, cx: &mut App) -> Entity<InputState> {
        cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(1, 3)
                .soft_wrap(true)
                .placeholder("Type a message...")
        })
    }

    /// Load historical messages for a session
    pub fn load_history_for_session(entity: &Entity<Self>, session_id: String, cx: &mut App) {
        Self::load_history_for_session_with_retry(entity.clone(), session_id, 20, cx);
    }

    fn load_history_for_session_with_retry(
        entity: Entity<Self>,
        session_id: String,
        remaining_attempts: usize,
        cx: &mut App,
    ) {
        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                if remaining_attempts == 0 {
                    log::error!("MessageService not initialized, cannot load history");
                    return;
                }

                let weak_entity = entity.downgrade();
                cx.spawn(async move |cx| {
                    Timer::after(Duration::from_millis(500)).await;
                    let _ = cx.update(|cx| {
                        if let Some(entity) = weak_entity.upgrade() {
                            Self::load_history_for_session_with_retry(
                                entity,
                                session_id,
                                remaining_attempts - 1,
                                cx,
                            );
                        }
                    });
                })
                .detach();
                return;
            }
        };

        let weak_entity = entity.downgrade();

        log::info!("Loading history for session: {}", session_id);

        cx.spawn(
            async move |cx| match message_service.load_history(&session_id).await {
                Ok(messages) => {
                    log::info!(
                        "Loaded {} historical messages for session: {}",
                        messages.len(),
                        session_id
                    );

                    let weak = weak_entity.clone();
                    let _ = cx.update(|cx| {
                        if let Some(entity) = weak.upgrade() {
                            entity.update(cx, |this, cx| {
                                for (index, persisted_msg) in messages.into_iter().enumerate() {
                                    log::debug!(
                                        "Loading historical message {}: timestamp={}",
                                        index,
                                        persisted_msg.timestamp
                                    );
                                    Self::add_update_to_list(
                                        &mut this.rendered_items,
                                        persisted_msg.update,
                                        index,
                                        cx,
                                    );
                                }

                                this.next_index = this.rendered_items.len();

                                log::info!(
                                    "Loaded history for session {}: {} items, next_index={}",
                                    session_id,
                                    this.rendered_items.len(),
                                    this.next_index
                                );

                                this.add_diff_summary_if_needed(cx);
                                this.scroll_handle.scroll_to_bottom();
                                cx.notify();
                            });
                        } else {
                            log::warn!("Entity dropped while loading history");
                        }
                    });
                }
                Err(e) => {
                    log::error!("Failed to load history for session {}: {}", session_id, e);
                }
            },
        )
        .detach();
    }

    /// Subscribe to session updates after the entity is created
    /// Uses MessageService for simplified subscription with automatic filtering
    pub fn subscribe_to_updates(
        entity: &Entity<Self>,
        session_filter: Option<String>,
        cx: &mut App,
    ) {
        let weak_entity = entity.downgrade();

        // Get MessageService for subscription
        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("MessageService not initialized, cannot subscribe to updates");
                return;
            }
        };

        // Clone session_filter for logging before and after the async closure
        let session_filter_log = session_filter.clone();
        let session_filter_log_end = session_filter.clone();

        // Use MessageService to subscribe with automatic filtering
        let mut rx = message_service.subscribe_session_updates(session_filter);

        // Spawn background task to receive updates and update entity
        cx.spawn(async move |cx| {
            log::info!(
                "Starting background task for session: {}",
                session_filter_log.as_deref().unwrap_or("all")
            );

            while let Some(update) = rx.recv().await {
                log::info!(
                    "Background task received update for session: {}",
                    session_filter_log.as_deref().unwrap_or("all")
                );

                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            let index = this.next_index;
                            this.next_index += 1;
                            // log::debug!("Processing update type: {:?}", update);
                            Self::add_update_to_list(&mut this.rendered_items, update, index, cx);

                            cx.notify(); // Trigger re-render immediately

                            // Scroll to bottom after render completes
                            this.scroll_handle.scroll_to_bottom();
                            log::info!(
                                "Rendered session update, total items: {}",
                                this.rendered_items.len()
                            );
                        });
                    } else {
                        log::warn!("Entity dropped, skipping update");
                    }
                });
            }

            log::info!(
                "Background task ended for session: {}",
                session_filter_log.as_deref().unwrap_or("all")
            );
        })
        .detach();

        log::info!(
            "Subscribed to session updates via MessageService for: {}",
            session_filter_log_end.as_deref().unwrap_or("all sessions")
        );
    }

    /// Subscribe to permission requests after the entity is created
    pub fn subscribe_to_permissions(
        entity: &Entity<Self>,
        session_filter: Option<String>,
        cx: &mut App,
    ) {
        let weak_entity = entity.downgrade();
        let permission_bus = AppState::global(cx).permission_bus.clone();

        // Create unbounded channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<
            crate::core::event_bus::permission_bus::PermissionRequestEvent,
        >();

        // Clone session_filter for logging after the closure
        let filter_log = session_filter.clone();
        let filter_log_inner = session_filter.clone();

        // Subscribe to permission bus, send requests to channel in callback
        permission_bus.subscribe(move |event| {
            // Filter by session_id if specified
            if let Some(ref filter_id) = session_filter {
                if &event.session_id != filter_id {
                    return; // Skip this permission request
                }
            }

            // This callback runs in agent I/O thread
            let _ = tx.send(event.clone());
            log::info!(
                "Permission request sent to channel: permission_id={}, session_id={}",
                event.permission_id,
                event.session_id
            );
        });

        // Spawn background task to receive from channel and update entity
        cx.spawn(async move |cx| {
            log::info!(
                "Starting permission background task for session: {}",
                filter_log_inner.as_deref().unwrap_or("all")
            );
            while let Some(event) = rx.recv().await {
                log::info!(
                    "Permission background task received request for session: {}",
                    event.session_id
                );
                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            log::info!(
                                "Processing permission request: permission_id={}",
                                event.permission_id
                            );
                            // Create PermissionRequestView entity using cx.new
                            let permission_view = cx.new(|cx| {
                                let inner = cx.new(|_| {
                                    crate::PermissionRequest::new(
                                        event.permission_id.clone(),
                                        event.session_id.clone(),
                                        &event.tool_call,
                                        event.options.clone(),
                                    )
                                });
                                crate::PermissionRequestView { item: inner }
                            });
                            this.rendered_items
                                .push(RenderedItem::PermissionRequest(permission_view));

                            cx.notify(); // Trigger re-render immediately

                            // Scroll to bottom after render completes
                            // Use a very large offset to ensure we reach the bottom
                            let scroll_handle = this.scroll_handle.clone();
                            cx.defer(move |_| {
                                // Set to a very large Y offset to ensure scrolling to bottom
                                scroll_handle
                                    .set_offset(gpui::point(gpui::px(0.), gpui::px(999999.)));
                            });

                            log::info!(
                                "Rendered permission request, total items: {}",
                                this.rendered_items.len()
                            );
                        });
                    } else {
                        log::warn!("Entity dropped, skipping permission request");
                    }
                });
            }
            log::info!(
                "Permission background task ended for session: {}",
                filter_log_inner.as_deref().unwrap_or("all")
            );
        })
        .detach();

        let filter_log_str = filter_log.as_deref().unwrap_or("all sessions");
        log::info!("Subscribed to permission bus for: {}", filter_log_str);
    }

    /// Subscribe to CodeSelectionBus to receive code selection events
    pub fn subscribe_to_code_selections(entity: &Entity<Self>, cx: &mut App) {
        crate::core::event_bus::subscribe_entity_to_code_selections(
            entity,
            AppState::global(cx).code_selection_bus.clone(),
            "ConversationPanel",
            |panel, selection, cx| {
                panel.code_selections.push(selection);
                cx.notify();
            },
            cx,
        );
    }

    /// Subscribe to WorkspaceUpdateBus to receive session status updates
    pub fn subscribe_to_status_updates(
        entity: &Entity<Self>,
        session_filter: Option<String>,
        cx: &mut App,
    ) {
        let weak_entity = entity.downgrade();
        let workspace_bus = AppState::global(cx).workspace_bus.clone();

        // Create unbounded channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<
            crate::core::event_bus::workspace_bus::WorkspaceUpdateEvent,
        >();

        let filter_log = session_filter.clone();
        let filter_log2 = session_filter.clone();
        let filter_log3 = session_filter.clone();

        // Subscribe to workspace bus, send status updates to channel in callback
        workspace_bus.lock().unwrap().subscribe(move |event| {
            // Only handle SessionStatusUpdated events
            if let crate::core::event_bus::workspace_bus::WorkspaceUpdateEvent::SessionStatusUpdated { session_id, .. } = event {
                // Filter by session_id if specified
                if let Some(ref filter_id) = session_filter {
                    if session_id != filter_id {
                        return; // Skip this status update
                    }
                }

                // Send to channel
                let _ = tx.send(event.clone());
                log::debug!(
                    "Session status update sent to channel: session_id={}",
                    session_id
                );
            }
        });

        // Spawn background task to receive from channel and update entity
        cx.spawn(async move |cx| {
            log::info!(
                "Starting status update background task for session: {}",
                filter_log2.as_deref().unwrap_or("all")
            );
            while let Some(event) = rx.recv().await {
                if let crate::core::event_bus::workspace_bus::WorkspaceUpdateEvent::SessionStatusUpdated {
                    session_id,
                    agent_name,
                    status,
                    last_active,
                    message_count,
                } = event
                {
                    log::debug!(
                        "Status update background task received for session: {}",
                        session_id
                    );
                    let weak = weak_entity.clone();
                    let _ = cx.update(|cx| {
                        if let Some(entity) = weak.upgrade() {
                            entity.update(cx, |this, cx| {
                                log::debug!(
                                    "Processing session status update: session_id={}, status={:?}",
                                    session_id,
                                    status
                                );

                                // Mark last message as complete when session completes or becomes idle
                                if matches!(status, SessionStatus::Completed | SessionStatus::Idle) {
                                    if let Some(last_item) = this.rendered_items.last_mut() {
                                        last_item.mark_complete();
                                        log::debug!("Marked last message as complete due to status change to {:?}", status);
                                    }

                                    // Add DiffSummary to message stream when session ends
                                    this.add_diff_summary_if_needed(cx);
                                }

                                // Update session status
                                this.session_status = Some(SessionStatusInfo {
                                    agent_name,
                                    status,
                                    last_active,
                                    message_count,
                                });
                                cx.notify(); // Trigger re-render
                            });
                        } else {
                            log::warn!("Entity dropped, skipping status update");
                        }
                    });
                }
            }
            log::info!(
                "Status update background task ended for session: {}",
                filter_log.as_deref().unwrap_or("all")
            );
        })
        .detach();

        log::info!(
            "Subscribed to workspace bus for status updates: {}",
            filter_log3.as_deref().unwrap_or("all sessions")
        );
    }

    /// Collect all ToolCall instances from rendered items
    fn collect_tool_calls(&self, cx: &App) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();

        for item in &self.rendered_items {
            if let RenderedItem::ToolCall(entity) = item {
                let tool_call = entity.read(cx).tool_call().clone();
                tool_calls.push(tool_call);
            }
        }

        tool_calls
    }

    /// Add DiffSummary to the message stream if there are any tool calls with diffs
    fn add_diff_summary_if_needed(&mut self, cx: &mut Context<Self>) {
        // Collect all tool calls
        let tool_calls = self.collect_tool_calls(cx);

        // Create summary data from tool calls
        let summary_data = DiffSummaryData::from_tool_calls(&tool_calls);

        // Only add summary if there are actual changes
        if summary_data.has_changes() {
            log::info!(
                "Adding DiffSummary to message stream with {} files changed",
                summary_data.total_files()
            );
            let diff_summary = cx.new(|_| DiffSummary::new(summary_data));
            self.rendered_items
                .push(RenderedItem::DiffSummary(diff_summary));
        }
    }

    /// Helper to add an update to the rendered items list
    fn add_update_to_list(
        items: &mut Vec<RenderedItem>,
        update: SessionUpdate,
        index: usize,
        cx: &mut App,
    ) {
        let update_type = session_update_type_name(&update);
        log::debug!("Processing SessionUpdate[{}]: {}", index, update_type);

        match update {
            SessionUpdate::UserMessageChunk(chunk) => {
                // Mark last message as complete if it was an AgentMessage
                if let Some(last_item) = items.last_mut() {
                    if !last_item.can_accept_agent_message_chunk()
                        && !last_item.can_accept_agent_thought_chunk()
                    {
                        // Different type, mark complete
                        last_item.mark_complete();
                    }
                }

                log::debug!("  └─ Creating UserMessage");
                items.push(Self::create_user_message(chunk, index, cx));
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                // Try to merge with the last AgentMessage item
                let merged = items
                    .last_mut()
                    .map(|last_item| {
                        if last_item.can_accept_agent_message_chunk() {
                            last_item.try_append_agent_message_chunk(chunk.clone())
                        } else {
                            // Different type, mark the last item as complete
                            last_item.mark_complete();
                            false
                        }
                    })
                    .unwrap_or(false);

                if merged {
                    log::debug!("  └─ Merged AgentMessageChunk into existing message");
                } else {
                    log::debug!("  └─ Creating new AgentMessage");
                    let data = create_agent_message_data(chunk, index);
                    items.push(RenderedItem::AgentMessage(
                        format!("agent-msg-{}", index),
                        data,
                    ));
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                let text = extract_text_from_content(&chunk.content);

                // Try to merge with the last AgentThought item
                let merged = items
                    .last_mut()
                    .map(|last_item| {
                        if last_item.can_accept_agent_thought_chunk() {
                            last_item.try_append_agent_thought_chunk(text.clone(), cx)
                        } else {
                            // Different type, mark the last item as complete
                            last_item.mark_complete();
                            false
                        }
                    })
                    .unwrap_or(false);

                if merged {
                    log::debug!("  └─ Merged AgentThoughtChunk into existing thought");
                } else {
                    log::debug!("  └─ Creating new AgentThought");
                    let entity = cx.new(|_| AgentThoughtItemState::new(text));
                    items.push(RenderedItem::AgentThought(entity));
                }
            }
            SessionUpdate::ToolCall(tool_call) => {
                // Check if a ToolCall with this ID already exists
                let mut found = false;
                for item in items.iter_mut() {
                    if let RenderedItem::ToolCall(entity) = item {
                        let entity_clone = entity.clone();
                        let matches =
                            entity_clone.read(cx).tool_call_id() == &tool_call.tool_call_id;

                        if matches {
                            // Update the existing tool call by replacing it with the new data
                            entity.update(cx, |state, cx| {
                                log::debug!(
                                    "  └─ Updating existing ToolCall: {} (title: {:?} -> {:?})",
                                    tool_call.tool_call_id,
                                    state.tool_call().title,
                                    tool_call.title
                                );
                                state.update_tool_call(tool_call.clone(), cx);
                            });
                            found = true;
                            break;
                        }
                    }
                }

                // If no existing ToolCall found, create a new one
                if !found {
                    // Mark last message as complete before adding ToolCall
                    if let Some(last_item) = items.last_mut() {
                        last_item.mark_complete();
                    }

                    log::debug!("  └─ Creating new ToolCall: {}", tool_call.tool_call_id);
                    let entity = cx.new(|_| ToolCallItem::new(tool_call));
                    items.push(RenderedItem::ToolCall(entity));
                }
            }
            SessionUpdate::ToolCallUpdate(tool_call_update) => {
                log::debug!("  └─ Updating ToolCall: {}", tool_call_update.tool_call_id);
                // Find the existing ToolCall entity by ID and update it
                let mut found = false;
                for item in items.iter_mut() {
                    if let RenderedItem::ToolCall(entity) = item {
                        let entity_clone = entity.clone();
                        let matches =
                            entity_clone.read(cx).tool_call_id() == &tool_call_update.tool_call_id;

                        if matches {
                            // Update the existing tool call
                            entity.update(cx, |state, cx| {
                                log::debug!(
                                    "     ✓ Found and updating ToolCall {} (status: {:?})",
                                    tool_call_update.tool_call_id,
                                    tool_call_update.fields.status
                                );
                                state.apply_update(tool_call_update.fields.clone(), cx);
                            });
                            found = true;
                            break;
                        }
                    }
                }

                // If no existing ToolCall found, try to create one from the update
                if !found {
                    log::warn!(
                        "     ⚠ ToolCallUpdate for non-existent ID: {}. Attempting to create.",
                        tool_call_update.tool_call_id
                    );

                    // Try to convert ToolCallUpdate to ToolCall
                    match ToolCall::try_from(tool_call_update) {
                        Ok(tool_call) => {
                            log::debug!("     ✓ Successfully created ToolCall from update");
                            let entity = cx.new(|_| ToolCallItem::new(tool_call));
                            items.push(RenderedItem::ToolCall(entity));
                        }
                        Err(e) => {
                            log::error!("     ✗ Failed to create ToolCall from update: {:?}", e);
                        }
                    }
                }
            }
            SessionUpdate::Plan(plan) => {
                // Mark last message as complete before adding Plan
                if let Some(last_item) = items.last_mut() {
                    last_item.mark_complete();
                }

                log::debug!("  └─ Creating Plan with {} entries", plan.entries.len());
                items.push(RenderedItem::Plan(plan));
            }
            SessionUpdate::AvailableCommandsUpdate(commands_update) => {
                // Mark last message as complete before adding commands update
                if let Some(last_item) = items.last_mut() {
                    last_item.mark_complete();
                }

                log::debug!(
                    "  └─ Commands update: {} available",
                    commands_update.available_commands.len()
                );
                items.push(RenderedItem::InfoUpdate(format!(
                    "📋 Available Commands: {} commands",
                    commands_update.available_commands.len()
                )));
            }
            SessionUpdate::CurrentModeUpdate(mode_update) => {
                // Mark last message as complete before adding mode update
                if let Some(last_item) = items.last_mut() {
                    last_item.mark_complete();
                }

                log::debug!("  └─ Mode changed to: {}", mode_update.current_mode_id);
                items.push(RenderedItem::InfoUpdate(format!(
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

    /// Create a UserMessage RenderedItem from a ContentChunk
    fn create_user_message(chunk: ContentChunk, _index: usize, cx: &mut App) -> RenderedItem {
        use crate::UserMessageData;

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

    /// Handle paste event and add images to pasted_images list
    /// Returns true if we handled the paste (had images), false otherwise
    fn handle_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        log::info!("Handling paste in ConversationPanel");

        let mut handled = false;
        if let Some(clipboard_item) = cx.read_from_clipboard() {
            for entry in clipboard_item.entries().iter() {
                if let ClipboardEntry::Image(image) = entry {
                    log::info!("Processing pasted image: {:?}", image.format);
                    let image = image.clone();
                    handled = true;

                    cx.spawn_in(window, async move |this, cx| {
                        match crate::utils::clipboard::image_to_content(image).await {
                            Ok((image_content, filename)) => {
                                _ = cx.update(move |_window, cx| {
                                    let _ = this.update(cx, |this, cx| {
                                        this.pasted_images.push((image_content, filename));
                                        cx.notify();
                                    });
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to process pasted image: {}", e);
                            }
                        }
                    })
                    .detach();
                }
            }
        }
        handled
    }

    /// Send a message to the current session
    /// Dispatches SendMessageToSession action to workspace for handling
    fn send_message(
        &self,
        text: String,
        images: Vec<(ImageContent, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Only send if we have a session_id
        let Some(ref session_id) = self.session_id else {
            log::warn!("Cannot send message: no session_id");
            return;
        };

        log::info!(
            "Dispatching SendMessageToSession action for session: {}",
            session_id
        );

        // Create action and dispatch to workspace
        let action = SendMessageToSession {
            session_id: session_id.clone(),
            message: text,
            images,
        };

        window.dispatch_action(Box::new(action), cx);
    }

    /// Cancel the current session
    /// Dispatches cancel via AgentService to avoid lost actions
    fn send_cancel_message(&self, _window: &mut Window, cx: &mut Context<Self>) {
        // Only send if we have a session_id
        let Some(ref session_id) = self.session_id else {
            log::warn!("Cannot cancel session: no session_id");
            return;
        };

        let session_id = session_id.clone();
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("AgentService not initialized, cannot cancel session");
                return;
            }
        };

        log::info!(
            "[ConversationPanel] Sending cancel request for session: {}",
            session_id
        );

        cx.spawn(async move |_this, _cx| {
            match agent_service.cancel_session_by_id(&session_id).await {
                Ok(()) => {
                    log::info!(
                        "[ConversationPanel] Session {} cancelled successfully",
                        session_id
                    );
                }
                Err(e) => {
                    log::error!(
                        "[ConversationPanel] Failed to cancel session {}: {}",
                        session_id,
                        e
                    );
                }
            }
        })
        .detach();
    }

    /// Render the loading skeleton when session is in progress
    fn render_loading_skeleton(&self) -> impl IntoElement {
        v_flex().gap_3().w_full().child(
            h_flex()
                .items_start()
                .gap_2()
                // Agent icon skeleton (circular, same size as agent icon)
                .child(Skeleton::new().size(px(16.)).rounded_full().mt_1())
                // Message content skeleton (2-3 lines with different widths)
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Skeleton::new().w(px(300.)).h_4().rounded_md())
                        .child(Skeleton::new().w(px(250.)).h_4().rounded_md())
                        .child(Skeleton::new().w(px(200.)).h_4().rounded_md()),
                ),
        )
    }

    /// Render the status bar at the bottom of the conversation panel
    fn render_status_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let status_info = self.session_status.as_ref()?;

        // Format last active time
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(status_info.last_active);
        let time_str = if duration.num_seconds() < 60 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else {
            format!("{}d ago", duration.num_days())
        };

        // Status icon and color based on session status
        let (status_icon, status_color) = match status_info.status {
            SessionStatus::Active => (IconName::CircleCheck, cx.theme().success),
            SessionStatus::InProgress => (IconName::Loader, cx.theme().primary),
            SessionStatus::Pending => (IconName::LoaderCircle, cx.theme().warning),
            SessionStatus::Idle => (IconName::Moon, cx.theme().muted_foreground),
            SessionStatus::Closed => (IconName::CircleX, cx.theme().red),
            SessionStatus::Completed => (IconName::CircleCheck, cx.theme().success),
            SessionStatus::Failed => (IconName::CircleX, cx.theme().red),
        };

        let status_text = format!("{:?}", status_info.status);

        Some(
            div()
                .flex_none()
                .w_full()
                .border_t_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted.opacity(0.3))
                .px_4()
                .py_2()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            // Left side: agent name and status
                            h_flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Icon::new(IconName::Bot)
                                                .size(px(14.))
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(cx.theme().foreground)
                                                .child(status_info.agent_name.clone()),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_1()
                                        .when_else(
                                            status_info.status == SessionStatus::InProgress,
                                            |this| {
                                                this.child(Spinner::new().icon(status_icon.clone()))
                                                    .size(px(12.))
                                                    .text_color(status_color)
                                            },
                                            |this| {
                                                this.child(
                                                    Icon::new(status_icon.clone())
                                                        .size(px(12.))
                                                        .text_color(status_color),
                                                )
                                            },
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(status_color)
                                                .child(status_text),
                                        ),
                                ),
                        )
                        .child(
                            // Right side: last active time and message count
                            h_flex()
                                .items_center()
                                .gap_4()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            Icon::new(IconName::Info)
                                                .size(px(12.))
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(time_str),
                                        ),
                                )
                                .when(status_info.message_count > 0, |this| {
                                    this.child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            .child(
                                                Icon::new(IconName::File)
                                                    .size(px(12.))
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!(
                                                        "{}",
                                                        status_info.message_count
                                                    )),
                                            ),
                                    )
                                }),
                        ),
                ),
        )
    }
}

impl DockPanel for ConversationPanel {
    fn title() -> &'static str {
        "Conversation"
    }

    fn description() -> &'static str {
        "Conversation panel using Agent Client Protocol schema with rich UI"
    }

    fn closable() -> bool {
        true
    }

    fn zoomable() -> Option<gpui_component::dock::PanelControl> {
        Some(gpui_component::dock::PanelControl::default())
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn on_active_any(view: gpui::AnyView, active: bool, window: &mut Window, cx: &mut App) {
        let _ = (view, active, window, cx);
    }

    fn paddings() -> gpui::Pixels {
        px(0.)
    }
}

impl Focusable for ConversationPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConversationPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = v_flex().p_4().gap_3().bg(cx.theme().background);

        for item in &self.rendered_items {
            match item {
                RenderedItem::UserMessage(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::AgentMessage(id, data) => {
                    let msg = AgentMessage::new(get_element_id(id), data.clone());
                    children = children.child(msg);
                }
                RenderedItem::AgentThought(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::Plan(plan) => {
                    let todo_list = AgentTodoList::from_plan(plan.clone());
                    children = children.child(v_flex().pl_6().child(todo_list));
                }
                RenderedItem::ToolCall(entity) => {
                    children = children.child(v_flex().pl_6().child(entity.clone()));
                }
                RenderedItem::PermissionRequest(entity) => {
                    children = children.child(v_flex().pl_6().child(entity.clone()));
                }
                RenderedItem::DiffSummary(entity) => {
                    // Render DiffSummary as part of message stream
                    children = children.child(entity.clone());
                }
                RenderedItem::InfoUpdate(text) => {
                    children = children.child(
                        div().pl_6().child(
                            div()
                                .p_2()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().muted.opacity(0.5))
                                .border_1()
                                .border_color(cx.theme().border.opacity(0.3))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(text.clone()),
                                ),
                        ),
                    );
                }
            }
        }

        // Add loading skeleton when session is in progress
        if let Some(status_info) = &self.session_status {
            if status_info.status == SessionStatus::InProgress {
                children = children.child(self.render_loading_skeleton());
            }
        }

        // Main layout: vertical flex with scroll area on top and input box at bottom
        v_flex()
            .id("messages")
            .size_full()
            .child(
                // Scrollable message area - takes remaining space
                div()
                    .id("conversation-scroll-container")
                    .flex_1()
                    .w_full()
                    .track_scroll(&self.scroll_handle)
                    .overflow_y_scroll()
                    .size_full()
                    .when(self.rendered_items.is_empty(), |this| {
                        // Show empty state with centered text
                        this.child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .text_sm()
                                        .child(t!("conversation.empty").to_string()),
                                ),
                        )
                    })
                    .when(!self.rendered_items.is_empty(), |this| {
                        // Show message list
                        this.pb_3() // Add padding at bottom so messages don't get hidden behind input box
                            .child(children)
                    }),
            )
            .when_some(self.render_status_bar(cx), |this, status_bar| {
                this.child(status_bar)
            })
            .child(
                // Chat input box at bottom (fixed, not scrollable)
                div()
                    .flex_none() // Don't allow shrinking
                    .w_full()
                    .bg(cx.theme().background) // Solid background
                    // .border_t_1()
                    .p_1()
                    // .border_color(cx.theme().border)
                    .child({
                        let entity = cx.entity().clone();
                        ChatInputBox::new("chat-input", self.input_state.clone())
                            .pasted_images(self.pasted_images.clone())
                            .code_selections(self.code_selections.clone())
                            .session_status(
                                self.session_status.as_ref().map(|info| info.status.clone()),
                            )
                            .on_paste(move |window, cx| {
                                entity.update(cx, |this, cx| {
                                    this.handle_paste(window, cx);
                                });
                            })
                            .on_remove_image(cx.listener(|this, idx, _, cx| {
                                // Remove the image at the given index
                                if *idx < this.pasted_images.len() {
                                    this.pasted_images.remove(*idx);
                                    cx.notify();
                                }
                            }))
                            .on_remove_code_selection(cx.listener(|this, idx, _, cx| {
                                // Remove the code selection at the given index
                                if *idx < this.code_selections.len() {
                                    this.code_selections.remove(*idx);
                                    cx.notify();
                                }
                            }))
                            .on_send(cx.listener(|this, _ev, window, cx| {
                                let text = this.input_state.read(cx).value().to_string();
                                if !text.trim().is_empty() || !this.pasted_images.is_empty() {
                                    // Clear the input
                                    this.input_state.update(cx, |state, cx| {
                                        state.set_value(SharedString::from(""), window, cx);
                                    });

                                    // Send the message with images if any
                                    let images = std::mem::take(&mut this.pasted_images);
                                    this.send_message(text, images, window, cx);

                                    // Clear pasted images and code selections after sending
                                    this.code_selections.clear();
                                    cx.notify();
                                }
                            }))
                            .on_cancel(cx.listener(|this, _ev, window, cx| {
                                log::info!("[ConversationPanel] on_cancel callback triggered");
                                this.send_cancel_message(window, cx);
                                cx.notify();
                            }))
                    }),
            )
    }
}
