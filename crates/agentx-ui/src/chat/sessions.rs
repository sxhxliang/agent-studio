//! The session list — a left-dock panel.
//!
//! A thin view/controller over [`ChatView`]: it reads the chat's session state
//! (observing it for changes) and drives the chat's session actions. The chat
//! itself stays the source of truth, so there is no duplicated state to sync.

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};

use agentx_domain::SessionId;

use super::ChatView;

pub struct SessionsPanel {
    chat: Entity<ChatView>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl SessionsPanel {
    pub fn new(chat: Entity<ChatView>, cx: &mut Context<Self>) -> Self {
        // Re-render whenever the chat's session state changes.
        let subscriptions = vec![cx.observe(&chat, |_, _, cx| cx.notify())];
        Self {
            chat,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// A bordered, rounded row: a clickable label that browses the session, plus
    /// an optional delete button. Highlighted when it is the one being shown.
    fn session_row(
        &self,
        id: SessionId,
        label: SharedString,
        selected: bool,
        deletable: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let bg = if selected { theme.secondary } else { theme.background };
        let border = theme.border.opacity(0.5);
        let open_id = id.clone();

        let mut row = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .px_2()
            .py_1()
            .rounded(px(6.))
            .bg(bg)
            .border_1()
            .border_color(border)
            .child(
                Button::new(SharedString::from(format!("open-{id}")))
                    .ghost()
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.chat
                            .update(cx, |chat, cx| chat.view_session(open_id.clone(), cx));
                    })),
            );
        if deletable {
            let delete_id = id.clone();
            row = row.child(
                Button::new(SharedString::from(format!("del-{id}")))
                    .ghost()
                    .small()
                    .label("✕")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.chat
                            .update(cx, |chat, cx| chat.remove_session(delete_id.clone(), cx));
                    })),
            );
        }
        row.into_any_element()
    }
}

impl Render for SessionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().background;
        let secondary = cx.theme().secondary;
        let border = cx.theme().border;
        let foreground = cx.theme().foreground;

        // Snapshot the chat's session state, then drop the borrow before building
        // the listeners (which capture `cx`).
        let (metas, current, viewed_id) = {
            let chat = self.chat.read(cx);
            let metas: Vec<(SessionId, SharedString)> = chat
                .sessions
                .iter()
                .map(|meta| (meta.id.clone(), meta.label.clone()))
                .collect();
            (metas, chat.session.clone(), chat.viewed.as_ref().map(|v| v.id.clone()))
        };

        let mut rows: Vec<AnyElement> = Vec::new();
        rows.push(self.session_row(current, "● live".into(), viewed_id.is_none(), false, cx));
        for (id, label) in metas {
            let selected = viewed_id.as_ref() == Some(&id);
            rows.push(self.session_row(id, label, selected, true, cx));
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
                            .label("+ new")
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.chat
                                    .update(cx, |chat, cx| chat.start_new_session(window, cx));
                            })),
                    ),
            )
            .child(
                div()
                    .id("session-list")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .child(v_flex().w_full().gap_2().children(rows)),
            )
    }
}

impl EventEmitter<PanelEvent> for SessionsPanel {}

impl Focusable for SessionsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for SessionsPanel {
    fn panel_name(&self) -> &'static str {
        "SessionsPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Sessions"
    }
}
