use agent_client_protocol::{ContentBlock, ContentChunk, SessionId};
use gpui::{
    App, AppContext, Context, ElementId, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, text::TextView, v_flex};
use serde::{Deserialize, Serialize};

/// Extended metadata for agent messages (stored in ContentChunk's meta field)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageMeta {
    /// Agent name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// Whether the message is complete
    #[serde(default)]
    pub is_complete: bool,
}

/// Agent message data structure based on ACP's ContentChunk
#[derive(Clone, Debug)]
pub struct AgentMessageData {
    /// Session ID
    pub session_id: SessionId,
    /// Message content chunks (supports streaming)
    pub chunks: Vec<ContentChunk>,
    /// Extended metadata (agent_name, is_complete, etc.)
    pub meta: AgentMessageMeta,
}

impl AgentMessageData {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            chunks: Vec::new(),
            meta: AgentMessageMeta::default(),
        }
    }

    pub fn with_agent_name(mut self, name: impl Into<String>) -> Self {
        self.meta.agent_name = Some(name.into());
        self
    }

    pub fn with_chunks(mut self, chunks: Vec<ContentChunk>) -> Self {
        self.chunks = chunks;
        self
    }

    pub fn add_chunk(mut self, chunk: ContentChunk) -> Self {
        self.chunks.push(chunk);
        self
    }

    /// Add a text chunk
    pub fn add_text(mut self, text: impl Into<String>) -> Self {
        self.chunks
            .push(ContentChunk::new(ContentBlock::from(text.into())));
        self
    }

    pub fn complete(mut self) -> Self {
        self.meta.is_complete = true;
        self
    }

    /// Get combined text from all text chunks
    pub fn full_text(&self) -> SharedString {
        self.chunks
            .iter()
            .filter_map(|chunk| match &chunk.content {
                ContentBlock::Text(text_content) => Some(text_content.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
            .into()
    }

    /// Check if the message is complete
    pub fn is_complete(&self) -> bool {
        self.meta.is_complete
    }

    /// Get agent name
    pub fn agent_name(&self) -> Option<&str> {
        self.meta.agent_name.as_deref()
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let agent_name: SharedString = self
            .data
            .meta
            .agent_name
            .clone()
            .map(|s| s.into())
            .unwrap_or_else(|| "Agent".into());
        let is_complete = self.data.is_complete();
        let full_text = self.data.full_text();
        let markdown_id = SharedString::from(format!("{}-markdown", self.id));

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
                    .when(!is_complete, |this| {
                        // Show thinking indicator when message is not complete
                        this.child(
                            Icon::new(IconName::LoaderCircle)
                                .size(px(12.))
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
            )
            // Message content with markdown rendering
            .child(
                div().pl_6().w_full().child(
                    TextView::markdown(markdown_id, full_text, window, cx)
                        .text_size(px(14.))
                        .text_color(cx.theme().foreground)
                        .line_height(px(22.))
                        .selectable(true),
                ),
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
    pub fn add_chunk(&mut self, chunk: ContentChunk, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.chunks.push(chunk);
            cx.notify();
        });
        cx.notify();
    }

    /// Append text to the last chunk or create a new one
    pub fn append_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            let text_str = text.into();

            // Try to append to the last chunk if it's a text chunk
            if let Some(last_chunk) = d.chunks.last_mut() {
                if let ContentBlock::Text(ref mut text_content) = last_chunk.content {
                    // Append to existing text
                    text_content.text.push_str(&text_str);
                } else {
                    // Create new chunk
                    d.chunks
                        .push(ContentChunk::new(ContentBlock::from(text_str)));
                }
            } else {
                // Create first chunk
                d.chunks
                    .push(ContentChunk::new(ContentBlock::from(text_str)));
            }

            cx.notify();
        });
        cx.notify();
    }

    /// Mark the message as complete
    pub fn mark_complete(&mut self, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.meta.is_complete = true;
            cx.notify();
        });
        cx.notify();
    }

    /// Set agent name
    pub fn set_agent_name(&mut self, name: impl Into<String>, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.meta.agent_name = Some(name.into());
            cx.notify();
        });
        cx.notify();
    }

    /// Clear all chunks
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.chunks.clear();
            d.meta.is_complete = false;
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
        self.data.read(cx).is_complete()
    }
}

impl Render for AgentMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        AgentMessage::new("agent-message", data)
    }
}
