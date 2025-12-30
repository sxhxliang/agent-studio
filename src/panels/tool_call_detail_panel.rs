use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::*, px,
};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, text::TextView, v_flex};

use gpui_component::highlighter::Language;
use similar::{ChangeTag, TextDiff};

use agent_client_protocol::{ContentBlock, ToolCall, ToolCallContent};

use crate::panels::dock_panel::DockPanel;

/// Represents a single line in a diff view
#[derive(Debug, Clone)]
enum DiffLine {
    /// Unchanged line (context)
    Context {
        line: String,
        old_num: usize,
        new_num: usize,
    },
    /// Line added in new version
    Insert { line: String, new_num: usize },
    /// Line deleted from old version
    Delete { line: String, old_num: usize },
}

/// Represents a display item in the diff view (can be a line or a collapsed section)
#[derive(Debug, Clone)]
enum DiffDisplayItem {
    /// A regular diff line
    Line(DiffLine),
    /// A collapsed section of unchanged lines
    Collapsed {
        start_old: usize,
        start_new: usize,
        count: usize,
    },
}

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

    /// Compute line-by-line diff using similar crate
    fn compute_diff(&self, old_text: &str, new_text: &str) -> Vec<DiffLine> {
        let diff = TextDiff::from_lines(old_text, new_text);
        let mut result = Vec::new();
        let mut old_line_num = 1;
        let mut new_line_num = 1;

        for change in diff.iter_all_changes() {
            // 移除行尾换行符
            let line = change.value().trim_end_matches('\n').to_string();

            match change.tag() {
                ChangeTag::Equal => {
                    result.push(DiffLine::Context {
                        line,
                        old_num: old_line_num,
                        new_num: new_line_num,
                    });
                    old_line_num += 1;
                    new_line_num += 1;
                }
                ChangeTag::Delete => {
                    result.push(DiffLine::Delete {
                        line,
                        old_num: old_line_num,
                    });
                    old_line_num += 1;
                }
                ChangeTag::Insert => {
                    result.push(DiffLine::Insert {
                        line,
                        new_num: new_line_num,
                    });
                    new_line_num += 1;
                }
            }
        }

        result
    }

    /// Apply context collapsing to diff lines
    /// Only show changed lines with N lines of context before/after
    fn apply_context_collapsing(&self, diff_lines: Vec<DiffLine>) -> Vec<DiffDisplayItem> {
        const CONTEXT_LINES: usize = 5; // Show 5 lines before and after changes
        const MIN_COLLAPSE_SIZE: usize = CONTEXT_LINES * 2 + 1; // Minimum lines to collapse

        let mut display_items: Vec<DiffDisplayItem> = Vec::new();
        let mut context_buffer: Vec<DiffLine> = Vec::new();
        let mut last_change_index: Option<usize> = None;

        for (i, line) in diff_lines.iter().enumerate() {
            match line {
                DiffLine::Context { .. } => {
                    // Accumulate context lines
                    context_buffer.push(line.clone());
                }
                DiffLine::Insert { .. } | DiffLine::Delete { .. } => {
                    // Found a change - process buffered context
                    if !context_buffer.is_empty() {
                        if let Some(last_idx) = last_change_index {
                            // There was a previous change
                            let distance = i - last_idx - 1;

                            if distance >= MIN_COLLAPSE_SIZE {
                                // Show CONTEXT_LINES after previous change
                                for ctx in context_buffer.iter().take(CONTEXT_LINES) {
                                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                                }

                                // Collapse the middle
                                let collapsed_count = distance - CONTEXT_LINES * 2;
                                if collapsed_count > 0 {
                                    if let DiffLine::Context {
                                        old_num, new_num, ..
                                    } = &context_buffer[CONTEXT_LINES]
                                    {
                                        display_items.push(DiffDisplayItem::Collapsed {
                                            start_old: *old_num,
                                            start_new: *new_num,
                                            count: collapsed_count,
                                        });
                                    }
                                }

                                // Show CONTEXT_LINES before current change
                                let start = context_buffer.len().saturating_sub(CONTEXT_LINES);
                                for ctx in context_buffer.iter().skip(start) {
                                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                                }
                            } else {
                                // Distance is small, show all context
                                for ctx in &context_buffer {
                                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                                }
                            }
                        } else {
                            // This is the first change
                            if context_buffer.len() > CONTEXT_LINES {
                                // Collapse leading context, only show last CONTEXT_LINES
                                let collapsed_count = context_buffer.len() - CONTEXT_LINES;
                                if let DiffLine::Context {
                                    old_num, new_num, ..
                                } = &context_buffer[0]
                                {
                                    display_items.push(DiffDisplayItem::Collapsed {
                                        start_old: *old_num,
                                        start_new: *new_num,
                                        count: collapsed_count,
                                    });
                                }

                                let start = context_buffer.len() - CONTEXT_LINES;
                                for ctx in context_buffer.iter().skip(start) {
                                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                                }
                            } else {
                                // Show all leading context
                                for ctx in &context_buffer {
                                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                                }
                            }
                        }

                        context_buffer.clear();
                    }

                    // Add the change line
                    display_items.push(DiffDisplayItem::Line(line.clone()));
                    last_change_index = Some(i);
                }
            }
        }

        // Handle trailing context
        if !context_buffer.is_empty() {
            if context_buffer.len() > CONTEXT_LINES {
                // Show first CONTEXT_LINES, collapse the rest
                for ctx in context_buffer.iter().take(CONTEXT_LINES) {
                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                }

                let collapsed_count = context_buffer.len() - CONTEXT_LINES;
                if let DiffLine::Context {
                    old_num, new_num, ..
                } = &context_buffer[CONTEXT_LINES]
                {
                    display_items.push(DiffDisplayItem::Collapsed {
                        start_old: *old_num,
                        start_new: *new_num,
                        count: collapsed_count,
                    });
                }
            } else {
                // Show all trailing context
                for ctx in &context_buffer {
                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                }
            }
        }

        display_items
    }

    /// Render a single diff line (Phase 1: plain text)
    fn render_diff_line(&self, diff_line: &DiffLine, cx: &Context<Self>) -> impl IntoElement {
        match diff_line {
            DiffLine::Context {
                line,
                old_num,
                new_num,
            } => {
                h_flex()
                    .w_full()
                    .font_family("Monaco, 'Courier New', monospace")
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .child(
                        // 行号列
                        div()
                            .min_w(px(70.))
                            .px_2()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{:>4} {:>4}  ", old_num, new_num)),
                    )
                    .child(
                        // 代码内容
                        div()
                            .flex_1()
                            .px_2()
                            .text_color(cx.theme().foreground)
                            .child(line.clone()),
                    )
            }
            DiffLine::Insert { line, new_num } => h_flex()
                .w_full()
                .bg(cx.theme().green.opacity(0.1))
                .border_l_2()
                .border_color(cx.theme().green)
                .font_family("Monaco, 'Courier New', monospace")
                .text_size(px(12.))
                .line_height(px(18.))
                .child(
                    div()
                        .min_w(px(70.))
                        .px_2()
                        .text_color(cx.theme().green)
                        .child(format!("     {:>4} +", new_num)),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .text_color(cx.theme().green)
                        .child(line.clone()),
                ),
            DiffLine::Delete { line, old_num } => h_flex()
                .w_full()
                .bg(cx.theme().red.opacity(0.1))
                .border_l_2()
                .border_color(cx.theme().red)
                .font_family("Monaco, 'Courier New', monospace")
                .text_size(px(12.))
                .line_height(px(18.))
                .child(
                    div()
                        .min_w(px(70.))
                        .px_2()
                        .text_color(cx.theme().red)
                        .child(format!("{:>4}      -", old_num)),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .text_color(cx.theme().red)
                        .child(line.clone()),
                ),
        }
    }

    /// Render a collapsed section placeholder
    fn render_collapsed_section(
        &self,
        start_old: usize,
        start_new: usize,
        count: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_center()
            .py_2()
            .bg(cx.theme().muted.opacity(0.3))
            .border_y_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "⋯ {} unchanged lines hidden ({}..{}, {}..{}) ⋯",
                        count,
                        start_old,
                        start_old + count - 1,
                        start_new,
                        start_new + count - 1
                    )),
            )
    }

    /// Render a diff display item (either a line or a collapsed section)
    fn render_diff_display_item(
        &self,
        item: &DiffDisplayItem,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        match item {
            DiffDisplayItem::Line(line) => self.render_diff_line(line, cx).into_any_element(),
            DiffDisplayItem::Collapsed {
                start_old,
                start_new,
                count,
            } => self
                .render_collapsed_section(*start_old, *start_new, *count, cx)
                .into_any_element(),
        }
    }

    /// Render complete diff view with file header
    fn render_diff_view(
        &self,
        diff: &agent_client_protocol::Diff,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // 检测语言 (Phase 2 将用于语法高亮)
        let _language = diff
            .path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| Language::from_str(ext))
            .unwrap_or(Language::Json); // 使用 Json 作为默认值(Plain 仅在 feature 启用时存在)

        // 计算 diff
        let diff_lines = match &diff.old_text {
            Some(old_text) => {
                if old_text == &diff.new_text {
                    Vec::new() // 无变化
                } else {
                    self.compute_diff(old_text, &diff.new_text)
                }
            }
            None => {
                // 新文件 - 所有行都是新增
                diff.new_text
                    .lines()
                    .enumerate()
                    .map(|(i, line)| DiffLine::Insert {
                        line: line.to_string(),
                        new_num: i + 1,
                    })
                    .collect()
            }
        };

        // Apply context collapsing to show only changed parts + context
        let display_items = self.apply_context_collapsing(diff_lines);

        const MAX_DIFF_LINES: usize = 5000;
        let total_lines = display_items.len();
        let truncated = total_lines > MAX_DIFF_LINES;

        v_flex()
            .w_full()
            .gap_2()
            // 文件头部
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
                    .border_1()
                    .border_color(cx.theme().border)
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
                    )
                    .when(diff.old_text.is_none(), |this| {
                        this.child(
                            div()
                                .px_2()
                                .py(px(2.))
                                .rounded(px(4.))
                                .bg(cx.theme().green.opacity(0.2))
                                .text_size(px(11.))
                                .text_color(cx.theme().green)
                                .child("NEW FILE"),
                        )
                    }),
            )
            // 大文件警告
            .when(truncated, |this| {
                this.child(
                    div()
                        .p_2()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().yellow.opacity(0.1))
                        .border_1()
                        .border_color(cx.theme().yellow)
                        .text_size(px(12.))
                        .text_color(cx.theme().yellow)
                        .child(format!(
                            "⚠️ Diff too large ({} lines). Showing first {}.",
                            total_lines, MAX_DIFF_LINES
                        )),
                )
            })
            // Diff 内容
            .child(
                div()
                    .w_full()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .child(
                        v_flex()
                            .w_full()
                            .when(display_items.is_empty(), |this| {
                                this.child(
                                    div()
                                        .p_4()
                                        .flex()
                                        .justify_center()
                                        .text_color(cx.theme().muted_foreground)
                                        .text_size(px(12.))
                                        .child("No changes"),
                                )
                            })
                            .children(
                                display_items
                                    .iter()
                                    .take(MAX_DIFF_LINES)
                                    .map(|item| self.render_diff_display_item(item, cx)),
                            ),
                    ),
            )
            .into_any_element()
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
    ) -> AnyElement {
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
                                    TextView::markdown(markdown_id, text.text.clone())
                                        // .text_size(px(14.))
                                        .text_color(cx.theme().foreground)
                                        // .line_height(px(22.))
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
            ToolCallContent::Diff(diff) => self.render_diff_view(diff, window, cx),
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
