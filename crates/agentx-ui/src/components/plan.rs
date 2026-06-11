use gpui::*;
use gpui_component::{Icon, IconName, Sizable as _, Theme, h_flex, v_flex};

use agentx_domain::{Plan, PlanEntry, PlanEntryStatus};

/// Render an agent plan as a compact status list.
pub fn render_plan(plan: &Plan, theme: &Theme) -> AnyElement {
    let completed = plan
        .entries
        .iter()
        .filter(|entry| entry.status == PlanEntryStatus::Completed)
        .count();
    let total = plan.entries.len();

    v_flex()
        .w_full()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .w_full()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(16.))
                                .text_color(theme.foreground),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child("Tasks"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(14.))
                        .text_color(theme.muted_foreground)
                        .child(format!("{completed}/{total}")),
                ),
        )
        .child(
            v_flex().w_full().gap_2().children(
                plan.entries
                    .iter()
                    .map(|entry| render_plan_entry(entry, theme)),
            ),
        )
        .into_any_element()
}

fn render_plan_entry(entry: &PlanEntry, theme: &Theme) -> AnyElement {
    let text_color = match entry.status {
        PlanEntryStatus::Completed => theme.muted_foreground,
        PlanEntryStatus::InProgress | PlanEntryStatus::Pending => theme.foreground,
    };

    let (icon, icon_color) = match entry.status {
        PlanEntryStatus::Completed => (IconName::CircleCheck, theme.success),
        PlanEntryStatus::InProgress => (IconName::Loader, theme.foreground),
        PlanEntryStatus::Pending => (IconName::Dash, theme.muted_foreground),
    };

    h_flex()
        .items_start()
        .gap_2()
        .child(
            div()
                .mt(px(1.))
                .child(Icon::new(icon).size(px(16.)).text_color(icon_color)),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(14.))
                .text_color(text_color)
                .line_height(px(20.))
                .child(entry.content.clone()),
        )
        .into_any_element()
}
