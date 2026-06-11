use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{Icon, IconName, Sizable as _, Theme, h_flex, text::TextView, v_flex};

use agentx_domain::{ContentBlock, ResourceContents};

pub fn render_user_message(content: &[ContentBlock], theme: &Theme) -> AnyElement {
    let (code_chips, other_blocks) = split_code_selection_blocks(content);

    v_flex()
        .gap_3()
        .w_full()
        .child(message_header(
            IconName::User,
            "You",
            theme.accent,
            theme.foreground,
        ))
        .child(
            v_flex()
                .gap_3()
                .pl_6()
                .w_full()
                .children(
                    other_blocks
                        .iter()
                        .filter_map(|block| render_user_content_block(block, theme)),
                )
                .when(!code_chips.is_empty(), |this| {
                    this.child(render_code_selection_chips(code_chips, theme))
                }),
        )
        .into_any_element()
}

pub fn render_agent_message(
    id: impl Into<SharedString>,
    agent_name: impl Into<SharedString>,
    content: &[ContentBlock],
    theme: &Theme,
) -> AnyElement {
    let markdown_id = id.into();
    let text = plain_text(content);

    v_flex()
        .gap_3()
        .w_full()
        .child(message_header(
            IconName::Bot,
            agent_name,
            theme.foreground,
            theme.foreground,
        ))
        .child(
            div().w_full().pl_6().pr_3().child(
                TextView::markdown(markdown_id, text)
                    .text_sm()
                    .text_color(theme.foreground)
                    .selectable(true)
                    .pr_3(),
            ),
        )
        .into_any_element()
}

pub fn render_agent_thought(
    text: &str,
    open: bool,
    toggle_button: Option<AnyElement>,
    theme: &Theme,
) -> AnyElement {
    let has_content = !text.is_empty();

    div()
        .pl_6()
        .child(
            v_flex()
                .w_full()
                .gap_2()
                .child(
                    div()
                        .p_3()
                        .rounded(px(8.))
                        .bg(theme.muted.opacity(0.3))
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Icon::new(IconName::Bot)
                                        .size(px(14.))
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Thinking..."),
                                )
                                .children(toggle_button),
                        ),
                )
                .when(open && has_content, |this| {
                    this.child(
                        div()
                            .mt_2()
                            .p_3()
                            .pl_6()
                            .text_sm()
                            .italic()
                            .text_color(theme.foreground.opacity(0.8))
                            .child(text.to_string()),
                    )
                }),
        )
        .into_any_element()
}

pub fn render_session_end(reason: impl Into<String>, theme: &Theme) -> AnyElement {
    h_flex()
        .gap_2()
        .items_center()
        .pl_6()
        .child(
            Icon::new(IconName::CircleCheck)
                .size(px(14.))
                .text_color(theme.muted_foreground),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("End ({})", reason.into())),
        )
        .into_any_element()
}

pub fn render_timeline_note(text: impl Into<SharedString>, theme: &Theme) -> AnyElement {
    div()
        .pl_6()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn render_timeline_error(text: impl Into<SharedString>) -> AnyElement {
    let red: Hsla = rgb(0xC0392B).into();
    div()
        .pl_6()
        .text_xs()
        .text_color(red)
        .child(text.into())
        .into_any_element()
}

fn message_header(
    icon: IconName,
    label: impl Into<SharedString>,
    icon_color: Hsla,
    label_color: Hsla,
) -> AnyElement {
    h_flex()
        .items_center()
        .gap_2()
        .child(Icon::new(icon).size(px(16.)).text_color(icon_color))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(label_color)
                .child(label.into()),
        )
        .into_any_element()
}

fn render_user_content_block(block: &ContentBlock, theme: &Theme) -> Option<AnyElement> {
    match block {
        ContentBlock::Text(text) => Some(
            div()
                .text_size(px(14.))
                .text_color(theme.foreground)
                .line_height(px(22.))
                .child(text.clone())
                .into_any_element(),
        ),
        ContentBlock::ResourceLink {
            name,
            uri,
            mime_type,
        } => Some(render_resource_card(
            name,
            uri,
            mime_type.as_deref(),
            None,
            theme,
        )),
        ContentBlock::Resource(ResourceContents::Text {
            uri,
            text,
            mime_type,
        }) => Some(render_resource_card(
            &resource_name(uri),
            uri,
            mime_type.as_deref(),
            Some(text),
            theme,
        )),
        ContentBlock::Resource(ResourceContents::Blob { uri, mime_type, .. }) => Some(
            render_resource_card(&resource_name(uri), uri, mime_type.as_deref(), None, theme),
        ),
        ContentBlock::Image { mime_type, .. } => Some(
            h_flex()
                .items_center()
                .gap_2()
                .p_2()
                .rounded(px(8.))
                .bg(theme.muted)
                .border_1()
                .border_color(theme.border)
                .child(
                    Icon::new(IconName::File)
                        .size(px(16.))
                        .text_color(theme.accent),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme.foreground)
                        .child(format!("Image ({mime_type})")),
                )
                .into_any_element(),
        ),
    }
}

