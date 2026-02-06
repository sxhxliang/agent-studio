use gpui::{
    App, ClipboardEntry, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, ScrollHandle, SharedString, Styled, Window, div, prelude::*, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, h_flex, input::InputState, skeleton::Skeleton,
    spinner::Spinner, v_flex,
};

// Use the published ACP schema crate
use agent_client_protocol::{ContentChunk, ImageContent, PlanEntryStatus, SessionUpdate, ToolCall};
use chrono::{DateTime, Utc};
use rust_i18n::t;
use smol::Timer;
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
    update_state_manager::{UpdateProcessor, UpdateStateIndex},
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
    /// Fast index for O(1) lookups (tool calls, streaming messages)
    update_index: UpdateStateIndex,
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

const MESSAGE_SERVICE_RETRY_DELAY_MS: u64 = 500;
const MESSAGE_SERVICE_MAX_RETRIES: usize = 60;

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
        let update_index = UpdateStateIndex::new();
        let next_index = rendered_items.len();

        Self {
            focus_handle,
            rendered_items,
            update_index,
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
        let persistence_service = match AppState::global(cx).persistence_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("PersistenceService not initialized, cannot load history");
                return;
            }
        };

        let weak_entity = entity.downgrade();

        log::info!("Loading history for session: {}", session_id);

        cx.spawn(
            async move |cx| match persistence_service.load_messages(&session_id).await {
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
                                let agent_name = AppState::global(cx)
                                    .agent_service()
                                    .and_then(|service| service.get_agent_for_session(&session_id));

                                // Use optimized UpdateProcessor for batch loading
                                for persisted_msg in messages.into_iter() {
                                    log::debug!(
                                        "Loading historical message {}: timestamp={}",
                                        this.next_index,
                                        persisted_msg.timestamp
                                    );

                                    let mut processor = UpdateProcessor::<ConversationPanel>::new(
                                        &mut this.rendered_items,
                                        &mut this.update_index,
                                        Some(session_id.as_str()),
                                        agent_name.as_deref(),
                                        this.next_index,
                                    );

                                    processor.process_update(persisted_msg.update, cx);
                                    this.next_index += 1;
                                }

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
        Self::subscribe_to_updates_with_retry(
            entity.clone(),
            session_filter,
            MESSAGE_SERVICE_MAX_RETRIES,
            cx,
        );
    }

    fn subscribe_to_updates_with_retry(
        entity: Entity<Self>,
        session_filter: Option<String>,
        remaining_attempts: usize,
        cx: &mut App,
    ) {
        let weak_entity = entity.downgrade();

        // Get MessageService for subscription
        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                if remaining_attempts == 0 {
                    log::error!("MessageService not initialized, cannot subscribe to updates");
                    return;
                }

                let weak_entity = weak_entity.clone();
                cx.spawn(async move |cx| {
                    Timer::after(Duration::from_millis(MESSAGE_SERVICE_RETRY_DELAY_MS)).await;
                    let _ = cx.update(|cx| {
                        if let Some(entity) = weak_entity.upgrade() {
                            Self::subscribe_to_updates_with_retry(
                                entity,
                                session_filter,
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

            while let Some(event) = rx.recv().await {
                log::info!(
                    "Background task received update for session: {}",
                    session_filter_log.as_deref().unwrap_or("all")
                );

                let session_id = event.session_id.clone();
                let agent_name = event.agent_name.clone();
                let update = (*event.update).clone();

                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            // Use optimized UpdateProcessor
                            let mut processor = UpdateProcessor::<ConversationPanel>::new(
                                &mut this.rendered_items,
                                &mut this.update_index,
                                Some(session_id.as_str()),
                                agent_name.as_deref(),
                                this.next_index,
                            );

                            processor.process_update(update, cx);
                            this.next_index += 1;

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
        let event_hub = AppState::global(cx).event_hub.clone();

        // Create unbounded channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<
            crate::core::event_bus::PermissionRequestEvent,
        >();

        // Clone session_filter for logging after the closure
        let filter_log = session_filter.clone();
        let filter_log_inner = session_filter.clone();

        // Subscribe to permission requests, send requests to channel in callback
        if let Some(filter_id) = session_filter.clone() {
            event_hub.subscribe_permission_requests_for_session(filter_id, move |event| {
                // This callback runs in agent I/O thread
                let _ = tx.send(event.clone());
                log::info!(
                    "Permission request sent to channel: permission_id={}, session_id={}",
                    event.permission_id,
                    event.session_id
                );
            });
        } else {
            event_hub.subscribe_permission_requests(move |event| {
                // This callback runs in agent I/O thread
                let _ = tx.send(event.clone());
                log::info!(
                    "Permission request sent to channel: permission_id={}, session_id={}",
                    event.permission_id,
                    event.session_id
                );
            });
        }

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
        log::info!("Subscribed to permission events for: {}", filter_log_str);
    }

    /// Subscribe to code selection events via EventHub
    pub fn subscribe_to_code_selections(entity: &Entity<Self>, cx: &mut App) {
        crate::core::event_bus::subscribe_entity_to_code_selections(
            entity,
            AppState::global(cx).event_hub.clone(),
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
        let event_hub = AppState::global(cx).event_hub.clone();

        // Create unbounded channel for cross-thread communication
        let (tx, mut rx) =
            tokio::sync::mpsc::unbounded_channel::<crate::core::event_bus::WorkspaceUpdateEvent>();

        let filter_log = session_filter.clone();
        let filter_log2 = session_filter.clone();
        let filter_log3 = session_filter.clone();

        // Subscribe to workspace bus, send status updates to channel in callback
        event_hub.subscribe_workspace_updates(move |event| {
            // Only handle SessionStatusUpdated events
            if let crate::core::event_bus::WorkspaceUpdateEvent::SessionStatusUpdated {
                session_id,
                ..
            } = event
            {
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
                if let crate::core::event_bus::WorkspaceUpdateEvent::SessionStatusUpdated {
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

    /// Render the loading skeleton and status info when session is in progress
    fn render_loading_skeleton(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Only show loading skeleton when session is actively processing
        let should_show_loading = self.session_status.as_ref().map_or(false, |status_info| {
            matches!(
                status_info.status,
                SessionStatus::InProgress | SessionStatus::Pending
            )
        });

        if !should_show_loading {
            return v_flex().into_any_element();
        }

        // Find the current todo from Plan entries
        let current_todo = self.rendered_items.iter().rev().find_map(|item| {
            if let RenderedItem::Plan(plan) = item {
                plan.entries
                    .iter()
                    .find(|entry| entry.status == PlanEntryStatus::InProgress)
                    .map(|entry| entry.content.clone())
            } else {
                None
            }
        });

        // Build status indicator row
        let status_info = self.session_status.as_ref().unwrap(); // Safe because of check above
        let (status_icon, status_color) = match status_info.status {
            SessionStatus::InProgress => (IconName::Loader, cx.theme().primary),
            SessionStatus::Pending => (IconName::LoaderCircle, cx.theme().warning),
            _ => return v_flex().into_any_element(), // Fallback
        };

        // Calculate elapsed time from last_active
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(status_info.last_active);
        let total_seconds = duration.num_seconds().max(0) as u64;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let elapsed_time = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

        // Main skeleton layout: horizontal layout with avatar spinner + status info + content skeletons
        v_flex()
            .w_full()
            .gap_3()
            .child(
                // Top row: Spinner avatar + status info (task + time) horizontally aligned
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        // Agent avatar as spinner with status icon
                        Spinner::new()
                            .icon(status_icon.clone())
                            .with_size(gpui_component::Size::Medium)
                            .color(status_color),
                    )
                    .child(
                        // Status info row: task + time
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .flex_1()
                            .when_some(current_todo, |this, todo| {
                                // Current task indicator
                                this.child(
                                    h_flex()
                                        .items_center()
                                        .gap_1p5()
                                        .px_2()
                                        .py_1()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().muted.opacity(0.5))
                                        .child(
                                            Icon::new(crate::assets::Icon::ListTodo)
                                                .size(px(12.))
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .max_w(px(400.))
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .child(todo),
                                        ),
                                )
                            })
                            .child(
                                // Elapsed time display
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        Icon::new(IconName::Info)
                                            .size(px(12.))
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(elapsed_time),
                                    ),
                            ),
                    ),
            )
            .child(
                // Content skeletons - indented to align with text content
                h_flex()
                    .gap_3()
                    .child(
                        // Spacer to align with content (matches spinner width)
                        div().w(px(24.)),
                    )
                    .child(
                        // Message content skeletons - simulate text lines with varying widths
                        v_flex()
                            .flex_1()
                            .gap_2()
                            .child(
                                Skeleton::new()
                                    .w_full()
                                    .max_w(px(480.))
                                    .h(px(16.))
                                    .rounded(cx.theme().radius),
                            )
                            .child(
                                Skeleton::new()
                                    .w_full()
                                    .max_w(px(420.))
                                    .h(px(16.))
                                    .rounded(cx.theme().radius),
                            )
                            .child(
                                Skeleton::new()
                                    .w_full()
                                    .max_w(px(360.))
                                    .h(px(16.))
                                    .rounded(cx.theme().radius),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl DockPanel for ConversationPanel {
    fn title() -> &'static str {
        "Conversation"
    }

    fn title_key() -> Option<&'static str> {
        Some("conversation.title")
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

        // Add loading skeleton when session is in progress (conditional rendering handled in function)
        children = children.child(self.render_loading_skeleton(cx));

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
