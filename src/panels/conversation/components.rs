/// UI Components for ConversationPanel
use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex, v_flex,
};

use agent_client_protocol::{
    ContentBlock, ToolCall, ToolCallContent, ToolCallId, ToolCallStatus, ToolCallUpdateFields,
    ToolKind,
};
use similar::{ChangeTag, TextDiff};

use super::helpers::extract_xml_content;
use super::types::{ResourceInfo, ToolCallStatusExt, ToolKindExt, get_file_icon};
use crate::{ShowToolCallDetail, UserMessageData};

// ============================================================================
// Diff Statistics
// ============================================================================

/// Diff statistics
#[derive(Debug, Clone, Default)]
struct DiffStats {
    additions: usize,
    deletions: usize,
}

/// Calculate diff statistics from old and new text
fn calculate_diff_stats(old_text: &str, new_text: &str) -> DiffStats {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut additions = 0;
    let mut deletions = 0;

    for change in diff.iter_all_changes() {
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

/// Extract diff statistics from tool call content
fn extract_diff_stats_from_tool_call(tool_call: &ToolCall) -> Option<DiffStats> {
    // Find the first Diff content in the tool call
    for content in &tool_call.content {
        if let ToolCallContent::Diff(diff) = content {
            return Some(match &diff.old_text {
                Some(old_text) => calculate_diff_stats(old_text, &diff.new_text),
                None => {
                    // New file - all lines are additions
                    DiffStats {
                        additions: diff.new_text.lines().count(),
                        deletions: 0,
                    }
                }
            });
        }
    }
    None
}

// ============================================================================
// Stateful Resource Item
// ============================================================================

pub struct ResourceItemState {
    resource: ResourceInfo,
    open: bool,
}

impl ResourceItemState {
    pub fn new(resource: ResourceInfo) -> Self {
        Self {
            resource,
            open: false,
        }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }
}

impl Render for ResourceItemState {
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
        let mime_type = self.resource.mime_type.clone();

        Collapsible::new()
            .open(is_open)
            .w_full()
            .gap_2()
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
                        Icon::new(get_file_icon(&mime_type.map(|s| s.to_string())))
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
                    .when(line_count > 0, |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} lines", line_count)),
                        )
                    })
                    .when(has_content, |this| {
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

// ============================================================================
// Stateful Agent Thought Item
// ============================================================================

pub struct AgentThoughtItemState {
    text: String,
    open: bool,
}

impl AgentThoughtItemState {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            open: false,
        }
    }

    /// Append more text to the thought (for streaming updates)
    pub fn append_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text.push_str(&text.into());
        cx.notify();
    }
}

impl Render for AgentThoughtItemState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_content = !self.text.is_empty();

        div().pl_6().child(
            Collapsible::new()
                .open(self.open)
                .w_full()
                .gap_2()
                .child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(cx.theme().muted.opacity(0.3))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Bot)
                                        .size(px(14.))
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Thinking..."),
                                )
                                .when(has_content, |this| {
                                    this.child(
                                        Button::new("agent-thought-toggle")
                                            .icon(if self.open {
                                                IconName::ChevronUp
                                            } else {
                                                IconName::ChevronDown
                                            })
                                            .ghost()
                                            .xsmall()
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                this.open = !this.open;
                                                cx.notify();
                                            })),
                                    )
                                }),
                        ),
                )
                .when(has_content, |this| {
                    this.content(
                        div()
                            .mt_2()
                            .p_3()
                            .pl_6()
                            .text_sm()
                            .italic()
                            .text_color(cx.theme().foreground.opacity(0.8))
                            .child(self.text.clone()),
                    )
                }),
        )
    }
}

// ============================================================================
// Stateful Tool Call Item
// ============================================================================

pub struct ToolCallItemState {
    pub(super) tool_call: ToolCall,
    pub(super) open: bool,
}

