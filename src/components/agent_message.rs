use agent_client_protocol::{ContentBlock, ContentChunk, SessionId};
use gpui::{
    App, AppContext, Context, ElementId, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, text::TextView, v_flex};
use serde::{Deserialize, Serialize};

use crate::assets::get_agent_icon;

/// Extended metadata for agent messages.
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
        let mut total_len = 0usize;
        for chunk in &self.chunks {
            if let ContentBlock::Text(text_content) = &chunk.content {
                total_len = total_len.saturating_add(text_content.text.len());
            }
        }

        if total_len == 0 {
            return "".into();
        }

        let mut text = String::with_capacity(total_len);
        for chunk in &self.chunks {
            if let ContentBlock::Text(text_content) = &chunk.content {
                text.push_str(&text_content.text);
            }
        }

        text.into()
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let agent_name = self.data.agent_name().unwrap_or("Agent");
        let show_thinking = !self.data.is_complete();
        let full_text = self.data.full_text();
        let markdown_id = SharedString::from(format!("{}-markdown", self.id));

        // Get icon based on agent name
        let icon = Icon::new(get_agent_icon(agent_name));

        v_flex()
            .gap_3()
            .w_full()
            // Agent icon and message content
            .child(
                h_flex()
                    .items_start()
                    .gap_2()
                    .child(icon.size(px(16.)).mt_1().text_color(cx.theme().foreground))
                    // Message content with markdown rendering
                    .child(
                        div()
                            .w_full()
                            .child(
                                TextView::markdown(markdown_id, full_text)
                                    // .text_xs()
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .selectable(true)
                                    .pr_3(),
                            )
                            .pr_3(),
                    )
                    .when(show_thinking, |this| {
                        this.child(
                            Icon::new(IconName::LoaderCircle)
                                .size(px(12.))
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
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

    fn update_message(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut AgentMessageData)) {
        self.data.update(cx, move |data, cx| {
            f(data);
            cx.notify();
        });
        cx.notify();
    }

    /// Update the message data completely
    pub fn update_data(&mut self, data: AgentMessageData, cx: &mut Context<Self>) {
        self.update_message(cx, |d| *d = data);
    }

    /// Add a content chunk (for streaming)
    pub fn add_chunk(&mut self, chunk: ContentChunk, cx: &mut Context<Self>) {
        self.update_message(cx, |d| d.chunks.push(chunk));
    }

    /// Append text to the last chunk or create a new one
    pub fn append_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.update_message(cx, |d| {
            let text = text.into();

            match d.chunks.last_mut().map(|chunk| &mut chunk.content) {
                Some(ContentBlock::Text(text_content)) => text_content.text.push_str(&text),
                _ => d.chunks.push(ContentChunk::new(ContentBlock::from(text))),
            }
        });
    }

    /// Mark the message as complete
    pub fn mark_complete(&mut self, cx: &mut Context<Self>) {
        self.update_message(cx, |d| d.meta.is_complete = true);
    }

    /// Set agent name
    pub fn set_agent_name(&mut self, name: impl Into<String>, cx: &mut Context<Self>) {
        self.update_message(cx, |d| d.meta.agent_name = Some(name.into()));
    }

    /// Clear all chunks
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.update_message(cx, |d| {
            d.chunks.clear();
            d.meta.is_complete = false;
        });
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
