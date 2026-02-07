use std::path::PathBuf;

use agent_client_protocol::Diff;
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, App, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, v_flex};
use similar::{ChangeTag, TextDiff};

/// Represents a single line in a diff view
#[derive(Debug, Clone)]
pub enum DiffLine {
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
pub enum DiffDisplayItem {
    /// A regular diff line
    Line(DiffLine),
    /// A collapsed section of unchanged lines
    Collapsed {
        start_old: usize,
        start_new: usize,
        count: usize,
    },
}

/// Configuration for DiffView rendering
#[derive(Debug, Clone)]
pub struct DiffViewConfig {
    /// Maximum number of lines to display (default: 5000)
    pub max_lines: usize,
    /// Number of context lines to show before/after changes (default: 5)
    pub context_lines: usize,
    /// Whether to show file header (default: true)
    pub show_file_header: bool,
    /// Whether to show truncation warning (default: true)
    pub show_truncation_warning: bool,
    /// Whether to show collapsed placeholders at file edges (default: false)
    pub show_edge_collapsed: bool,
}

impl Default for DiffViewConfig {
    fn default() -> Self {
        Self {
            max_lines: 5000,
            context_lines: 5,
            show_file_header: true,
            show_truncation_warning: false,
            show_edge_collapsed: false,
        }
    }
}

/// A reusable diff view component that displays file diffs with syntax highlighting
pub struct DiffView {
    diff: Diff,
    config: DiffViewConfig,
}

impl DiffView {
    /// Create a new DiffView with default configuration
    pub fn new(diff: Diff) -> Self {
        Self {
            diff,
            config: DiffViewConfig::default(),
        }
    }

    /// Create a new DiffView with custom configuration
    pub fn with_config(diff: Diff, config: DiffViewConfig) -> Self {
        Self { diff, config }
    }

    /// Set maximum number of lines to display
    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.config.max_lines = max_lines;
        self
    }

    /// Set number of context lines to show before/after changes
    pub fn context_lines(mut self, context_lines: usize) -> Self {
        self.config.context_lines = context_lines;
        self
    }

    /// Set whether to show file header
    pub fn show_file_header(mut self, show: bool) -> Self {
        self.config.show_file_header = show;
        self
    }

    /// Set whether to show truncation warning
    pub fn show_truncation_warning(mut self, show: bool) -> Self {
        self.config.show_truncation_warning = show;
        self
    }

    /// Set whether to show collapsed placeholders at file edges
    pub fn show_edge_collapsed(mut self, show: bool) -> Self {
        self.config.show_edge_collapsed = show;
        self
    }

    /// Compute line-by-line diff using similar crate
    fn compute_diff(&self, old_text: &str, new_text: &str) -> Vec<DiffLine> {
        let diff = TextDiff::from_lines(old_text, new_text);
        let mut result = Vec::new();
        let mut old_line_num = 1;
        let mut new_line_num = 1;

        for change in diff.iter_all_changes() {
            // Remove trailing newlines
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
        let context_lines = self.config.context_lines;
        let min_collapse_size = context_lines * 2 + 1; // Minimum lines to collapse

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

                            if distance >= min_collapse_size {
                                // Show context_lines after previous change
                                for ctx in context_buffer.iter().take(context_lines) {
                                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                                }

                                // Collapse the middle
                                let collapsed_count = distance - context_lines * 2;
                                if collapsed_count > 0 {
                                    if let DiffLine::Context {
                                        old_num, new_num, ..
                                    } = &context_buffer[context_lines]
                                    {
                                        display_items.push(DiffDisplayItem::Collapsed {
                                            start_old: *old_num,
                                            start_new: *new_num,
                                            count: collapsed_count,
                                        });
                                    }
                                }

                                // Show context_lines before current change
                                let start = context_buffer.len().saturating_sub(context_lines);
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
                            if context_buffer.len() > context_lines {
                                // Collapse leading context, only show last context_lines
                                if self.config.show_edge_collapsed {
                                    let collapsed_count = context_buffer.len() - context_lines;
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
                                }

                                let start = context_buffer.len() - context_lines;
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
            if context_buffer.len() > context_lines {
                // Show first context_lines, collapse the rest
                for ctx in context_buffer.iter().take(context_lines) {
                    display_items.push(DiffDisplayItem::Line(ctx.clone()));
                }
                if self.config.show_edge_collapsed {
                    let collapsed_count = context_buffer.len() - context_lines;
                    if let DiffLine::Context {
                        old_num, new_num, ..
                    } = &context_buffer[context_lines]
                    {
                        display_items.push(DiffDisplayItem::Collapsed {
                            start_old: *old_num,
                            start_new: *new_num,
                            count: collapsed_count,
                        });
                    }
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

    /// Render a single diff line
    fn render_diff_line<'a>(
        &self,
        diff_line: &'a DiffLine,
        _window: &'a mut Window,
        cx: &'a mut App,
    ) -> impl IntoElement + 'a {
        match diff_line {
            DiffLine::Context {
                line,
                old_num,
                new_num,
            } => h_flex()
                .w_full()
                .font_family("Monaco, 'Courier New', monospace")
                .text_size(px(12.))
                .line_height(px(18.))
                .child(
                    // Line number column
                    div()
                        .min_w(px(70.))
                        .px_2()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{:>4} {:>4}  ", old_num, new_num)),
                )
                .child(
                    // Code content
                    div()
                        .flex_1()
                        .px_2()
                        .text_color(cx.theme().foreground)
                        .child(line.clone()),
                ),
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
    fn render_collapsed_section<'a>(
        &self,
        start_old: usize,
        start_new: usize,
        count: usize,
        _window: &'a mut Window,
        cx: &'a mut App,
    ) -> impl IntoElement + 'a {
        h_flex()
            .w_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().muted.opacity(0.3))
            .border_y_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "... {} unchanged lines hidden ({}..{}, {}..{}) ...",
                        count,
                        start_old,
                        start_old + count - 1,
                        start_new,
                        start_new + count - 1
                    )),
            )
    }

