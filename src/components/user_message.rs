use gpui::{
    div, prelude::FluentBuilder as _, px, App, AppContext, Context, ElementId, Entity, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window,
};

use gpui_component::{collapsible::Collapsible, h_flex, v_flex, ActiveTheme, Icon, IconName};

/// Message content type enumeration
#[derive(Clone, Debug)]
pub enum MessageContentType {
    /// Plain text content
    Text,
    /// Resource content (file, code, etc.)
    Resource,
}

/// Resource information for message content
#[derive(Clone, Debug)]
pub struct ResourceContent {
    /// Resource URI (e.g., file:///path/to/file)
    pub uri: SharedString,
    /// MIME type of the resource
    pub mime_type: SharedString,
    /// Text content of the resource
    pub text: SharedString,
}

impl ResourceContent {
    pub fn new(
        uri: impl Into<SharedString>,
        mime_type: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            uri: uri.into(),
            mime_type: mime_type.into(),
            text: text.into(),
        }
    }

    /// Extract filename from URI
    pub fn filename(&self) -> SharedString {
        self.uri
            .split('/')
            .last()
            .unwrap_or("unknown")
            .to_string()
            .into()
    }

    /// Get icon based on MIME type
    pub fn icon(&self) -> IconName {
        if self.mime_type.contains("python") {
            IconName::File
        } else if self.mime_type.contains("javascript") || self.mime_type.contains("typescript") {
            IconName::File
        } else if self.mime_type.contains("rust") {
            IconName::File
        } else if self.mime_type.contains("json") {
            IconName::File
        } else {
            IconName::File
        }
    }
}

/// Message content item
#[derive(Clone, Debug)]
pub enum MessageContent {
    /// Plain text content
    Text { text: SharedString },
    /// Resource content (file, code, etc.)
    Resource { resource: ResourceContent },
}

impl MessageContent {
    pub fn text(text: impl Into<SharedString>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn resource(resource: ResourceContent) -> Self {
        Self::Resource { resource }
    }

    pub fn content_type(&self) -> MessageContentType {
        match self {
            Self::Text { .. } => MessageContentType::Text,
            Self::Resource { .. } => MessageContentType::Resource,
        }
    }
}

/// User message data structure
#[derive(Clone, Debug)]
pub struct UserMessageData {
    /// Session ID
    pub session_id: SharedString,
    /// Message content items
    pub contents: Vec<MessageContent>,
}

impl UserMessageData {
    pub fn new(session_id: impl Into<SharedString>) -> Self {
        Self {
            session_id: session_id.into(),
            contents: Vec::new(),
        }
    }

    pub fn with_contents(mut self, contents: Vec<MessageContent>) -> Self {
        self.contents = contents;
        self
    }

    pub fn add_content(mut self, content: MessageContent) -> Self {
        self.contents.push(content);
        self
    }
}

/// Resource item component (collapsible)
#[derive(IntoElement)]
struct ResourceItem {
    id: ElementId,
    resource: ResourceContent,
    open: bool,
}

impl ResourceItem {
    pub fn new(id: impl Into<ElementId>, resource: ResourceContent, open: bool) -> Self {
        Self {
            id: id.into(),
            resource,
            open,
        }
    }
}

impl RenderOnce for ResourceItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let filename = self.resource.filename();
        let line_count = self.resource.text.lines().count();

        Collapsible::new()
            .w_full()
            .gap_2()
            // Header
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
                        Icon::new(self.resource.icon())
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(filename),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} lines", line_count)),
                    )
                    .child(
                        Icon::new(if self.open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .size(px(14.))
                        .text_color(cx.theme().muted_foreground),
                    ),
            )
            // Content - code display
            .when(self.open, |this| {
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
                                .child(self.resource.text),
                        ),
                )
            })
    }
}

/// User message component
#[derive(IntoElement)]
pub struct UserMessage {
    id: ElementId,
    data: UserMessageData,
    resource_states: Vec<bool>, // Track open/close state for each resource
}

