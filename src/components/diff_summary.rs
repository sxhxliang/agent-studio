use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*,
    px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::PathBuf;

use agent_client_protocol::{Diff, ToolCall, ToolCallContent, ToolCallId, ToolCallStatus};

/// Statistics for a single file's changes
#[derive(Debug, Clone, Default)]
pub struct FileChangeStats {
    pub path: PathBuf,
    pub additions: usize,
    pub deletions: usize,
    pub is_new_file: bool,
}

impl FileChangeStats {
    /// Calculate statistics from old and new text
    pub fn from_diff(path: PathBuf, old_text: Option<&str>, new_text: &str) -> Self {
        let (additions, deletions, is_new_file) = match old_text {
            Some(old) => {
                let diff = TextDiff::from_lines(old, new_text);
                let (mut adds, mut dels) = (0, 0);
                for change in diff.iter_all_changes() {
                    match change.tag() {
                        ChangeTag::Insert => adds += 1,
                        ChangeTag::Delete => dels += 1,
                        ChangeTag::Equal => {}
                    }
                }
                (adds, dels, false)
            }
            None => (new_text.lines().count(), 0, true),
        };

        Self { path, additions, deletions, is_new_file }
    }

    /// Get total number of changed lines
    pub fn total_changes(&self) -> usize {
        self.additions + self.deletions
    }
}

/// Summary of all file changes in a session
#[derive(Debug, Clone, Default)]
pub struct DiffSummaryData {
    /// Map of file path to change statistics
    pub files: HashMap<PathBuf, FileChangeStats>,
    /// Original tool calls (for finding the ToolCall when clicking)
    pub tool_calls: Vec<ToolCall>,
    /// Merged file states: (initial old_text, final new_text) for multi-edit files
    merged_states: HashMap<PathBuf, (Option<String>, String)>,
}

impl DiffSummaryData {
    /// Extract diff statistics from a list of tool calls
    /// Correctly handles multiple edits to the same file by tracking initial and final states
    pub fn from_tool_calls(tool_calls: &[ToolCall]) -> Self {
        // Track initial state (first old_text) and final state (last new_text) for each file
        let mut file_states: HashMap<PathBuf, (Option<String>, String, bool)> = HashMap::new();

        for tool_call in tool_calls {
            for content in &tool_call.content {
                if let ToolCallContent::Diff(diff) = content {
                    file_states
                        .entry(diff.path.clone())
                        .and_modify(|(_first_old, final_new, is_new)| {
                            // Update only the final state, preserve the initial state
                            *final_new = diff.new_text.clone();
                            // If any edit has old_text, it's not a new file
                            if diff.old_text.is_some() {
                                *is_new = false;
                            }
                        })
                        .or_insert((
                            diff.old_text.clone(),
                            diff.new_text.clone(),
                            diff.old_text.is_none(),
                        ));
                }
            }
        }

        // Calculate final statistics from initial to final state
        let mut files = HashMap::new();
        let mut merged_states = HashMap::new();

        for (path, (first_old, final_new, _is_new)) in file_states {
            let stats = FileChangeStats::from_diff(
                path.clone(),
                first_old.as_deref(),
                &final_new,
            );
            files.insert(path.clone(), stats);
            // Store merged state for creating synthetic ToolCall later
            merged_states.insert(path, (first_old, final_new));
        }

        Self {
            files,
            tool_calls: tool_calls.to_vec(),
            merged_states,
        }
    }

    /// Find or create a ToolCall for the given file path
    /// For files edited multiple times, returns a synthetic ToolCall with merged diff (initial → final)
    /// For files edited once, returns the original ToolCall
    pub fn find_tool_call_for_file(&self, path: &PathBuf) -> Option<ToolCall> {
        let edit_count = self.tool_calls.iter()
            .flat_map(|tc| &tc.content)
            .filter(|c| matches!(c, ToolCallContent::Diff(d) if &d.path == path))
            .count();

        match edit_count {
            0 => None,
            1 => self.tool_calls.iter()
                .find(|tc| tc.content.iter().any(|c|
                    matches!(c, ToolCallContent::Diff(d) if &d.path == path)
                ))
                .cloned(),
            _ => self.create_merged_tool_call(path, edit_count),
        }
    }

    /// Create a synthetic ToolCall with merged diff for multiply-edited files
    fn create_merged_tool_call(&self, path: &PathBuf, edit_count: usize) -> Option<ToolCall> {
        let (first_old, final_new) = self.merged_states.get(path)?;
        let filename = path.file_name()?.to_str().unwrap_or("unknown");

        let merged_diff = Diff::new(path.clone(), final_new.clone())
            .old_text(first_old.clone());

        let mut tool_call = ToolCall::new(
            ToolCallId::from(format!("merged-{}", path.display())),
            format!("Edit {} ({} times)", filename, edit_count),
        );
        tool_call.status = ToolCallStatus::Completed;
        tool_call.content = vec![ToolCallContent::Diff(merged_diff)];

        Some(tool_call)
    }

