use gpui::*;
use gpui_component::{Theme, h_flex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

impl StatusTone {
    fn color(self, theme: &Theme) -> Hsla {
        match self {
            StatusTone::Neutral => theme.muted_foreground,
            StatusTone::Info => theme.primary,
            StatusTone::Success => theme.success,
            StatusTone::Warning => theme.warning,
            StatusTone::Danger => theme.danger,
        }
    }
}

pub fn status_dot(tone: StatusTone, theme: &Theme) -> AnyElement {
    div()
        .size(px(7.))
        .rounded_full()
        .bg(tone.color(theme))
        .into_any_element()
}

pub fn status_badge(label: impl Into<SharedString>, tone: StatusTone, theme: &Theme) -> AnyElement {
    let color = tone.color(theme);
    h_flex()
        .gap_1()
        .items_center()
        .px_1p5()
        .py_0p5()
        .rounded(px(999.))
        .bg(color.opacity(0.12))
        .child(status_dot(tone, theme))
        .child(div().text_xs().text_color(color).child(label.into()))
        .into_any_element()
}
