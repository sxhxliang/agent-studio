use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    Icon, IconName, Sizable as _, Theme, collapsible::Collapsible, h_flex, v_flex,
};
use similar::{ChangeTag, TextDiff};

use agentx_domain::{ToolCall, ToolCallContent, ToolCallStatus, ToolKind};

use super::{StatusTone, render_diff};

pub fn tool_status_tone(status: ToolCallStatus) -> StatusTone {
    match status {
        ToolCallStatus::Pending => StatusTone::Neutral,
        ToolCallStatus::InProgress => StatusTone::Info,
        ToolCallStatus::Completed => StatusTone::Success,
        ToolCallStatus::Failed => StatusTone::Danger,
    }
}

pub fn tool_status_label(status: ToolCallStatus) -> &'static str {
    match status {
        ToolCallStatus::Pending => "Pending",
        ToolCallStatus::InProgress => "Running",
        ToolCallStatus::Completed => "Done",
        ToolCallStatus::Failed => "Failed",
    }
}

pub fn render_tool_call_content(content: &ToolCallContent, theme: &Theme) -> AnyElement {
    match content {
        ToolCallContent::Text(text) => div()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(text.clone())
            .into_any_element(),
        ToolCallContent::Diff {
            path,
            old_text,
            new_text,
        } => render_diff(path, old_text.as_deref().unwrap_or(""), new_text, theme),
    }
}

pub fn render_tool_call_body(contents: &[ToolCallContent], theme: &Theme) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .children(
            contents
                .iter()
                .map(|content| render_tool_call_content(content, theme)),
        )
        .into_any_element()
}

pub fn render_tool_call_item(
    call: &ToolCall,
    open: bool,
    toggle_button: Option<AnyElement>,
    detail_button: Option<AnyElement>,
    theme: &Theme,
) -> AnyElement {
    let has_content = !call.content.is_empty();
    let status_icon = tool_status_icon(call.status);
    let status_color = match call.status {
        ToolCallStatus::Completed => theme.success,
        ToolCallStatus::Failed => theme.danger,
        ToolCallStatus::InProgress => theme.accent,
        ToolCallStatus::Pending => theme.muted_foreground,
    };
    let diff_stats = diff_stats(call);

    Collapsible::new()
        .open(open)
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .p_2()
                .rounded(px(8.))
                .bg(theme.secondary)
                .child(
                    tool_kind_icon(call.kind)
                        .size(px(16.))
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(13.))
                        .text_color(theme.foreground)
                        .line_height(px(18.))
                        .whitespace_normal()
                        .child(call.title.clone()),
                )
                .when_some(diff_stats, |this, stats| {
                    this.child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.success)
                                    .child(format!("+{}", stats.additions)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.danger)
                                    .child(format!("-{}", stats.deletions)),
                            ),
                    )
                })
                .child(status_icon.size(px(14.)).text_color(status_color))
                .when(has_content || detail_button.is_some(), |this| {
                    this.child(
                        h_flex()
                            .gap_2()
                            .children(toggle_button)
                            .children(detail_button),
                    )
                }),
        )
        .when(has_content, |this| {
            this.content(
                v_flex().gap_2().pl_8().children(
                    call.content
                        .iter()
                        .map(|content| render_tool_call_content(content, theme)),
                ),
            )
            .max_h(px(300.))
            .overflow_hidden()
        })
        .into_any_element()
}

fn tool_kind_icon(kind: ToolKind) -> Icon {
    match kind {
        ToolKind::Read => Icon::new(IconName::Eye),
        ToolKind::Edit => Icon::new(IconName::Replace),
        ToolKind::Delete => Icon::new(IconName::Delete),
        ToolKind::Move => Icon::new(IconName::ArrowRight),
        ToolKind::Search => Icon::new(IconName::Search),
        ToolKind::Execute => Icon::new(IconName::SquareTerminal),
        ToolKind::Think => Icon::new(IconName::Bot),
        ToolKind::Fetch => Icon::new(IconName::Globe),
        ToolKind::Other => Icon::new(IconName::Ellipsis),
    }
}

fn tool_status_icon(status: ToolCallStatus) -> Icon {
    match status {
        ToolCallStatus::Completed => Icon::new(IconName::CircleCheck),
        ToolCallStatus::Failed => Icon::new(IconName::CircleX),
        ToolCallStatus::Pending | ToolCallStatus::InProgress => Icon::new(IconName::Dash),
    }
}

#[derive(Clone, Copy)]
struct DiffStats {
    additions: usize,
    deletions: usize,
}

fn diff_stats(call: &ToolCall) -> Option<DiffStats> {
    call.content.iter().find_map(|content| match content {
        ToolCallContent::Diff {
            old_text, new_text, ..
        } => Some(calculate_diff_stats(
            old_text.as_deref().unwrap_or(""),
            new_text,
        )),
        ToolCallContent::Text(_) => None,
    })
}

fn calculate_diff_stats(old_text: &str, new_text: &str) -> DiffStats {
    let mut additions = 0;
    let mut deletions = 0;

    for change in TextDiff::from_lines(old_text, new_text).iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            ChangeTag::Equal => {}
        }
    }

    DiffStats {
        additions,
        deletions,
    }
}
