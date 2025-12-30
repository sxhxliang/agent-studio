use gpui::{
    App, AppContext, Context, ElementId, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use agent_client_protocol::{
    ContentBlock, EmbeddedResource, EmbeddedResourceResource, ResourceLink, SessionId,
    TextResourceContents,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};

/// User message data structure based on ACP's PromptRequest format
#[derive(Clone, Debug)]
pub struct UserMessageData {
    /// Session ID
    pub session_id: SessionId,
    /// Message content blocks (following ACP ContentBlock format)
    pub contents: Vec<ContentBlock>,
}

impl UserMessageData {
    pub fn new(session_id: impl Into<SessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            contents: Vec::new(),
        }
    }

    pub fn with_contents(mut self, contents: Vec<ContentBlock>) -> Self {
        self.contents = contents;
        self
    }

    pub fn add_content(mut self, content: ContentBlock) -> Self {
        self.contents.push(content);
        self
    }

    /// Add a text content block
    pub fn add_text(mut self, text: impl Into<String>) -> Self {
        self.contents.push(ContentBlock::from(text.into()));
        self
    }

    /// Add a resource link content block
    pub fn add_resource_link(mut self, name: impl Into<String>, uri: impl Into<String>) -> Self {
        self.contents
            .push(ContentBlock::ResourceLink(ResourceLink::new(name, uri)));
        self
    }

    /// Add an embedded resource content block
    pub fn add_embedded_resource(
        mut self,
        uri: impl Into<String>,
        text: impl Into<String>,
        mime_type: Option<String>,
    ) -> Self {
        let mut resource = TextResourceContents::new(text, uri);
        if let Some(mt) = mime_type {
            resource = resource.mime_type(mt);
        }
        self.contents
            .push(ContentBlock::Resource(EmbeddedResource::new(
                EmbeddedResourceResource::TextResourceContents(resource),
            )));
        self
    }
}

/// Helper to extract display information from ContentBlock
pub fn get_resource_info(content: &ContentBlock) -> Option<ResourceInfo> {
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
                text: None, // Blob content is not displayable as text
            }),
            // Handle future variants
            _ => None,
        },
        _ => None,
    }
}

/// Extract filename from URI
fn extract_filename(uri: &str) -> String {
    uri.split('/').next_back().unwrap_or("unknown").to_string()
}

/// Resource information for display
pub struct ResourceInfo {
    pub uri: SharedString,
    pub name: SharedString,
    pub mime_type: Option<SharedString>,
    pub text: Option<SharedString>,
}

impl ResourceInfo {
    /// Get icon based on MIME type
    fn icon(&self) -> IconName {
        if let Some(ref mime) = self.mime_type {
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
}

/// Resource item component (collapsible) - stateful version
pub struct ResourceItem {
    resource: ResourceInfo,
    open: bool,
}

impl ResourceItem {
    pub fn new(resource: ResourceInfo) -> Self {
        Self {
            resource,
            open: false,
        }
    }

    /// Toggle the open/close state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        cx.notify();
    }
}

impl Render for ResourceItem {
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

        Collapsible::new()
            .open(is_open)
            .w_full()
            .gap_2()
            // Header - with toggle button
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
                            .child(resource_name.clone()),
                    )
                    .when(line_count > 0, |this: gpui::Div| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} lines", line_count)),
                        )
                    })
                    .when(has_content, |this| {
                        // Add toggle button only if there's content
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
            // Content - code display (only if we have text)
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

/// User message component
#[derive(IntoElement)]
pub struct UserMessage {
    id: ElementId,
    data: UserMessageData,
}

impl UserMessage {
    pub fn new(id: impl Into<ElementId>, data: UserMessageData) -> Self {
        Self {
            id: id.into(),
            data,
        }
    }
}

impl RenderOnce for UserMessage {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
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
            .child(v_flex().gap_3().pl_6().w_full().children(
                self.data.contents.into_iter().filter_map(|content| {
                    match &content {
                        ContentBlock::Text(text_content) => Some(
                            div()
                                .text_size(px(14.))
                                .text_color(cx.theme().foreground)
                                .line_height(px(22.))
                                .child(text_content.text.clone())
                                .into_any_element(),
                        ),
                        // Skip resources in simple render - use UserMessageView for interactive resources
                        ContentBlock::ResourceLink(_) | ContentBlock::Resource(_) => None,
                        // Skip other content types for now (Image, Audio)
                        _ => None,
                    }
                }),
            ))
    }
}

/// A stateful wrapper for UserMessage that can be used as a GPUI view
pub struct UserMessageView {
    pub(crate) data: Entity<UserMessageData>,
    pub(crate) resource_items: Vec<Entity<ResourceItem>>,
}

impl UserMessageView {
    pub fn new(data: UserMessageData, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let data_entity = cx.new(|_| data.clone());

            // Create ResourceItem entities for each resource in the data
            let resource_items: Vec<Entity<ResourceItem>> = data
                .contents
                .iter()
                .filter_map(|content| get_resource_info(content))
                .map(|resource_info| cx.new(|_| ResourceItem::new(resource_info)))
                .collect();

            Self {
                data: data_entity,
                resource_items,
            }
        })
    }

    /// Update the message data
    pub fn update_data(&mut self, data: UserMessageData, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            *d = data.clone();
            cx.notify();
        });

        // Recreate resource items
        self.resource_items = data
            .contents
            .iter()
            .filter_map(|content| get_resource_info(content))
            .map(|resource_info| cx.new(|_| ResourceItem::new(resource_info)))
            .collect();

        cx.notify();
    }

    /// Add content to the message
    pub fn add_content(&mut self, content: ContentBlock, cx: &mut Context<Self>) {
        let is_resource = matches!(
            content,
            ContentBlock::ResourceLink(_) | ContentBlock::Resource(_)
        );

        self.data.update(cx, |d, cx| {
            d.contents.push(content.clone());
            cx.notify();
        });

        // If it's a resource, create a new ResourceItem entity
        if is_resource {
            if let Some(resource_info) = get_resource_info(&content) {
                let item = cx.new(|_| ResourceItem::new(resource_info));
                self.resource_items.push(item);
            }
        }

        cx.notify();
    }

    /// Toggle resource open state by index
    pub fn toggle_resource(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(item) = self.resource_items.get(index) {
            item.update(cx, |item, cx| {
                item.toggle(cx);
            });
        }
    }

    /// Set resource open state by index
    pub fn set_resource_open(&mut self, index: usize, open: bool, cx: &mut Context<Self>) {
        if let Some(item) = self.resource_items.get(index) {
            item.update(cx, |item, cx| {
                item.set_open(open, cx);
            });
        }
    }
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
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
                    .bg(cx.theme().secondary)
                    .border_1()
                    .border_color(cx.theme().border)
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
                                if get_resource_info(&content).is_some() {
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
                            // Skip other content types for now (Image, Audio)
                            _ => None,
                        }
                    })),
            )
    }
}