impl UserMessage {
    pub fn new(id: impl Into<ElementId>, data: UserMessageData) -> Self {
        let resource_count = data
            .contents
            .iter()
            .filter(|c| matches!(c, MessageContent::Resource { .. }))
            .count();

        Self {
            id: id.into(),
            data,
            resource_states: vec![false; resource_count],
        }
    }

    pub fn with_resource_state(mut self, index: usize, open: bool) -> Self {
        if index < self.resource_states.len() {
            self.resource_states[index] = open;
        }
        self
    }
}

impl RenderOnce for UserMessage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut resource_index = 0;

        v_flex()
            .gap_3()
            .w_full()
            // User icon and label
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
            // Message content
            .child(
                v_flex()
                    .gap_3()
                    .pl_6()
                    .w_full()
                    .children(self.data.contents.into_iter().map(|content| {
                        match content {
                            MessageContent::Text { text } => div()
                                .text_size(px(14.))
                                .text_color(cx.theme().foreground)
                                .line_height(px(22.))
                                .child(text)
                                .into_any_element(),
                            MessageContent::Resource { resource } => {
                                let current_index = resource_index;
                                resource_index += 1;
                                let open = self
                                    .resource_states
                                    .get(current_index)
                                    .copied()
                                    .unwrap_or(false);
                                let id = SharedString::from(format!(
                                    "{}-resource-{}",
                                    self.id, current_index
                                ));

                                ResourceItem::new(id, resource, open).into_any_element()
                            }
                        }
                    })),
            )
    }
}

/// A stateful wrapper for UserMessage that can be used as a GPUI view
pub struct UserMessageView {
    data: Entity<UserMessageData>,
    resource_states: Entity<Vec<bool>>,
}

impl UserMessageView {
    pub fn new(data: UserMessageData, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        let resource_count = data
            .contents
            .iter()
            .filter(|c| matches!(c, MessageContent::Resource { .. }))
            .count();

        cx.new(|cx| {
            let data_entity = cx.new(|_| data);
            let states_entity = cx.new(|_| vec![false; resource_count]);

            Self {
                data: data_entity,
                resource_states: states_entity,
            }
        })
    }

    /// Update the message data
    pub fn update_data(&mut self, data: UserMessageData, cx: &mut Context<Self>) {
        let resource_count = data
            .contents
            .iter()
            .filter(|c| matches!(c, MessageContent::Resource { .. }))
            .count();

        self.data.update(cx, |d, cx| {
            *d = data;
            cx.notify();
        });

        self.resource_states.update(cx, |states, cx| {
            *states = vec![false; resource_count];
            cx.notify();
        });

        cx.notify();
    }

    /// Add content to the message
    pub fn add_content(&mut self, content: MessageContent, cx: &mut Context<Self>) {
        let is_resource = matches!(content, MessageContent::Resource { .. });

        self.data.update(cx, |d, cx| {
            d.contents.push(content);
            cx.notify();
        });

        if is_resource {
            self.resource_states.update(cx, |states, cx| {
                states.push(false);
                cx.notify();
            });
        }

        cx.notify();
    }

    /// Toggle resource open state
    pub fn toggle_resource(&mut self, index: usize, cx: &mut Context<Self>) {
        self.resource_states.update(cx, |states, cx| {
            if let Some(state) = states.get_mut(index) {
                *state = !*state;
                cx.notify();
            }
        });
        cx.notify();
    }

    /// Set resource open state
    pub fn set_resource_open(&mut self, index: usize, open: bool, cx: &mut Context<Self>) {
        self.resource_states.update(cx, |states, cx| {
            if let Some(state) = states.get_mut(index) {
                *state = open;
                cx.notify();
            }
        });
        cx.notify();
    }
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let resource_states = self.resource_states.read(cx).clone();

        let mut msg = UserMessage::new("user-message", data);
        for (index, open) in resource_states.iter().enumerate() {
            msg = msg.with_resource_state(index, *open);
        }
        msg
    }
}