    /// Render a diff display item (either a line or a collapsed section)
    fn render_diff_display_item<'a>(
        &self,
        item: &'a DiffDisplayItem,
        window: &'a mut Window,
        cx: &'a mut App,
    ) -> AnyElement {
        match item {
            DiffDisplayItem::Line(line) => {
                self.render_diff_line(line, window, cx).into_any_element()
            }
            DiffDisplayItem::Collapsed {
                start_old,
                start_new,
                count,
            } => self
                .render_collapsed_section(*start_old, *start_new, *count, window, cx)
                .into_any_element(),
        }
    }

    /// Render file header
    fn render_file_header<'a>(
        &self,
        path: &'a PathBuf,
        is_new_file: bool,
        _window: &'a mut Window,
        cx: &'a mut App,
    ) -> impl IntoElement + 'a {
        h_flex()
            .items_center()
            .gap_2()
            .p_2()
            .rounded(cx.theme().radius)
            .bg(cx.theme().secondary)
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
                    .child(path.display().to_string()),
            )
            .when(is_new_file, |this| {
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
            })
    }

    /// Render truncation warning
    fn render_truncation_warning<'a>(
        &self,
        total_lines: usize,
        max_lines: usize,
        _window: &'a mut Window,
        cx: &'a mut App,
    ) -> impl IntoElement + 'a {
        div()
            .p_2()
            .rounded(cx.theme().radius)
            .bg(cx.theme().yellow.opacity(0.1))
            .text_size(px(12.))
            .text_color(cx.theme().yellow)
            .child(format!(
                "Warning: Diff too large ({} lines). Showing first {}.",
                total_lines, max_lines
            ))
    }
}

impl RenderOnce for DiffView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Compute diff
        let diff_lines = match &self.diff.old_text {
            Some(old_text) => {
                if old_text == &self.diff.new_text {
                    Vec::new() // No changes
                } else {
                    self.compute_diff(old_text, &self.diff.new_text)
                }
            }
            None => {
                // New file - all lines are insertions
                self.diff
                    .new_text
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

        let total_lines = display_items.len();
        let truncated = total_lines > self.config.max_lines;
        let is_new_file = self.diff.old_text.is_none();

        v_flex()
            .w_full()
            .gap_2()
            // File header
            .when(self.config.show_file_header, |this| {
                this.child(self.render_file_header(&self.diff.path, is_new_file, window, cx))
            })
            // Large file warning
            .when(truncated && self.config.show_truncation_warning, |this| {
                this.child(self.render_truncation_warning(
                    total_lines,
                    self.config.max_lines,
                    window,
                    cx,
                ))
            })
            // Diff content
            .child(
                div()
                    .w_full()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
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
                                    .take(self.config.max_lines)
                                    .map(|item| self.render_diff_display_item(item, window, cx)),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_diff_detects_insertions() {
        let diff = Diff::new("file.txt", "line1\nline2".to_string()).old_text("line1".to_string());
        let view = DiffView::new(diff);
        let lines = view.compute_diff("line1", "line1\nline2");
        assert!(matches!(lines.last(), Some(DiffLine::Insert { .. })));
    }

    #[test]
    fn apply_context_collapsing_shows_collapsed_items() {
        let diff = Diff::new("file.txt", "a\nb\nc\nd\ne\nf".to_string())
            .old_text("a\nb\nc\nX\ne\nf".to_string());
        let mut view = DiffView::new(diff)
            .context_lines(1)
            .show_edge_collapsed(true);
        let lines = view.compute_diff("a\nb\nc\nX\ne\nf", "a\nb\nc\nd\ne\nf");
        let items = view.apply_context_collapsing(lines);
        assert!(!items.is_empty());
    }
}
