use gpui::{
    div, prelude::*, px, App, Context, ElementId, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex,
    scroll::ScrollbarAxis,
    v_flex, ActiveTheme, Icon, IconName, Sizable, StyledExt,
};

// Use the published ACP schema crate
use agent_client_protocol_schema::{
    ContentBlock, ContentChunk, EmbeddedResourceResource, Plan, SessionUpdate, ToolCall,
    ToolCallContent, ToolCallStatus, ToolKind,
};

use crate::{
    dock_panel::DockPanel, AgentMessage, AgentMessageData, AgentTodoList, AppState, UserMessageData,
};

// ============================================================================
// Helper Traits and Functions
// ============================================================================

trait ToolKindExt {
    fn icon(&self) -> IconName;
}

impl ToolKindExt for ToolKind {
    fn icon(&self) -> IconName {
        match self {
            ToolKind::Read => IconName::File,
            ToolKind::Edit => IconName::Replace,
            ToolKind::Delete => IconName::Delete,
            ToolKind::Move => IconName::ArrowRight,
            ToolKind::Search => IconName::Search,
            ToolKind::Execute => IconName::SquareTerminal,
            ToolKind::Think => IconName::Bot,
            ToolKind::Fetch => IconName::Globe,
            ToolKind::SwitchMode => IconName::ArrowRight,
            ToolKind::Other | _ => IconName::Ellipsis,
        }
    }
}

trait ToolCallStatusExt {
    fn icon(&self) -> IconName;
}

impl ToolCallStatusExt for ToolCallStatus {
    fn icon(&self) -> IconName {
        match self {
            ToolCallStatus::Pending => IconName::Dash,
            ToolCallStatus::InProgress => IconName::LoaderCircle,
            ToolCallStatus::Completed => IconName::CircleCheck,
            ToolCallStatus::Failed => IconName::CircleX,
            _ => IconName::Dash,
        }
    }
}

fn extract_filename(uri: &str) -> String {
    uri.split('/').next_back().unwrap_or("unknown").to_string()
}

fn get_file_icon(mime_type: &Option<String>) -> IconName {
    if let Some(ref mime) = mime_type {
        if mime.contains("python")
            || mime.contains("javascript")
            || mime.contains("typescript")
            || mime.contains("rust")
            || mime.contains("json")
        {
            return IconName::File;
        }
    }
    IconName::File
}

// ============================================================================
// Resource Info Structure
// ============================================================================

#[derive(Clone)]
struct ResourceInfo {
    uri: SharedString,
    name: SharedString,
    mime_type: Option<SharedString>,
    text: Option<SharedString>,
}

impl ResourceInfo {
    fn from_content_block(content: &ContentBlock) -> Option<Self> {
        match content {
            ContentBlock::ResourceLink(link) => Some(ResourceInfo {
                uri: link.uri.clone().into(),
                name: link.name.clone().into(),
                mime_type: link.mime_type.clone().map(|s| s.into()),
                text: None,
            }),
            ContentBlock::Resource(embedded) => match &embedded.resource {
                EmbeddedResourceResource::TextResourceContents(text_res) => Some(ResourceInfo {
                    uri: text_res.uri.clone().into(),
                    name: extract_filename(&text_res.uri).into(),
                    mime_type: text_res.mime_type.clone().map(|s| s.into()),
                    text: Some(text_res.text.clone().into()),
                }),
                EmbeddedResourceResource::BlobResourceContents(blob_res) => Some(ResourceInfo {
                    uri: blob_res.uri.clone().into(),
                    name: extract_filename(&blob_res.uri).into(),
                    mime_type: blob_res.mime_type.clone().map(|s| s.into()),
                    text: None,
                }),
                _ => None,
            },
            _ => None,
        }
    }
}

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
    fn apply_update(&mut self, update_fields: agent_client_protocol_schema::ToolCallUpdateFields, cx: &mut Context<Self>) {
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
    AgentMessage(String, AgentMessageData),
    AgentThought(String),
    Plan(Plan),
    ToolCall(Entity<ToolCallItemState>),
    // Simple text updates for commands and mode changes
    InfoUpdate(String),
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
}