fn render_resource_card(
    name: &str,
    uri: &str,
    mime_type: Option<&str>,
    text: Option<&String>,
    theme: &Theme,
) -> AnyElement {
    let line_count = text.map(|text| text.lines().count()).unwrap_or(0);
    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .p_2()
                .rounded(px(8.))
                .bg(theme.muted)
                .border_1()
                .border_color(theme.border)
                .child(
                    Icon::new(IconName::File)
                        .size(px(16.))
                        .text_color(theme.accent),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(name.to_string()),
                )
                .when(line_count > 0, |this| {
                    this.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(format!("{line_count} lines")),
                    )
                })
                .when_some(mime_type, |this, mime_type| {
                    this.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(mime_type.to_string()),
                    )
                }),
        )
        .when_some(text, |this, text| {
            this.child(
                div()
                    .w_full()
                    .p_3()
                    .rounded(px(8.))
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_family("Monaco, 'Courier New', monospace")
                            .text_color(theme.foreground)
                            .line_height(px(18.))
                            .child(text.clone()),
                    ),
            )
        })
        .when(text.is_none(), |this| {
            this.child(
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(uri.to_string()),
            )
        })
        .into_any_element()
}

struct CodeSelectionChip {
    file_path: String,
    line_range: String,
}

fn split_code_selection_blocks(
    content: &[ContentBlock],
) -> (Vec<CodeSelectionChip>, Vec<ContentBlock>) {
    let mut code_chips = Vec::new();
    let mut other_blocks = Vec::new();

    for block in content {
        if let ContentBlock::Text(text) = block {
            if let Some(chip) = parse_code_selection_text(text) {
                code_chips.push(chip);
                continue;
            }
        }
        other_blocks.push(block.clone());
    }

    (code_chips, other_blocks)
}

fn parse_code_selection_text(text: &str) -> Option<CodeSelectionChip> {
    let trimmed = text.trim();
    if !trimmed.starts_with("```\n// File: ") || !trimmed.ends_with("\n```") {
        return None;
    }

    let first_line = trimmed.strip_prefix("```\n")?.lines().next()?;
    let after_prefix = first_line.strip_prefix("// File: ")?;
    let paren_pos = after_prefix.rfind('(')?;
    let file_path = after_prefix[..paren_pos].trim().to_string();
    let line_range = after_prefix[paren_pos + 1..]
        .trim_end_matches(')')
        .trim()
        .to_string();

    let filename = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&file_path)
        .to_string();

    let display_range = if line_range.starts_with("Line ") {
        line_range
            .strip_prefix("Line ")
            .unwrap_or(&line_range)
            .to_string()
    } else if line_range.starts_with("Lines ") {
        line_range
            .strip_prefix("Lines ")
            .unwrap_or(&line_range)
            .replace('-', "~")
    } else {
        line_range
    };

    Some(CodeSelectionChip {
        file_path: filename,
        line_range: display_range,
    })
}

fn render_code_selection_chips(chips: Vec<CodeSelectionChip>, theme: &Theme) -> AnyElement {
    h_flex()
        .gap_1p5()
        .items_center()
        .flex_wrap()
        .children(chips.into_iter().map(|chip| {
            h_flex()
                .gap_1()
                .items_center()
                .py_0p5()
                .px_1p5()
                .rounded(px(6.))
                .bg(theme.primary.opacity(0.1))
                .border_1()
                .border_color(theme.primary.opacity(0.3))
                .child(
                    Icon::new(IconName::Frame)
                        .size(px(13.))
                        .text_color(theme.primary),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.foreground.opacity(0.85))
                        .child(format!("{}:{}", chip.file_path, chip.line_range)),
                )
        }))
        .into_any_element()
}

fn resource_name(uri: &str) -> String {
    uri.split('/').next_back().unwrap_or("unknown").to_string()
}

fn plain_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(ContentBlock::as_plain_text)
        .collect::<Vec<_>>()
        .join("")
}
