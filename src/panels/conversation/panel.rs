use gpui::{
    App, ClipboardEntry, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render, ScrollHandle, SharedString, Styled, Window, div, prelude::*, px
};
use gpui_component::{
    h_flex, input::InputState, scroll::ScrollableElement, v_flex, ActiveTheme, Icon, IconName,
};

// Use the published ACP schema crate
use agent_client_protocol::{ContentChunk, ImageContent, SessionUpdate, ToolCall};
use chrono::{DateTime, Utc};

use crate::{
    app::actions::AddCodeSelection, panels::dock_panel::DockPanel, AgentMessage, AgentTodoList,
    AppState, ChatInputBox, SendMessageToSession,
    core::services::SessionStatus,
};

// Import from submodules
use super::{
    components::{ResourceItemState, ToolCallItemState, UserMessageView},
    helpers::{extract_text_from_content, get_element_id, session_update_type_name},
    rendered_item::{create_agent_message_data, RenderedItem},
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

    fn new(_window: &mut Window, cx: &mut App) -> Self {
        log::info!("🔧 Initializing ConversationPanel (new)");
        let focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();
        let input_state = cx.new(|cx| {
            InputState::new(_window, cx)
                .auto_grow(1, 3)
                .soft_wrap(true)
                .placeholder("Type a message...")
        });
        let session_updates = Self::load_mock_data();

        let mut rendered_items = Vec::new();
        for (index, update) in session_updates.into_iter().enumerate() {
            Self::add_update_to_list(&mut rendered_items, update, index, cx);
        }

        let next_index = rendered_items.len();

        Self {
            focus_handle,
            rendered_items,
            next_index,
            session_id: None,
            scroll_handle,
            input_state,
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            session_status: None,
        }
    }

    fn new_for_session(session_id: String, window: &mut Window, cx: &mut App) -> Self {
        log::info!(
            "🔧 Initializing ConversationPanel for session: {}",
            session_id
        );
        let focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(1, 3)
                .soft_wrap(true)
                .placeholder("Type a message...")
        });

        Self {
            focus_handle,
            rendered_items: Vec::new(),
            next_index: 0,
            session_id: Some(session_id),
            scroll_handle,
            input_state,
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            session_status: None,
        }
    }

    /// Load historical messages for a session
    pub fn load_history_for_session(entity: &Entity<Self>, session_id: String, cx: &mut App) {
        let weak_entity = entity.downgrade();

        // Get MessageService
        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("MessageService not initialized, cannot load history");
                return;
            }
        };

        log::info!("Loading history for session: {}", session_id);

        // Spawn background task to load history
        cx.spawn(async move |cx| {
            match message_service.load_history(&session_id).await {
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
                                // Process each historical message
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

                                // Update next_index to continue after historical messages
                                this.next_index = this.rendered_items.len();

                                log::info!(
                                    "Loaded history for session {}: {} items, next_index={}",
                                    session_id,
                                    this.rendered_items.len(),
                                    this.next_index
                                );

                                cx.notify(); // Trigger re-render

                                // Scroll to bottom after loading history
                                let scroll_handle = this.scroll_handle.clone();
                                cx.defer(move |_| {
                                    scroll_handle
                                        .set_offset(gpui::point(gpui::px(0.), gpui::px(999999.)));
                                });
                            });
                        } else {
                            log::warn!("Entity dropped while loading history");
                        }
                    });
                }
                Err(e) => {
                    log::error!("Failed to load history for session {}: {}", session_id, e);
                }
            }
        })
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
                            log::info!("Processing update type: {:?}", update);
                            Self::add_update_to_list(&mut this.rendered_items, update, index, cx);

                            cx.notify(); // Trigger re-render immediately

                            // Scroll to bottom after render completes
                            let scroll_handle = this.scroll_handle.clone();
                            cx.defer(move |_| {
                                // Set to a very large Y offset to ensure scrolling to bottom
                                scroll_handle
                                    .set_offset(gpui::point(gpui::px(0.), gpui::px(999999.)));
                            });

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
                            last_item.try_append_agent_thought_chunk(text.clone())
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
                    items.push(RenderedItem::AgentThought(text));
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
                                    state.tool_call.title,
                                    tool_call.title
                                );
                                // Replace the entire tool_call to get the latest data
                                state.tool_call = tool_call.clone();
                                // If there's content, open it
                                if state.has_content() {
                                    state.open = true;
                                }
                                cx.notify();
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
                    let entity = cx.new(|_| ToolCallItemState::new(tool_call, false));
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
                            let entity = cx.new(|_| ToolCallItemState::new(tool_call, false));
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

    /// Load mock session updates from JSON file
    fn load_mock_data() -> Vec<SessionUpdate> {
        let json_str = include_str!("../../../mock_conversation_acp.json");
        match serde_json::from_str::<Vec<SessionUpdate>>(json_str) {
            Ok(updates) => {
                log::info!(
                    "✅ Successfully loaded {} mock conversation updates",
                    updates.len()
                );
                updates
            }
            Err(e) => {
                log::error!("❌ Failed to load mock conversation data: {}", e);
                Vec::new()
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
                        // Write image to temp file first (to get filename)
                        match crate::utils::file::write_image_to_temp_file(&image).await {
                            Ok(temp_path) => {
                                log::info!("Image written to temp file: {}", temp_path);

                                // Extract filename from path
                                let filename = std::path::Path::new(&temp_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("image.png")
                                    .to_string();

                                // Read the file and convert to base64 (using std::fs for sync read)
                                match std::fs::read(&temp_path) {
                                    Ok(bytes) => {
                                        use base64::Engine;
                                        let base64_data = base64::engine::general_purpose::STANDARD
                                            .encode(&bytes);

                                        // Determine MIME type from format
                                        let mime_type = match image.format {
                                            gpui::ImageFormat::Png => "image/png",
                                            gpui::ImageFormat::Jpeg => "image/jpeg",
                                            gpui::ImageFormat::Webp => "image/webp",
                                            gpui::ImageFormat::Gif => "image/gif",
                                            gpui::ImageFormat::Svg => "image/svg+xml",
                                            gpui::ImageFormat::Bmp => "image/bmp",
                                            gpui::ImageFormat::Tiff => "image/tiff",
                                        }
                                        .to_string();

                                        // Create ImageContent
                                        let image_content =
                                            ImageContent::new(base64_data, mime_type);

                                        // Add to pasted_images
                                        _ = cx.update(move |_window, cx| {
                                            let _ = this.update(cx, |this, cx| {
                                                this.pasted_images.push((image_content, filename));
                                                cx.notify();
                                            });
                                        });

                                        // Optionally delete the temp file after reading
                                        let _ = std::fs::remove_file(&temp_path);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to read image file: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to write image to temp file: {}", e);
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
        images: &Vec<(ImageContent, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Only send if we have a session_id
        let Some(ref session_id) = self.session_id else {
            log::warn!("Cannot send message: no session_id");
            return;
        };

        log::info!("Dispatching SendMessageToSession action for session: {}", session_id);

        // Create action and dispatch to workspace
        let action = SendMessageToSession {
            session_id: session_id.clone(),
            message: text,
            images: images.clone(),
        };

        window.dispatch_action(Box::new(action), cx);
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
                                        .child(Icon::new(status_icon).size(px(12.)).text_color(status_color))
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
                                                    .child(format!("{}", status_info.message_count)),
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
        let mut children = v_flex().p_4().gap_6().bg(cx.theme().background);

        for item in &self.rendered_items {
            match item {
                RenderedItem::UserMessage(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::AgentMessage(id, data) => {
                    let msg = AgentMessage::new(get_element_id(id), data.clone());
                    children = children.child(msg);
                }
                RenderedItem::AgentThought(text) => {
                    children = children.child(
                        div().pl_6().child(
                            div()
                                .p_3()
                                .rounded_lg()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().muted.opacity(0.3))
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
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Thinking..."),
                                        ),
                                )
                                .child(
                                    div()
                                        .mt_2()
                                        .text_sm()
                                        .italic()
                                        .text_color(cx.theme().foreground.opacity(0.8))
                                        .child(text.clone()),
                                ),
                        ),
                    );
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

        // Main layout: vertical flex with scroll area on top and input box at bottom
        v_flex()
            .size_full()
            .child(
                // Scrollable message area - takes remaining space
                div()
                    .id("conversation-scroll-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .overflow_y_scrollbar()
                    // .track_scroll(&self.scroll_handle)
                    .pb_3() // Add padding at bottom so messages don't get hidden behind input box
                    .child(children),
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
                                    this.send_message(text, &this.pasted_images, window, cx);

                                    // Clear pasted images and code selections after sending
                                    this.pasted_images.clear();
                                    this.code_selections.clear();
                                    cx.notify();
                                }
                            }))
                    }),
            )
    }
}