impl ConversationPanelAcp {
    /// Create a new panel with mock data (for demo purposes)
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        log::info!("🚀 Creating ConversationPanelAcp view");
        let entity = cx.new(|cx| Self::new(window, cx));
        Self::subscribe_to_updates(&entity, None, cx);
        log::info!("✅ ConversationPanelAcp view created and subscribed");
        entity
    }

    /// Create a new panel for a specific session (no mock data)
    pub fn view_for_session(session_id: String, cx: &mut App) -> Entity<Self> {
        log::info!(
            "🚀 Creating ConversationPanelAcp for session: {}",
            session_id
        );
        let entity = cx.new(|cx| Self::new_for_session(session_id.clone(), cx));
        Self::subscribe_to_updates(&entity, Some(session_id.clone()), cx);
        log::info!(
            "✅ ConversationPanelAcp created for session: {}",
            session_id
        );
        entity
    }

    fn new(_window: &mut Window, cx: &mut App) -> Self {
        log::info!("🔧 Initializing ConversationPanelAcp (new)");
        let focus_handle = cx.focus_handle();
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
        };

        panel
    }

    fn new_for_session(session_id: String, cx: &mut App) -> Self {
        log::info!(
            "🔧 Initializing ConversationPanelAcp for session: {}",
            session_id
        );
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            rendered_items: Vec::new(),
            next_index: 0,
            session_id: Some(session_id),
        }
    }

    /// Subscribe to session updates after the entity is created
    pub fn subscribe_to_updates(
        entity: &Entity<Self>,
        session_filter: Option<String>,
        cx: &mut App,
    ) {
        let weak_entity = entity.downgrade();
        let session_bus = AppState::global(cx).session_bus.clone();

        // Create unbounded channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<SessionUpdate>();

        // Clone session_filter for logging after the closure
        let filter_log = session_filter.clone();

        // Subscribe to session bus, send updates to channel in callback
        session_bus.subscribe(move |event| {
            // Filter by session_id if specified
            if let Some(ref filter_id) = session_filter {
                if &event.session_id != filter_id {
                    return; // Skip this update
                }
            }

            // This callback runs in agent I/O thread
            let _ = tx.send((*event.update).clone());
            log::info!(
                "Session update sent to channel: session_id={}",
                event.session_id
            );
        });

        // Clone session_filter for logging inside the closure
        let filter_log_inner = filter_log.clone();

        // Spawn background task to receive from channel and update entity
        cx.spawn(async move |cx| {
            log::info!("Starting background task for session: {}", filter_log_inner.as_deref().unwrap_or("all"));
            while let Some(update) = rx.recv().await {
                log::info!("Background task received update for session: {}", filter_log_inner.as_deref().unwrap_or("all"));
                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            let index = this.next_index;
                            this.next_index += 1;
                            log::info!("Processing update type: {:?}", update);
                            Self::add_update_to_list(&mut this.rendered_items, update, index, cx);
                            cx.notify(); // Trigger re-render immediately
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
            log::info!("Background task ended for session: {}", filter_log_inner.as_deref().unwrap_or("all"));
        })
        .detach();

        let filter_log_str = filter_log.as_deref().unwrap_or("all sessions");
        log::info!("Subscribed to session bus for: {}", filter_log_str);
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
                log::debug!("  └─ Creating UserMessage");
                items.push(Self::create_user_message(chunk, index, cx));
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                log::debug!("  └─ Creating AgentMessage");
                let data = Self::create_agent_message_data(chunk, index);
                items.push(RenderedItem::AgentMessage(
                    format!("agent-msg-{}", index),
                    data,
                ));
            }
            SessionUpdate::AgentThoughtChunk(chunk) => {
                log::debug!("  └─ Creating AgentThought");
                let text = Self::extract_text_from_content(&chunk.content);
                items.push(RenderedItem::AgentThought(text));
            }
            SessionUpdate::ToolCall(tool_call) => {
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
                        let matches = entity_clone.read(cx).tool_call_id() == &tool_call_update.tool_call_id;

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
                            log::error!(
                                "     ✗ Failed to create ToolCall from update: {:?}",
                                e
                            );
                        }
                    }
                }
            }
            SessionUpdate::Plan(plan) => {
                log::debug!("  └─ Creating Plan with {} entries", plan.entries.len());
                items.push(RenderedItem::Plan(plan));
            }
            SessionUpdate::AvailableCommandsUpdate(commands_update) => {
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
        let json_str = include_str!("../mock_conversation_acp.json");
        match serde_json::from_str::<Vec<SessionUpdate>>(json_str) {
            Ok(updates) => {
                log::info!("✅ Successfully loaded {} mock conversation updates", updates.len());
                updates
            },
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

        children.scrollable(ScrollbarAxis::Vertical).size_full()
    }
}
