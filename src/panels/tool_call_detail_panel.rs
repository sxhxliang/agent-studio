use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use gpui_component::{h_flex, text::TextView, v_flex, ActiveTheme, Icon, IconName};

use agent_client_protocol::{ContentBlock, ToolCall, ToolCallContent};

use crate::panels::dock_panel::DockPanel;

/// Panel that displays detailed tool call content
pub struct ToolCallDetailPanel {
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    /// The tool call to display
    tool_call: Option<ToolCall>,
}

impl ToolCallDetailPanel {
    pub fn new(_window: &mut Window, cx: &mut App) -> Self {
        let focus_handle = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();

        Self {
            focus_handle,
            scroll_handle,
            tool_call: None,
        }
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let panel = Self::new(window, cx);
            Self::subscribe_to_tool_call_updates(cx);
            panel
        })
    }
    /// Create a new panel for a specific session (no mock data)
    // pub fn view_for_tool_call(tool_call: ToolCall, window: &mut Window, cx: &mut App) -> Entity<Self> {
    //     // log::info!(
    //     //     "🚀 Creating ConversationPanel for session: {}",
    //     //     session_id
    //     // );
    //     let entity = cx.new(|cx| Self::new_for_session(session_id.clone(), window, cx));
    //     entity
    // }
    /// Update the tool call to display
    pub fn update_tool_call(&mut self, tool_call: ToolCall, cx: &mut Context<Self>) {
        self.tool_call = Some(tool_call);
        cx.notify();
    }
    /// Setup the tool call to display
    pub fn set_tool_call(&mut self, tool_call: ToolCall) {
        self.tool_call = Some(tool_call);
    }

    /// Clear the displayed tool call
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.tool_call = None;
        cx.notify();
    }

    /// Subscribe to the global selected tool call state
    pub fn subscribe_to_tool_call_updates(cx: &mut Context<Self>) {
        let app_state = crate::AppState::global(cx);
        let selected_tool_call = app_state.selected_tool_call.clone();

        cx.observe(&selected_tool_call, |this, tool_call_entity, cx| {
            let tool_call = tool_call_entity.read(cx);
            if let Some(tc) = tool_call.clone() {
                this.update_tool_call(tc, cx);
            } else {
                this.clear(cx);
            }
        })
        .detach();
    }

    /// Render content based on ToolCallContent type
    fn render_content(
        &self,
        content: &ToolCallContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match content {
            ToolCallContent::Content(c) => match &c.content {
                ContentBlock::Text(text) => {
                    let markdown_id = SharedString::from(format!(
                        "detail-{}-markdown",
                        self.tool_call.as_ref().unwrap().tool_call_id
                    ));
                    div()
                        .w_full()
                        .p_4()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().secondary)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_family("Monaco, 'Courier New', monospace")
                                .text_color(cx.theme().foreground)
                                .line_height(px(20.))
                                .whitespace_normal()
                                .child(
                                    TextView::markdown(markdown_id, text.text.clone(), window, cx)
                                        .text_size(px(14.))
                                        .text_color(cx.theme().foreground)
                                        .line_height(px(22.))
                                        .selectable(true),
                                ),
                        )
                        .into_any_element()
                }
                _ => div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child("Unsupported content type")
                    .into_any_element(),
            },
            ToolCallContent::Diff(diff) => v_flex()
                .w_full()
                .gap_3()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::File)
                                .size(px(16.))
                                .text_color(cx.theme().accent),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(diff.path.display().to_string()),
                        ),
                )
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .when(diff.old_text.is_some(), |this| {
                            this.child(
                                v_flex()
                                    .w_full()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().red)
                                            .child("- Old"),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .p_3()
                                            .rounded(cx.theme().radius)
                                            .bg(cx.theme().secondary)
                                            .border_1()
                                            .border_color(cx.theme().red.opacity(0.3))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .font_family("Monaco, 'Courier New', monospace")
                                                    .text_color(cx.theme().foreground)
                                                    .line_height(px(18.))
                                                    .whitespace_normal()
                                                    .child(
                                                        diff.old_text.clone().unwrap_or_default(),
                                                    ),
                                            ),
                                    ),
                            )
                        })
                        .child(
                            v_flex()
                                .w_full()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().green)
                                        .child("+ New"),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .p_3()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().secondary)
                                        .border_1()
                                        .border_color(cx.theme().green.opacity(0.3))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_family("Monaco, 'Courier New', monospace")
                                                .text_color(cx.theme().foreground)
                                                .line_height(px(18.))
                                                .whitespace_normal()
                                                .child(diff.new_text.clone()),
                                        ),
                                ),
                        ),
                )
                .into_any_element(),
            ToolCallContent::Terminal(terminal) => v_flex()
                .w_full()
                .gap_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(IconName::SquareTerminal)
                                .size(px(16.))
                                .text_color(cx.theme().accent),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(format!("Terminal: {}", terminal.terminal_id)),
                        ),
                )
                .child(
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
                                .child("Terminal output display"),
                        ),
                )
                .into_any_element(),
            _ => div()
                .text_size(px(13.))
                .text_color(cx.theme().muted_foreground)
                .child("Unknown content type")
                .into_any_element(),
        }
    }
}

impl DockPanel for ToolCallDetailPanel {
    fn title() -> &'static str {
        "Tool Call Details"
    }

    fn description() -> &'static str {
        "View detailed tool call content"
    }

    fn closable() -> bool {
        true
    }

    fn zoomable() -> Option<gpui_component::dock::PanelControl> {
        Some(gpui_component::dock::PanelControl::default())
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> gpui::Pixels {
        px(0.)
    }
}

impl Focusable for ToolCallDetailPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ToolCallDetailPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll_handle = self.scroll_handle.clone();

        div()
            .size_full()
            // .track_focus(&self.focus_handle)
            .child(
                div()
                    .id("tool-call-detail-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll_handle)
                    .child(
                        v_flex()
                            .w_full()
                            .p_4()
                            .gap_4()
                            .when_some(self.tool_call.as_ref(), |this, tool_call| {
                                this.child(
                                    v_flex()
                                        .w_full()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Icon::new(IconName::File)
                                                        .size(px(18.))
                                                        .text_color(cx.theme().accent),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(16.))
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(cx.theme().foreground)
                                                        .child(tool_call.title.clone()),
                                                ),
                                        )
                                        .child(div().w_full().h(px(1.)).bg(cx.theme().border))
                                        .children(tool_call.content.iter().map(|content| {
                                            self.render_content(content, window, cx)
                                        })),
                                )
                            })
                            .when(self.tool_call.is_none(), |this| {
                                this.child(
                                    div().flex_1().flex().items_center().justify_center().child(
                                        div()
                                            .text_size(px(14.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Click on a tool call to view details"),
                                    ),
                                )
                            }),
                    ),
            )
    }
}
