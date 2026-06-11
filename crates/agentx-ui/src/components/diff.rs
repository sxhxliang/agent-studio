use gpui::*;
use gpui_component::{Theme, v_flex};
use similar::{ChangeTag, TextDiff};

/// Render a compact line diff. Removals are red, additions green, context muted.
pub fn render_diff(path: &str, old: &str, new: &str, theme: &Theme) -> AnyElement {
    let removed: Hsla = rgb(0xC0392B).into();
    let added: Hsla = rgb(0x27AE60).into();
    let mut lines = v_flex().w_full().gap_0p5().child(
        div()
            .text_xs()
            .text_color(theme.foreground)
            .child(path.to_string()),
    );

    for change in TextDiff::from_lines(old, new).iter_all_changes() {
        let (prefix, color) = match change.tag() {
            ChangeTag::Delete => ("-", removed),
            ChangeTag::Insert => ("+", added),
            ChangeTag::Equal => (" ", theme.muted_foreground),
        };
        let text = format!("{prefix}{}", change.value().trim_end_matches('\n'));
        lines = lines.child(div().text_xs().text_color(color).child(text));
    }

    lines.into_any_element()
}
