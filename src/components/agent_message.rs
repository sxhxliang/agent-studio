use gpui::{
    div, prelude::FluentBuilder as _, px, App, AppContext, Context, ElementId, Entity, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window,
};

use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, IconName};

/// Agent message content type enumeration
#[derive(Clone, Debug)]
pub enum AgentContentType {
    /// Plain text content
    Text,
    /// Image content
    Image,
    /// Other content types
    Other,
}

/// Agent message content item
#[derive(Clone, Debug)]
pub struct AgentMessageContent {
    /// Content type
    pub content_type: AgentContentType,
    /// Text content (for text type)
    pub text: SharedString,
}

impl AgentMessageContent {
    pub fn text(text: impl Into<SharedString>) -> Self {
        Self {
            content_type: AgentContentType::Text,
            text: text.into(),
        }
    }

    pub fn with_type(mut self, content_type: AgentContentType) -> Self {
        self.content_type = content_type;
        self
    }
}

/// Agent message data structure
#[derive(Clone, Debug)]
pub struct AgentMessageData {
    /// Session ID
    pub session_id: SharedString,
    /// Message content chunks (supports streaming)
    pub chunks: Vec<AgentMessageContent>,
    /// Agent name (optional)
    pub agent_name: Option<SharedString>,
    /// Whether the message is complete
    pub is_complete: bool,
}

impl AgentMessageData {
    pub fn new(session_id: impl Into<SharedString>) -> Self {
        Self {
            session_id: session_id.into(),
            chunks: Vec::new(),
            agent_name: None,
            is_complete: false,
        }
    }

    pub fn with_agent_name(mut self, name: impl Into<SharedString>) -> Self {
        self.agent_name = Some(name.into());
        self
    }

    pub fn with_chunks(mut self, chunks: Vec<AgentMessageContent>) -> Self {
        self.chunks = chunks;
        self
    }

    pub fn add_chunk(mut self, chunk: AgentMessageContent) -> Self {
        self.chunks.push(chunk);
        self
    }

    pub fn complete(mut self) -> Self {
        self.is_complete = true;
        self
    }

    /// Get combined text from all chunks
    pub fn full_text(&self) -> SharedString {
        self.chunks
            .iter()
            .map(|c| c.text.as_ref())
            .collect::<Vec<_>>()
            .join("")
            .into()
    }
}

/// Agent message component
#[derive(IntoElement)]
pub struct AgentMessage {
    id: ElementId,
    data: AgentMessageData,
}

impl AgentMessage {
    pub fn new(id: impl Into<ElementId>, data: AgentMessageData) -> Self {
        Self {
            id: id.into(),
            data,
        }
    }
}

impl RenderOnce for AgentMessage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let agent_name = self
            .data
            .agent_name
            .clone()
            .unwrap_or_else(|| "Agent".into());

        v_flex()
            .gap_3()
            .w_full()
            // Agent icon and label
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Bot)
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(agent_name),
                    )
                    .when(!self.data.is_complete, |this| {
                        // Show thinking indicator when message is not complete
                        this.child(
                            Icon::new(IconName::LoaderCircle)
                                .size(px(12.))
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
            )
            // Message content
            .child(
                div()
                    .pl_6()
                    .w_full()
                    .text_size(px(14.))
                    .text_color(cx.theme().foreground)
                    .line_height(px(22.))
                    .child(self.data.full_text()),
            )
    }
}

/// A stateful wrapper for AgentMessage that can be used as a GPUI view
pub struct AgentMessageView {
    data: Entity<AgentMessageData>,
}

impl AgentMessageView {
    pub fn new(data: AgentMessageData, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let data_entity = cx.new(|_| data);
            Self { data: data_entity }
        })
    }

    /// Update the message data completely
    pub fn update_data(&mut self, data: AgentMessageData, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            *d = data;
            cx.notify();
        });
        cx.notify();
    }

    /// Add a content chunk (for streaming)
    pub fn add_chunk(&mut self, chunk: AgentMessageContent, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.chunks.push(chunk);
            cx.notify();
        });
        cx.notify();
    }

    /// Append text to the last chunk or create a new one
    pub fn append_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            let text_str = text.into();

            // Try to append to the last chunk if it's a text chunk
            if let Some(last_chunk) = d.chunks.last_mut() {
                if matches!(last_chunk.content_type, AgentContentType::Text) {
                    // Append to existing text
                    let new_text = format!("{}{}", last_chunk.text, text_str);
                    last_chunk.text = new_text.into();
                } else {
                    // Create new chunk
                    d.chunks.push(AgentMessageContent::text(text_str));
                }
            } else {
                // Create first chunk
                d.chunks.push(AgentMessageContent::text(text_str));
            }

            cx.notify();
        });
        cx.notify();
    }

    /// Mark the message as complete
    pub fn mark_complete(&mut self, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.is_complete = true;
            cx.notify();
        });
        cx.notify();
    }

    /// Set agent name
    pub fn set_agent_name(&mut self, name: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.agent_name = Some(name.into());
            cx.notify();
        });
        cx.notify();
    }

    /// Clear all chunks
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.chunks.clear();
            d.is_complete = false;
            cx.notify();
        });
        cx.notify();
    }

    /// Get the full text content
    pub fn get_text(&self, cx: &App) -> SharedString {
        self.data.read(cx).full_text()
    }

    /// Check if the message is complete
    pub fn is_complete(&self, cx: &App) -> bool {
        self.data.read(cx).is_complete
    }
}

impl Render for AgentMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        AgentMessage::new("agent-message", data)
    }
}
