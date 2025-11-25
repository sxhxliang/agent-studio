use gpui::{
    div, prelude::FluentBuilder as _, px, App, AppContext, ClickEvent, Context, ElementId, Entity,
    IntoElement, ParentElement, Render, RenderOnce, SharedString, Styled, Window,
};

use gpui_component::{
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex, ActiveTheme, Icon, IconName, Sizable,
};

/// Tool call kind enumeration
#[derive(Clone, Debug, PartialEq)]
pub enum ToolCallKind {
    /// Reading files or data
    Read,
    /// Modifying files or content
    Edit,
    /// Removing files or data
    Delete,
    /// Moving or renaming files
    Move,
    /// Searching for information
    Search,
    /// Running commands or code
    Execute,
    /// Internal reasoning or planning
    Think,
    /// Retrieving external data
    Fetch,
    /// Other tool types
    Other,
}

impl Default for ToolCallKind {
    fn default() -> Self {
        Self::Other
    }
}

impl ToolCallKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Move => "move",
            Self::Search => "search",
            Self::Execute => "execute",
            Self::Think => "think",
            Self::Fetch => "fetch",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "read" => Self::Read,
            "edit" => Self::Edit,
            "delete" => Self::Delete,
            "move" => Self::Move,
            "search" => Self::Search,
            "execute" => Self::Execute,
            "think" => Self::Think,
            "fetch" => Self::Fetch,
            _ => Self::Other,
        }
    }

    /// Get the icon for this tool kind
    pub fn icon(&self) -> IconName {
        match self {
            Self::Read => IconName::File,
            Self::Edit => IconName::Replace,
            Self::Delete => IconName::Delete,
            Self::Move => IconName::ArrowRight,
            Self::Search => IconName::Search,
            Self::Execute => IconName::SquareTerminal,
            Self::Think => IconName::Bot,
            Self::Fetch => IconName::Globe,
            Self::Other => IconName::Ellipsis,
        }
    }
}

/// Tool call status enumeration
#[derive(Clone, Debug, PartialEq)]
pub enum ToolCallStatus {
    /// Tool call is pending execution
    Pending,
    /// Tool call is currently executing
    InProgress,
    /// Tool call completed successfully
    Completed,
    /// Tool call failed
    Failed,
}

impl Default for ToolCallStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl ToolCallStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Pending,
        }
    }

    /// Get the icon for this status
    pub fn icon(&self) -> IconName {
        match self {
            Self::Pending => IconName::Dash,
            Self::InProgress => IconName::LoaderCircle,
            Self::Completed => IconName::CircleCheck,
            Self::Failed => IconName::CircleX,
        }
    }
}

/// Tool call content
#[derive(Clone, Debug)]
pub struct ToolCallContent {
    pub text: SharedString,
}

impl ToolCallContent {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

/// A tool call item data structure
#[derive(Clone, Debug)]
pub struct ToolCallData {
    /// Unique identifier for this tool call
    pub tool_call_id: SharedString,
    /// Human-readable title describing what the tool is doing
    pub title: SharedString,
    /// The category of tool being invoked
    pub kind: ToolCallKind,
    /// The current execution status
    pub status: ToolCallStatus,
    /// Content produced by the tool call
    pub content: Vec<ToolCallContent>,
}

impl ToolCallData {
    pub fn new(tool_call_id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            title: title.into(),
            kind: ToolCallKind::default(),
            status: ToolCallStatus::default(),
            content: Vec::new(),
        }
    }

    pub fn with_kind(mut self, kind: ToolCallKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_status(mut self, status: ToolCallStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_content(mut self, content: Vec<ToolCallContent>) -> Self {
        self.content = content;
        self
    }

    pub fn has_content(&self) -> bool {
        !self.content.is_empty()
    }
}

/// Tool call item component
#[derive(IntoElement)]
pub struct ToolCallItem {
    id: ElementId,
    data: ToolCallData,
    open: bool,
    on_toggle: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl ToolCallItem {
    pub fn new(id: impl Into<ElementId>, data: ToolCallData) -> Self {
        Self {
            id: id.into(),
            data,
            open: false,
            on_toggle: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ToolCallItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let has_content = self.data.has_content();
        let status_color = match self.data.status {
            ToolCallStatus::Completed => cx.theme().green,
            ToolCallStatus::Failed => cx.theme().red,
            ToolCallStatus::InProgress => cx.theme().accent,
            ToolCallStatus::Pending => cx.theme().muted_foreground,
        };

        let on_toggle = self.on_toggle;
        let id = self.id;
        let open = self.open;

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
                        Icon::new(self.data.kind.icon())
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        // Title
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(self.data.title.clone()),
                    )
                    .child(
                        // Status icon
                        Icon::new(self.data.status.icon())
                            .size(px(14.))
                            .text_color(status_color),
                    )
                    .when(has_content, |this| {
                        // Add expand/collapse button only if there's content
                        let btn = Button::new(SharedString::from(format!("{}-toggle", id)))
                            .icon(if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall();

                        let btn = if let Some(handler) = on_toggle {
                            btn.on_click(move |ev, window, cx| {
                                handler(ev, window, cx);
                            })
                        } else {
                            btn
                        };

                        this.child(btn)
                    }),
            )
            // Content - only visible when open and has content
            .when(has_content, |this| {
                this.content(
                    v_flex()
                        .gap_1()
                        .p_3()
                        .pl_8()
                        .children(self.data.content.iter().map(|content| {
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .line_height(px(18.))
                                .child(content.text.clone())
                        })),
                )
            })
    }
}

/// A stateful wrapper for ToolCallItem that can be used as a GPUI view
pub struct ToolCallItemView {
    data: Entity<ToolCallData>,
    open: bool,
}

impl ToolCallItemView {
    pub fn new(data: ToolCallData, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let data_entity = cx.new(|_| data);
            Self {
                data: data_entity,
                open: false,
            }
        })
    }

    /// Update the tool call data
    pub fn update_data(&mut self, data: ToolCallData, cx: &mut App) {
        self.data.update(cx, |d, cx| {
            *d = data;
            cx.notify();
        });
    }

    /// Update the status
    pub fn update_status(&mut self, status: ToolCallStatus, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.status = status;
            cx.notify();
        });
        cx.notify();
    }

    /// Add content to the tool call
    pub fn add_content(&mut self, content: ToolCallContent, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.content.push(content);
            cx.notify();
        });
        cx.notify();
    }

    /// Set content for the tool call
    pub fn set_content(&mut self, content: Vec<ToolCallContent>, cx: &mut Context<Self>) {
        self.data.update(cx, |d, cx| {
            d.content = content;
            cx.notify();
        });
        cx.notify();
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
}

impl Render for ToolCallItemView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let id = SharedString::from(format!("tool-call-{}", data.tool_call_id));
        let open = self.open;

        ToolCallItem::new(id, data)
            .open(open)
            .on_toggle(cx.listener(|this, _ev, _window, cx| {
                this.toggle(cx);
            }))
    }
}
