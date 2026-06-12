//! `SessionManagerPanel` rendering — grouped agent cards with session rows.
//!
//! Reads state and builds elements; click handlers call back into the intents
//! defined on [`SessionManagerPanel`] in the parent module.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};

use agentx_domain::{AgentId, SessionId, SessionStatus};

use crate::components::{StatusTone, session_status_tone};

use super::SessionManagerPanel;

impl Render for SessionManagerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let background = theme.background;
        let secondary = theme.secondary;
        let border = theme.border;
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let primary = theme.primary;
        let success = theme.success;
        let warning = theme.warning;
        let danger = theme.danger;

        // Tone → color, using owned locals so the theme borrow ends before the
        // click handlers below capture `cx`.
        let dot = move |status: SessionStatus| {
            let color = match session_status_tone(status) {
                StatusTone::Neutral => muted,
                StatusTone::Info => primary,
                StatusTone::Success => success,
                StatusTone::Warning => warning,
                StatusTone::Danger => danger,
            };
            div().size(px(8.)).rounded_full().bg(color)
        };

        let current = self.chat.read(cx).current_session();
        // Snapshot owned data so the build below borrows neither `self` nor the
        // theme while it wires `cx.listener` handlers.
        let snapshot: Vec<(AgentId, Vec<(SessionId, SessionStatus)>)> = self
            .groups
            .iter()
            .map(|group| {
                (
                    group.agent.clone(),
                    group
                        .sessions
                        .iter()
                        .map(|session| (session.id.clone(), session.status))
                        .collect(),
                )
            })
            .collect();

        let mut group_cards: Vec<AnyElement> = Vec::new();
        for (agent, sessions) in snapshot {
            let count = sessions.len();
            let mut rows: Vec<AnyElement> = Vec::new();
            for (id, status) in sessions {
                let short: String = id.as_str().chars().take(8).collect();
                let selected = id == current;
                let open_id = id.clone();
                let close_id = id.clone();
                let row_bg = if selected { secondary } else { background };
                rows.push(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .p_2()
                        .rounded(px(6.))
                        .bg(row_bg)
                        .border_1()
                        .border_color(border.opacity(0.5))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(dot(status))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(foreground)
                                        .child(format!("Session {short}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("{status:?}")),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new(SharedString::from(format!("open-{id}")))
                                        .ghost()
                                        .small()
                                        .label("Open")
                                        .on_click(cx.listener(move |this, _event, _window, cx| {
                                            this.open(open_id.clone(), cx);
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("close-{id}")))
                                        .ghost()
                                        .small()
                                        .label("Close")
                                        .on_click(cx.listener(move |this, _event, _window, cx| {
                                            this.close(close_id.clone(), cx);
                                        })),
                                ),
                        )
                        .into_any_element(),
                );
            }

            group_cards.push(
                v_flex()
                    .w_full()
                    .gap_2()
                    .p_3()
                    .rounded(px(10.))
                    .bg(secondary)
                    .border_1()
                    .border_color(border)
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(foreground)
                            .child(format!("{agent} ({count})")),
                    )
                    .when(count > 0, |this| {
                        this.child(v_flex().w_full().gap_1().children(rows))
                    })
                    .when(count == 0, |this| {
                        this.child(div().text_xs().text_color(muted).child("No live sessions"))
                    })
                    .into_any_element(),
            );
        }

        v_flex()
            .size_full()
            .gap_2()
            .p_2()
            .bg(background)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_2()
                    .rounded(px(8.))
                    .bg(secondary)
                    .border_1()
                    .border_color(border.opacity(0.6))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child("Sessions"),
                    )
                    .child(
                        Button::new("new-session")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::Plus))
                            .label("New")
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.start_new(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("session-manager-list")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .child(v_flex().w_full().gap_2().children(group_cards)),
            )
    }
}

impl EventEmitter<PanelEvent> for SessionManagerPanel {}

impl Focusable for SessionManagerPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for SessionManagerPanel {
    fn panel_name(&self) -> &'static str {
        "SessionManagerPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Manager"
    }
}
