use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use agent_client_protocol::{self as acp, ToolCall, ToolCallContent, ToolCallStatus, ToolKind};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};

/// Helper trait to get icon for ToolKind
pub trait ToolKindExt {
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

/// Helper trait to get icon for ToolCallStatus
pub trait ToolCallStatusExt {
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

/// Helper to extract text from ToolCallContent
fn extract_text_from_content(content: &ToolCallContent) -> Option<String> {
    match content {
        ToolCallContent::Content(c) => match &c.content {
            acp::ContentBlock::Text(text) => Some(text.text.clone()),
            _ => None,
        },
        ToolCallContent::Diff(diff) => Some(format!(
            "Modified: {:?}\n{} -> {}",
            diff.path,
            diff.old_text.as_deref().unwrap_or("<new file>"),
            diff.new_text
        )),
        ToolCallContent::Terminal(terminal) => Some(format!("Terminal: {}", terminal.terminal_id)),
        _ => None,
    }
}

/// Tool call item component based on ACP's ToolCall - stateful version
pub struct ToolCallItem {
    tool_call: ToolCall,
    open: bool,
}

impl ToolCallItem {
    pub fn new(tool_call: ToolCall) -> Self {
        Self {
            tool_call,
            open: false,
        }
    }

    /// Toggle the open state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        cx.notify();
    }
    // pub fn update_tool_title(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
    //     self.tool_call.title = title;
    //     cx.notify();
    // }
    /// Update the tool call data
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        log::debug!("tool_call: {:?}", &tool_call);
        self.tool_call = tool_call;
        cx.notify();
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.tool_call.status = status;
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.tool_call.content.push(content);
        cx.notify();
    }

    fn has_content(&self) -> bool {
        !self.tool_call.content.is_empty()
    }
}

impl Render for ToolCallItem {
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

        Collapsible::new()
            .open(open)
            .w_full()
            .gap_2()
            // Header - always visible
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
                    .child(
                        // Kind icon
                        Icon::new(self.tool_call.kind.icon())
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        // Title
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(self.tool_call.title.clone()),
                    )
                    .child(
                        // Status icon
                        Icon::new(self.tool_call.status.icon())
                            .size(px(14.))
                            .text_color(status_color),
                    )
                    .when(has_content, |this| {
                        // Add expand/collapse button only if there's content
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
            // Content - only visible when open and has content
            .when(has_content, |this| {
                this.content(v_flex().gap_1().p_3().pl_8().children(
                    self.tool_call.content.iter().filter_map(|content| {
                        extract_text_from_content(content).map(|text| {
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .line_height(px(18.))
                                .child(text)
                        })
                    }),
                ))
            })
    }
}

/// A stateful wrapper for ToolCallItem that can be used as a GPUI view
pub struct ToolCallItemView {
    item: Entity<ToolCallItem>,
}

impl ToolCallItemView {
    pub fn new(tool_call: ToolCall, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let item = cx.new(|_| ToolCallItem::new(tool_call));
            Self { item }
        })
    }

    /// Update the tool call data
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.update_tool_call(tool_call, cx);
        });
        cx.notify();
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.update_status(status, cx);
        });
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.add_content(content, cx);
        });
        cx.notify();
    }

    /// Set content for the tool call
    pub fn set_content(&mut self, content: Vec<ToolCallContent>, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.tool_call.content = content;
            cx.notify();
        });
        cx.notify();
    }

    /// Toggle the open state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.toggle(cx);
        });
        cx.notify();
    }

    /// Set the open state
    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.item.update(cx, |item, cx| {
            item.set_open(open, cx);
        });
        cx.notify();
    }
}

impl Render for ToolCallItemView {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.item.clone()
    }
}
