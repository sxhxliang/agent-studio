//! `WelcomeView` rendering — the pure-view half of the launcher.
//!
//! Reads state and builds elements; click handlers call back into the intents
//! defined on [`WelcomeView`] in the parent module.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Root, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    v_flex,
};

use agentx_domain::{AgentId, AgentStatus};

use super::WelcomeView;

impl WelcomeView {
    /// A selectable agent chip: a Bot glyph, the agent name, and a status dot
    /// when it is already running. Filled (primary) when it is the selection.
    fn agent_chip(&self, agent: &AgentId, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let selected = self.selected.as_ref() == Some(agent);
        let status = self
            .registry
            .agents()
            .into_iter()
            .find(|d| &d.id == agent)
            .map(|d| d.status);
        let dot = status.map(|status| match status {
            AgentStatus::Ready => theme.success,
            AgentStatus::Connecting => theme.warning,
            AgentStatus::Unavailable { .. } => theme.danger,
        });

        let fg = if selected { theme.primary_foreground } else { theme.foreground };
        let bg = if selected { theme.primary } else { theme.secondary };
        let border = if selected { theme.primary } else { theme.border };
        let hover_border = theme.primary.opacity(0.6);

        let id = agent.clone();
        h_flex()
            .id(SharedString::from(format!("agent-{agent}")))
            .gap_1p5()
            .items_center()
            .px_2p5()
            .py_1p5()
            .rounded(px(8.))
            .bg(bg)
            .border_1()
            .border_color(border)
            .hover(move |this| this.border_color(hover_border))
            .child(Icon::new(IconName::Bot).xsmall().text_color(fg))
            .child(div().text_sm().text_color(fg).child(agent.to_string()))
            .when_some(dot, |this, color| {
                this.child(div().size(px(7.)).rounded_full().bg(color))
            })
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_agent(id.clone(), cx);
            }))
            .into_any_element()
    }

    /// A recent-session row: a clock glyph and a label that resumes the session
    /// under the currently selected agent.
    fn recent_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let row_bg = theme.secondary.opacity(0.5);
        let row_border = theme.border.opacity(0.6);
        let hover_bg = theme.secondary;
        let icon_color = theme.muted_foreground;
        let label_color = theme.foreground;
        let recent = &self.recent[index];
        let id = recent.id.clone();
        h_flex()
            .id(SharedString::from(format!("recent-{}", recent.id)))
            .w_full()
            .gap_2()
            .items_center()
            .px_2p5()
            .py_1p5()
            .rounded(px(8.))
            .bg(row_bg)
            .border_1()
            .border_color(row_border)
            .hover(move |this| this.bg(hover_bg))
            .child(Icon::new(IconName::Inbox).xsmall().text_color(icon_color))
            .child(div().flex_1().text_sm().text_color(label_color).child(recent.label.clone()))
            .on_click(cx.listener(move |this, _event, window, cx| {
                this.resume_recent(id.clone(), window, cx);
            }))
            .into_any_element()
    }

    /// A small uppercase section label.
    fn section_label(text: &str, color: Hsla) -> AnyElement {
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color)
            .child(text.to_string())
            .into_any_element()
    }
}

impl Render for WelcomeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let border = theme.border;
        let danger = theme.danger;
        let primary = theme.primary;
        let background = theme.background;
        let card_bg = theme.background;

        // Agent chips, or a hint when none are configured.
        let agents_block = if self.agents.is_empty() {
            div()
                .text_sm()
                .text_color(muted)
                .child("No agents configured. Add one to config.json.")
                .into_any_element()
        } else {
            let chips: Vec<AnyElement> = self
                .agents
                .clone()
                .iter()
                .map(|agent| self.agent_chip(agent, cx))
                .collect();
            h_flex().w_full().flex_wrap().gap_2().children(chips).into_any_element()
        };

        // Recent sessions (omitted entirely when there are none).
        let recent_block = if self.recent.is_empty() {
            None
        } else {
            let rows: Vec<AnyElement> = (0..self.recent.len())
                .map(|index| self.recent_row(index, cx))
                .collect();
            Some(
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(Self::section_label("RECENT", muted))
                    .child(v_flex().w_full().gap_1().children(rows)),
            )
        };

        let can_launch = self.selected.is_some() && !self.launching;
        let send = Button::new("start-chat")
            .primary()
            .rounded_full()
            .icon(Icon::new(IconName::ArrowUp))
            .disabled(!can_launch)
            .on_click(cx.listener(|this, _event, window, cx| {
                this.start_chat(window, cx);
            }));

        let selected_hint = match (&self.selected, self.launching) {
            (_, true) => "Starting…".to_string(),
            (Some(agent), false) => format!("Start with {agent}"),
            (None, false) => "Select an agent above".to_string(),
        };

        // The composer card: a borderless input over a footer row.
        let composer = v_flex()
            .w_full()
            .gap_2p5()
            .p_3()
            .rounded(px(12.))
            .border_1()
            .border_color(border)
            .bg(card_bg)
            .shadow_md()
            .child(div().w_full().child(Input::new(&self.input).appearance(false)))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().text_color(muted).child(selected_hint))
                    .child(send),
            );

        let inner = v_flex()
            .w_full()
            .max_w(px(560.))
            .gap_5()
            .child(
                // Header: logo glyph, title, subtitle.
                v_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(IconName::Bot).large().text_color(primary))
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child("AgentX"),
                    )
                    .child(
                        div()
                            .text_base()
                            .text_color(muted)
                            .text_center()
                            .child("Pick an agent and describe your task."),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(Self::section_label("AGENT", muted))
                    .child(agents_block),
            )
            .child(composer)
            .children(recent_block)
            .when_some(self.status.clone(), |this, status| {
                this.child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(Icon::new(IconName::CircleX).xsmall().text_color(danger))
                        .child(div().text_sm().text_color(danger).child(status)),
                )
            });

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(background)
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .child(inner),
            )
            .children(Root::render_notification_layer(window, cx))
    }
}

impl Focusable for WelcomeView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
