use gpui::{
    div, prelude::*, px, App, ClipboardEntry, Context, ElementId, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex,
    input::InputState,
    scroll::ScrollableElement,
    v_flex, ActiveTheme, Icon, IconName, Sizable,
};

// Use the published ACP schema crate
use agent_client_protocol_schema::{
    ContentBlock, ContentChunk, EmbeddedResourceResource, Plan, SessionUpdate, ToolCall,
    ToolCallContent, ToolCallStatus,
};

use crate::{
    components::PastedImage, core::agent::AgentHandle, panels::dock_panel::DockPanel, AgentMessage,
    AgentMessageData, AgentTodoList, AppState, ChatInputBox, PermissionRequestView,
    UserMessageData,
};

// Import from types module
use super::types::{get_file_icon, ResourceInfo, ToolCallStatusExt, ToolKindExt};

// ============================================================================
// Stateful Resource Item
// ============================================================================

struct ResourceItemState {
    resource: ResourceInfo,
    open: bool,
}

impl ResourceItemState {
    fn new(resource: ResourceInfo) -> Self {
        Self {
            resource,
            open: false,
        }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }
}

impl Render for ResourceItemState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let line_count = self
            .resource
            .text
            .as_ref()
            .map(|t| t.lines().count())
            .unwrap_or(0);

        let is_open = self.open;
        let has_content = self.resource.text.is_some();
        let resource_name = self.resource.name.clone();
        let mime_type = self.resource.mime_type.clone();

        Collapsible::new()
            .open(is_open)
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        Icon::new(get_file_icon(&mime_type.map(|s| s.to_string())))
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(resource_name.clone()),
                    )
                    .when(line_count > 0, |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} lines", line_count)),
                        )
                    })
                    .when(has_content, |this| {
                        this.child(
                            Button::new(SharedString::from(format!(
                                "resource-toggle-{}",
                                resource_name
                            )))
                            .icon(if is_open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(
                                |this, _ev, _window, cx| {
                                    this.toggle(cx);
                                },
                            )),
                        )
                    }),
            )
            .when(has_content, |this| {
                this.content(
                    div()
                        .w_full()
                        .p_3()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().secondary)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_family("Monaco, 'Courier New', monospace")
                                .text_color(cx.theme().foreground)
                                .line_height(px(18.))
                                .child(self.resource.text.clone().unwrap_or_default()),
                        ),
                )
            })
    }
}

// ============================================================================
// Stateful Tool Call Item
// ============================================================================

struct ToolCallItemState {
    tool_call: ToolCall,
    open: bool,
}

impl ToolCallItemState {
    fn new(tool_call: ToolCall, open: bool) -> Self {
        Self { tool_call, open }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    fn has_content(&self) -> bool {
        !self.tool_call.content.is_empty()
    }

    /// Update this tool call with fields from a ToolCallUpdate
    fn apply_update(
        &mut self,
        update_fields: agent_client_protocol_schema::ToolCallUpdateFields,
        cx: &mut Context<Self>,
    ) {
        // Use the built-in update method from ToolCall
        self.tool_call.update(update_fields);

        // Auto-open when tool call completes or fails (so user can see result)
        match self.tool_call.status {
            ToolCallStatus::Completed | ToolCallStatus::Failed => {
                if self.has_content() {
                    self.open = true;
                }
            }
            _ => {}
        }

        cx.notify();
    }

    /// Get the tool call ID for matching updates
    fn tool_call_id(&self) -> &agent_client_protocol_schema::ToolCallId {
        &self.tool_call.tool_call_id
    }
}

impl Render for ToolCallItemState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_content = self.has_content();
        let status_color = match self.tool_call.status {
            ToolCallStatus::Completed => cx.theme().green,
            ToolCallStatus::Failed => cx.theme().red,
            ToolCallStatus::InProgress => cx.theme().accent,
            ToolCallStatus::Pending | _ => cx.theme().muted_foreground,
        };

        let open = self.open;
        let tool_call_id = self.tool_call.tool_call_id.clone();
        let title = self.tool_call.title.clone();
        let kind_icon = self.tool_call.kind.icon();
        let status_icon = self.tool_call.status.icon();

