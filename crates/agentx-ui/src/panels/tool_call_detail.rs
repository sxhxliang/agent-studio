use gpui::*;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Root, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex,
    scroll::ScrollableElement as _,
    v_flex,
};

use agentx_domain::ToolCall;

use crate::components::{render_tool_call_body, status_badge, tool_status_label, tool_status_tone};

/// A standalone detail panel for a single tool call.
pub struct ToolCallDetailPanel {
    tool_call: ToolCall,
    focus_handle: FocusHandle,
}

impl ToolCallDetailPanel {
    pub fn new(tool_call: ToolCall, cx: &mut Context<Self>) -> Self {
        Self {
            tool_call,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn view(tool_call: ToolCall, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(tool_call, cx))
    }
}

impl Render for ToolCallDetailPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let status = self.tool_call.status;
        let title = self.tool_call.title.clone();
        let kind = format!("{:?}", self.tool_call.kind);
        let body = render_tool_call_body(&self.tool_call.content, theme);

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .gap_3()
            .p_4()
            .bg(theme.background)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Inspector)
                            .small()
                            .text_color(theme.primary),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(kind),
                            ),
                    )
                    .child(status_badge(
                        tool_status_label(status),
                        tool_status_tone(status),
                        theme,
                    )),
            )
            .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(body))
    }
}

impl EventEmitter<PanelEvent> for ToolCallDetailPanel {}

impl Focusable for ToolCallDetailPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ToolCallDetailPanel {
    fn panel_name(&self) -> &'static str {
        "ToolCallDetailPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.tool_call.title.clone()
    }
}

pub fn open_tool_call_detail_window(tool_call: ToolCall, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(720.0), px(560.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let panel = cx.new(|cx| ToolCallDetailPanel::new(tool_call, cx));
        cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
    });
}