    /// Get total number of files changed
    pub fn total_files(&self) -> usize {
        self.files.len()
    }

    /// Get total additions across all files
    pub fn total_additions(&self) -> usize {
        self.files.values().map(|f| f.additions).sum()
    }

    /// Get total deletions across all files
    pub fn total_deletions(&self) -> usize {
        self.files.values().map(|f| f.deletions).sum()
    }

    /// Get files sorted by total changes (descending)
    pub fn sorted_files(&self) -> Vec<&FileChangeStats> {
        let mut files: Vec<_> = self.files.values().collect();
        files.sort_unstable_by(|a, b| b.total_changes().cmp(&a.total_changes()));
        files
    }

    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.files.is_empty()
    }
}

/// UI component to display diff summary
pub struct DiffSummary {
    data: DiffSummaryData,
    collapsed: bool,
}

impl DiffSummary {
    pub fn new(data: DiffSummaryData) -> Self {
        Self {
            data,
            collapsed: false,
        }
    }

    /// Toggle collapsed state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.collapsed = !self.collapsed;
        cx.notify();
    }

    /// Update the summary data
    pub fn update_data(&mut self, data: DiffSummaryData, cx: &mut Context<Self>) {
        self.data = data;
        cx.notify();
    }

    /// Render change statistics (additions/deletions)
    fn render_stats(&self, additions: usize, deletions: usize, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .when(additions > 0, |this| {
                this.child(
                    div()
                        .text_size(px(12.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().green)
                        .child(format!("+{}", additions))
                )
            })
            .when(deletions > 0, |this| {
                this.child(
                    div()
                        .text_size(px(12.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().red)
                        .child(format!("-{}", deletions))
                )
            })
    }

    /// Render a single file change row
    fn render_file_row(
        &self,
        stats: &FileChangeStats,
        _window: &mut Window,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let filename = stats.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file_path = stats.path.clone();
        let data = self.data.clone();

        div()
            .w_full()
            .on_mouse_down(
                gpui::MouseButton::Left,
                move |_event, window, cx| {
                    if let Some(tool_call) = data.find_tool_call_for_file(&file_path) {
                        use crate::ShowToolCallDetail;
                        window.dispatch_action(
                            Box::new(ShowToolCallDetail {
                                tool_call_id: tool_call.tool_call_id.to_string(),
                                tool_call,
                            }),
                            cx,
                        );
                    }
                },
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .rounded(px(4.))
                    .hover(|this| this.bg(cx.theme().muted.opacity(0.3)))
                    .cursor_pointer()
                    .child(
                        Icon::new(IconName::File)
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(filename),
                    )
                    .when(stats.is_new_file, |this| {
                        this.child(
                            div()
                                .px_2()
                                .py(px(2.))
                                .rounded(px(4.))
                                .bg(cx.theme().green.opacity(0.2))
                                .text_size(px(11.))
                                .text_color(cx.theme().green)
                                .child("NEW"),
                        )
                    })
                    .child(self.render_stats(stats.additions, stats.deletions, cx))
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(px(14.))
                            .text_color(cx.theme().muted_foreground),
                    )
            )
            .into_any_element()
    }
}

impl Render for DiffSummary {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.data.has_changes() {
            return div().into_any_element();
        }

        let total_files = self.data.total_files();
        let total_additions = self.data.total_additions();
        let total_deletions = self.data.total_deletions();
        let is_collapsed = self.collapsed;

        // Pre-render all file rows before entering the builder chain
        let file_rows: Vec<_> = if !is_collapsed {
            self.data
                .sorted_files()
                .into_iter()
                .map(|stats| self.render_file_row(stats, window, cx))
                .collect()
        } else {
            Vec::new()
        };

        v_flex()
            .w_full()
            .gap_2()
            .p_4()
            .rounded(cx.theme().radius)
            // .bg(cx.theme().secondary)
            .border_1()
            .border_color(cx.theme().border)
            // Header
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::Asterisk)
                            .size(px(16.))
                            .text_color(cx.theme().accent),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child(format!(
                                "{} file{} changed",
                                total_files,
                                if total_files == 1 { "" } else { "s" }
                            )),
                    )
                    .child(self.render_stats(total_additions, total_deletions, cx))
                    .child(
                        Button::new("diff-summary-toggle")
                            .icon(if is_collapsed {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronUp
                            })
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.toggle(cx);
                            })),
                    ),
            )
            // File list (only shown when not collapsed)
            .when(!is_collapsed, |this| {
                this.child(v_flex().w_full().gap_1().children(file_rows))
            })
            .into_any_element()
    }
}