        Collapsible::new()
            .open(open)
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
                    .child(
                        Icon::new(kind_icon)
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(title),
                    )
                    .child(
                        Icon::new(status_icon)
                            .size(px(14.))
                            .text_color(status_color),
                    )
                    .when(has_content, |this| {
                        this.child(
                            Button::new(SharedString::from(format!(
                                "tool-call-{}-toggle",
                                tool_call_id
                            )))
                            .icon(if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(
                                |this, _ev, _window, cx| {
                                    this.toggle(cx);
                                },
                            )),
                        )
                    }),
            )
            .when(has_content, |this| {
                this.content(v_flex().gap_1().p_3().pl_8().children(
                    self.tool_call.content.iter().filter_map(|content| {
                        match content {
                            ToolCallContent::Content(c) => match &c.content {
                                ContentBlock::Text(text) => Some(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(cx.theme().muted_foreground)
                                        .line_height(px(18.))
                                        .child(text.text.clone()),
                                ),
                                _ => None,
                            },
                            ToolCallContent::Diff(diff) => Some(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .line_height(px(18.))
                                    .child(format!(
                                        "Modified: {}\n{} -> {}",
                                        diff.path.display(),
                                        diff.old_text.as_deref().unwrap_or("<new file>"),
                                        diff.new_text
                                    )),
                            ),
                            ToolCallContent::Terminal(terminal) => Some(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .line_height(px(18.))
                                    .child(format!("Terminal: {}", terminal.terminal_id)),
                            ),
                            _ => None,
                        }
                    }),
                ))
            })
    }
}

// ============================================================================
// User Message View
// ============================================================================

struct UserMessageView {
    data: Entity<UserMessageData>,
    resource_items: Vec<Entity<ResourceItemState>>,
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let mut resource_index = 0;