impl ToolCallItemState {
    pub fn new(tool_call: ToolCall, open: bool) -> Self {
        Self { tool_call, open }
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    pub fn has_content(&self) -> bool {
        !self.tool_call.content.is_empty()
    }

    /// Update this tool call with fields from a ToolCallUpdate
    pub fn apply_update(&mut self, update_fields: ToolCallUpdateFields, cx: &mut Context<Self>) {
        log::debug!("Applying update to tool call: {:?}", update_fields);
        // Use the built-in update method from ToolCall
        self.tool_call.update(update_fields);

        // Auto-open when tool call completes or fails (so user can see result)
        match self.tool_call.status {
            ToolCallStatus::Completed | ToolCallStatus::Failed => {
                if self.has_content() {
                    self.open = true;
                }
            }
            _ => {}
        }

        cx.notify();
    }

    /// Get the tool call ID for matching updates
    pub fn tool_call_id(&self) -> &ToolCallId {
        &self.tool_call.tool_call_id
    }

    /// Get formatted display title for the tool call
    /// For Read tools, formats as: filename#L<offset>-<offset+limit>
    /// For other tools, returns the original title
    fn get_display_title(&self) -> String {
        // Only format Read tool calls
        if !matches!(self.tool_call.kind, ToolKind::Read) {
            return self.tool_call.title.clone();
        }

        // Try to extract file information from locations
        if let Some(first_location) = self.tool_call.locations.first() {
            // Extract filename from path
            let filename = first_location
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("file");

            // Try to get line range from raw_input (which contains offset and limit)
            if let Some(raw_input) = self.tool_call.raw_input.as_ref() {
                // raw_input is a serde_json::Value, so we need to parse it as an object
                if let Some(raw_obj) = raw_input.as_object() {
                    let offset = raw_obj.get("offset").and_then(|v| v.as_i64()).unwrap_or(1);
                    let limit = raw_obj.get("limit").and_then(|v| v.as_i64()).unwrap_or(100);

                    let end_line = offset + limit - 1;
                    return format!("Read ({}#L{}-L{})", filename, offset, end_line);
                }
            }

            // If we have location but no line info, just return filename
            return filename.to_string();
        }

        // Fallback to original title
        self.tool_call.title.clone()
    }
}

impl Render for ToolCallItemState {
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
        let title = self.get_display_title(); // Use formatted title
        let kind_icon = self.tool_call.kind.icon();
        let status_icon = self.tool_call.status.icon();

        // Extract diff stats if this is a diff tool call
        let diff_stats = extract_diff_stats_from_tool_call(&self.tool_call);

        Collapsible::new()
            .open(open)
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().secondary)
                    .child(
                        Icon::new(kind_icon)
                            .size(px(16.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(cx.theme().foreground)
                            .child(title),
                    )
                    // Show diff stats if available
                    .when_some(diff_stats, |this, stats| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(
                                    // Additions
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(cx.theme().green)
                                        .child(format!("+{}", stats.additions))
                                )
                                .child(
                                    // Deletions
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(cx.theme().red)
                                        .child(format!("-{}", stats.deletions))
                                )
                        )
                    })
                    .child(
                        Icon::new(status_icon)
                            .size(px(14.))
                            .text_color(status_color),
                    )
                    .when(has_content, |this| {
                        let tool_call_clone_for_detail = self.tool_call.clone();
                        this.child(
                            h_flex()
                                .gap_2()
                                .child(
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
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.toggle(cx);
                                    })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "tool-call-{}-detail",
                                        tool_call_id
                                    )))
                                    .icon(IconName::Info)
                                    .ghost()
                                    .xsmall()
                                    .on_click(cx.listener(move |_, _ev, window, cx| {
                                        // Dispatch ShowToolCallDetail action
                                        let action = ShowToolCallDetail {
                                            tool_call_id: tool_call_id.to_string(),
                                            tool_call: tool_call_clone_for_detail.clone(),
                                        };
                                        log::debug!("Dispatching ShowToolCallDetail action from ConversationPanel");
                                        window.dispatch_action(Box::new(action), cx);
                                    })),
                                ),
                        )
                    }),
            )
            .when(has_content, |this| {
                this.content(
                    v_flex()
                        .gap_1()
                        .p_3()
                        .pl_8()
                        .children(self.tool_call.content.iter().filter_map(|content| {
                            match content {
                                ToolCallContent::Content(c) => match &c.content {
                                    ContentBlock::Text(text) => {
                                        let cleaned_text =
                                            extract_xml_content(&text.text, &self.tool_call.kind);
                                        Some(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(cx.theme().muted_foreground)
                                                .line_height(px(18.))
                                                .child(cleaned_text),
                                        )
                                    }
                                    _ => None,
                                },
                                ToolCallContent::Diff(diff) => Some(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(cx.theme().muted_foreground)
                                        .line_height(px(18.))
                                        .child(format!(
                                            "Modified: {}\n{} -> {}",
                                            diff.path.display(),
                                            diff.old_text.as_deref().unwrap_or("<new file>"),
                                            diff.new_text
                                        )),
                                ),
                                ToolCallContent::Terminal(terminal) => Some(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(cx.theme().muted_foreground)
                                        .line_height(px(18.))
                                        .child(format!("Terminal: {}", terminal.terminal_id)),
                                ),
                                _ => None,
                            }
                        })),
                )
                .max_h(px(180.)) // Max 10 lines (18px * 10)
                .overflow_hidden()
            })
    }
}

// ============================================================================
// User Message View
// ============================================================================

pub struct UserMessageView {
    pub data: Entity<UserMessageData>,
    pub resource_items: Vec<Entity<ResourceItemState>>,
}

impl Render for UserMessageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx).clone();
        let mut resource_index = 0;

        v_flex()
            .gap_3()
            .w_full()
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
            .child(
                v_flex()
                    .gap_3()
                    .pl_6()
                    .w_full()
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
                                if ResourceInfo::from_content_block(&content).is_some() {
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
                            _ => None,
                        }
                    })),
            )
    }
}