        v_flex()
            .gap_3()
            .w_full()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::User)
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("You"),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .pl_6()
                    .w_full()
                    .children(data.contents.into_iter().filter_map(|content| {
                        match &content {
                            ContentBlock::Text(text_content) => Some(
                                div()
                                    .text_size(px(14.))
                                    .text_color(cx.theme().foreground)
                                    .line_height(px(22.))
                                    .child(text_content.text.clone())
                                    .into_any_element(),
                            ),
                            ContentBlock::ResourceLink(_) | ContentBlock::Resource(_) => {
                                if ResourceInfo::from_content_block(&content).is_some() {
                                    let current_index = resource_index;
                                    resource_index += 1;

                                    if let Some(item) = self.resource_items.get(current_index) {
                                        Some(item.clone().into_any_element())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    })),
            )
    }
}

// ============================================================================
// Rendered Item
// ============================================================================

enum RenderedItem {
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
    fn try_append_agent_message_chunk(&mut self, chunk: ContentChunk) -> bool {
        if let RenderedItem::AgentMessage(_id, ref mut data) = self {
            data.chunks.push(chunk);
            true
        } else {
            false
        }
    }

    /// Try to append an AgentThoughtChunk to this item (returns true if successful)
    fn try_append_agent_thought_chunk(&mut self, text: String) -> bool {
        if let RenderedItem::AgentThought(ref mut existing_text) = self {
            existing_text.push_str(&text);
            true
        } else {
            false
        }
    }

    /// Mark an AgentMessage as complete (no more chunks expected)
    fn mark_complete(&mut self) {
        if let RenderedItem::AgentMessage(_id, ref mut data) = self {
            data.meta.is_complete = true;
        }
    }

    /// Check if this item can accept chunks of a given type
    fn can_accept_agent_message_chunk(&self) -> bool {
        matches!(self, RenderedItem::AgentMessage(..))
    }

    fn can_accept_agent_thought_chunk(&self) -> bool {
        matches!(self, RenderedItem::AgentThought(..))
    }
}

/// Conversation panel that displays SessionUpdate messages from ACP
pub struct ConversationPanelAcp {
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
    /// List of pasted images
    pasted_images: Vec<PastedImage>,
}

impl ConversationPanelAcp {
    /// Create a new panel with mock data (for demo purposes)
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        log::info!("🚀 Creating ConversationPanelAcp view");
        let entity = cx.new(|cx| Self::new(window, cx));
        Self::subscribe_to_updates(&entity, None, cx);
        Self::subscribe_to_permissions(&entity, None, cx);
        log::info!("✅ ConversationPanelAcp view created and subscribed");
        entity
    }

    /// Create a new panel for a specific session (no mock data)
    pub fn view_for_session(session_id: String, window: &mut Window, cx: &mut App) -> Entity<Self> {
        log::info!(
            "🚀 Creating ConversationPanelAcp for session: {}",
            session_id
        );
        let entity = cx.new(|cx| Self::new_for_session(session_id.clone(), window, cx));

        // Load historical messages before subscribing to new updates
        Self::load_history_for_session(&entity, session_id.clone(), cx);

        Self::subscribe_to_updates(&entity, Some(session_id.clone()), cx);
        Self::subscribe_to_permissions(&entity, Some(session_id.clone()), cx);
        log::info!(
            "✅ ConversationPanelAcp created for session: {}",
            session_id
        );
        entity
    }

    fn new(_window: &mut Window, cx: &mut App) -> Self {
        log::info!("🔧 Initializing ConversationPanelAcp (new)");
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

        let panel = Self {
            focus_handle,
            rendered_items,
            next_index,
            session_id: None,
            scroll_handle,
            input_state,
            pasted_images: Vec::new(),
        };

        panel
    }

    fn new_for_session(session_id: String, window: &mut Window, cx: &mut App) -> Self {
        log::info!(
            "🔧 Initializing ConversationPanelAcp for session: {}",
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

    /// Helper to add an update to the rendered items list
    fn add_update_to_list(
        items: &mut Vec<RenderedItem>,
        update: SessionUpdate,
        index: usize,
        cx: &mut App,
    ) {
        let update_type = Self::session_update_type_name(&update);
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
                    let data = Self::create_agent_message_data(chunk, index);
                    items.push(RenderedItem::AgentMessage(
                        format!("agent-msg-{}", index),
                        data,
                    ));
                }
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                let text = Self::extract_text_from_content(&chunk.content);

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
                // Mark last message as complete before adding ToolCall
                if let Some(last_item) = items.last_mut() {
                    last_item.mark_complete();
                }

                log::debug!("  └─ Creating ToolCall: {}", tool_call.tool_call_id);
                let entity = cx.new(|_| ToolCallItemState::new(tool_call, false));
                items.push(RenderedItem::ToolCall(entity));
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
                    match agent_client_protocol_schema::ToolCall::try_from(tool_call_update) {
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

    fn create_user_message(chunk: ContentChunk, _index: usize, cx: &mut App) -> RenderedItem {
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

    fn create_agent_message_data(chunk: ContentChunk, _index: usize) -> AgentMessageData {
        AgentMessageData::new("default-session").add_chunk(chunk)
    }

    /// Extract text from ContentBlock
    fn extract_text_from_content(content: &ContentBlock) -> String {
        match content {
            ContentBlock::Text(text_content) => text_content.text.clone(),
            ContentBlock::Image(img) => {
                format!("[Image: {}]", img.mime_type)
            }
            ContentBlock::Audio(audio) => {
                format!("[Audio: {}]", audio.mime_type)
            }
            ContentBlock::ResourceLink(link) => {
                format!("[Resource: {}]", link.name)
            }
            ContentBlock::Resource(resource) => match &resource.resource {
                EmbeddedResourceResource::TextResourceContents(text_res) => {
                    format!(
                        "[Resource: {}]\n{}",
                        text_res.uri,
                        &text_res.text[..text_res.text.len().min(200)]
                    )
                }
                EmbeddedResourceResource::BlobResourceContents(blob_res) => {
                    format!("[Binary Resource: {}]", blob_res.uri)
                }
                _ => "[Unknown Resource]".to_string(),
            },
            _ => "[Unknown Content]".to_string(),
        }
    }

    /// Get a human-readable type name for SessionUpdate (for logging)
    fn session_update_type_name(update: &SessionUpdate) -> &'static str {
        match update {
            SessionUpdate::UserMessageChunk(_) => "UserMessageChunk",
            SessionUpdate::AgentMessageChunk(_) => "AgentMessageChunk",
            SessionUpdate::AgentThoughtChunk(_) => "AgentThoughtChunk",
            SessionUpdate::ToolCall(_) => "ToolCall",
            SessionUpdate::ToolCallUpdate(_) => "ToolCallUpdate",
            SessionUpdate::Plan(_) => "Plan",
            SessionUpdate::AvailableCommandsUpdate(_) => "AvailableCommandsUpdate",
            SessionUpdate::CurrentModeUpdate(_) => "CurrentModeUpdate",
            _ => "Unknown/Future SessionUpdate Type",
        }
    }

    fn get_id(id: &str) -> ElementId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        ElementId::from(("item", hasher.finish()))
    }

    /// Handle paste event and add images to pasted_images list
    fn handle_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("Handling paste in ConversationPanelAcp");

        if let Some(clipboard_item) = cx.read_from_clipboard() {
            for entry in clipboard_item.entries().iter() {
                if let ClipboardEntry::Image(image) = entry {
                    log::info!("Processing pasted image: {:?}", image.format);
                    let image = image.clone();

                    cx.spawn_in(window, async move |this, mut cx| {
                        // Write image to temp file
                        match crate::utils::file::write_image_to_temp_file(&image).await {
                            Ok(temp_path) => {
                                log::info!("Image written to temp file: {}", temp_path);

                                // Extract filename from path
                                let filename = std::path::Path::new(&temp_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("image.png")
                                    .to_string();

                                // Add to pasted_images
                                _ = cx.update(move |_window, cx| {
                                    this.update(cx, |this, cx| {
                                        this.pasted_images.push(PastedImage {
                                            path: temp_path,
                                            filename,
                                        });
                                        cx.notify();
                                    });
                                });
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
    }

    /// Send a message to the current session
    fn send_message(&self, text: String, cx: &mut Context<Self>) {
        // Only send if we have a session_id
        let Some(ref session_id) = self.session_id else {
            log::warn!("Cannot send message: no session_id");
            return;
        };

        log::info!("Sending message to session: {}", session_id);

        let session_id = session_id.clone();

        // Spawn async task to send the message
        cx.spawn(async move |_this, cx| {
            // Immediately publish user message to session bus for instant UI feedback
            use agent_client_protocol_schema as schema;
            use std::sync::Arc;

            // Create user message chunk
            let content_block = schema::ContentBlock::from(text.clone());
            let content_chunk = schema::ContentChunk::new(content_block);

            let user_event = crate::core::event_bus::session_bus::SessionUpdateEvent {
                session_id: session_id.clone(),
                update: Arc::new(schema::SessionUpdate::UserMessageChunk(content_chunk)),
            };

            // Publish to session bus
            cx.update(|cx| {
                AppState::global(cx).session_bus.publish(user_event);
            })
            .ok();
            log::info!("Published user message to session bus: {}", session_id);

            // Get agent handle and send prompt
            let agent_handle: Option<Arc<AgentHandle>> = cx
                .update(|cx| {
                    AppState::global(cx).agent_manager().and_then(|m| {
                        // Get the first available agent
                        let agents = m.list_agents();
                        agents.first().and_then(|name| m.get(name))
                    })
                })
                .ok()
                .flatten();

            if let Some(agent_handle) = agent_handle {
                // Send the prompt
                let request = agent_client_protocol::PromptRequest {
                    session_id: agent_client_protocol::SessionId::from(session_id.clone()),
                    prompt: vec![text.into()],
                    meta: None,
                };

                match agent_handle.prompt(request).await {
                    Ok(_) => {
                        log::info!("Prompt sent successfully to session: {}", session_id);
                    }
                    Err(e) => {
                        log::error!("Failed to send prompt to session {}: {}", session_id, e);
                    }
                }
            } else {
                log::error!("No agent handle available");
            }
        })
        .detach();
    }
}

impl DockPanel for ConversationPanelAcp {
    fn title() -> &'static str {
        "Conversation (ACP)"
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

impl Focusable for ConversationPanelAcp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConversationPanelAcp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = v_flex().p_4().gap_6().bg(cx.theme().background);

        for item in &self.rendered_items {
            match item {
                RenderedItem::UserMessage(entity) => {
                    children = children.child(entity.clone());
                }
                RenderedItem::AgentMessage(id, data) => {
                    let msg = AgentMessage::new(Self::get_id(id), data.clone());
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
            .on_action(
                cx.listener(|this, _: &crate::app::actions::Paste, window, cx| {
                    this.handle_paste(window, cx);
                }),
            )
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
            .child(
                // Chat input box at bottom (fixed, not scrollable)
                div()
                    .flex_none() // Don't allow shrinking
                    .w_full()
                    .bg(cx.theme().background) // Solid background
                    // .border_t_1()
                    .p_1()
                    // .border_color(cx.theme().border)
                    .child(
                        ChatInputBox::new("chat-input", self.input_state.clone())
                            .pasted_images(self.pasted_images.clone())
                            .on_remove_image(cx.listener(|this, idx, _, cx| {
                                // Remove the image at the given index
                                if *idx < this.pasted_images.len() {
                                    this.pasted_images.remove(*idx);
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
                                    this.send_message(text, cx);

                                    // Clear pasted images after sending
                                    this.pasted_images.clear();
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }
}
