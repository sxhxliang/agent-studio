//! `ChatView` rendering — the pure-view half of the chat panel.
//!
//! Every method here only reads state and builds elements; the click handlers
//! call back into the intents defined on [`ChatView`] in the parent module.

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Root, Theme,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    text::TextView,
    v_flex,
};
use similar::{ChangeTag, TextDiff};

use agentx_domain::{PermissionOutcome, Plan, SessionEvent, ToolCall, ToolCallContent};

use super::{ChatView, Entry, plain_text};

impl ChatView {
    /// A Mode row, a Model row, and a Commands row of buttons (the current pick
    /// is highlighted). Empty when the agent advertises none.
    fn render_selectors(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if !self.modes.is_empty() {
            let mut buttons = Vec::new();
            for mode in &self.modes {
                let mode_id = mode.id.clone();
                let selected = self.current_mode.as_deref() == Some(mode.id.as_str());
                let button = Button::new(SharedString::from(format!("mode-{}", mode.id)))
                    .label(mode.name.clone())
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.choose_mode(mode_id.clone(), window, cx);
                    }));
                let button = if selected { button.primary() } else { button.ghost() };
                buttons.push(button.into_any_element());
            }
            rows.push(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().child("Mode"))
                    .children(buttons)
                    .into_any_element(),
            );
        }
        if !self.models.is_empty() {
            let mut buttons = Vec::new();
            for model in &self.models {
                let model_id = model.id.clone();
                let selected = self.current_model.as_deref() == Some(model.id.as_str());
                let button = Button::new(SharedString::from(format!("model-{}", model.id)))
                    .label(model.name.clone())
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.choose_model(model_id.clone(), window, cx);
                    }));
                let button = if selected { button.primary() } else { button.ghost() };
                buttons.push(button.into_any_element());
            }
            rows.push(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().child("Model"))
                    .children(buttons)
                    .into_any_element(),
            );
        }
        if !self.commands.is_empty() {
            let mut buttons = Vec::new();
            for command in &self.commands {
                let name = command.name.clone();
                buttons.push(
                    Button::new(SharedString::from(format!("cmd-{}", command.name)))
                        .label(format!("/{}", command.name))
                        .ghost()
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.insert_command(name.clone(), window, cx);
                        }))
                        .into_any_element(),
                );
            }
            rows.push(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().child("Commands"))
                    .children(buttons)
                    .into_any_element(),
            );
        }
        rows
    }

    /// Build an interactive card per pending permission request: the tool's
    /// title, a button for each option the agent offered, and a Deny.
    fn permission_cards(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut cards = Vec::new();
        for request in &self.pending {
            let permission_id = request.id.to_string();
            let mut buttons = Vec::new();
            for option in &request.options {
                let permission_id = permission_id.clone();
                let option_id = option.id.clone();
                buttons.push(
                    Button::new(SharedString::from(format!("perm-{permission_id}-{option_id}")))
                        .label(option.label.clone())
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.resolve(
                                permission_id.clone(),
                                PermissionOutcome::Selected {
                                    option_id: option_id.clone(),
                                },
                                window,
                                cx,
                            );
                        }))
                        .into_any_element(),
                );
            }
            let deny_id = permission_id.clone();
            buttons.push(
                Button::new(SharedString::from(format!("perm-{permission_id}-deny")))
                    .label("Deny")
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.resolve(deny_id.clone(), PermissionOutcome::Cancelled, window, cx);
                    }))
                    .into_any_element(),
            );
            cards.push(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .child(format!("⚠ permission: {}", request.tool_call.title)),
                    )
                    .child(h_flex().gap_2().children(buttons))
                    .into_any_element(),
            );
        }
        cards
    }

    /// Render a list of timeline entries: rich elements for events, muted lines
    /// for notes, red lines for errors. Shared by live + read-only history.
    fn render_entries(&self, entries: &[Entry], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            items.push(match entry {
                Entry::Note(text) => {
                    let muted = cx.theme().muted_foreground;
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(text.clone())
                        .into_any_element()
                }
                Entry::Error(text) => {
                    let red: Hsla = rgb(0xC0392B).into();
                    div()
                        .text_xs()
                        .text_color(red)
                        .child(text.clone())
                        .into_any_element()
                }
                Entry::Event(event) => self.render_event(index, event, cx),
            });
        }
        items
    }

    /// Render one timeline event. Agent/user messages render as Markdown; tool
    /// calls as collapsible cards (with colored diffs); plans as cards; thoughts
    /// and the stop marker as muted text.
    fn render_event(
        &self,
        index: usize,
        event: &SessionEvent,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match event {
            SessionEvent::UserMessage { content } => {
                let theme = cx.theme();
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(div().text_xs().text_color(theme.muted_foreground).child("you"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.foreground)
                            .child(plain_text(content)),
                    )
                    .into_any_element()
            }
            SessionEvent::AgentMessage { content } => {
                let theme = cx.theme();
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(div().text_xs().text_color(theme.muted_foreground).child("agent"))
                    .child(
                        TextView::markdown(
                            SharedString::from(format!("agent-{index}")),
                            plain_text(content),
                        )
                        .text_sm()
                        .text_color(theme.foreground)
                        .selectable(true),
                    )
                    .into_any_element()
            }
            SessionEvent::AgentThought { text } => {
                let muted = cx.theme().muted_foreground;
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("💭 {text}"))
                    .into_any_element()
            }
            SessionEvent::ToolCall(call) => self.render_tool_call(call, cx),
            SessionEvent::Plan(plan) => {
                let theme = cx.theme();
                render_plan(plan, theme)
            }
            SessionEvent::Stopped { reason } => {
                let muted = cx.theme().muted_foreground;
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("— end ({reason:?})"))
                    .into_any_element()
            }
        }
    }

    /// A collapsible tool-call card: a clickable header, plus its content (text
    /// output or a colored diff) when expanded.
    fn render_tool_call(&self, call: &ToolCall, cx: &mut Context<Self>) -> AnyElement {
        let foreground = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let expanded = self.expanded.contains(&call.id);
        let indicator = if expanded { "▾" } else { "▸" };
        let id = call.id.clone();
        let header = Button::new(SharedString::from(format!("tool-{}", call.id)))
            .ghost()
            .label(format!(
                "{indicator} {:?} · {} [{:?}]",
                call.kind, call.title, call.status
            ))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.toggle_tool(id.clone(), cx);
            }));

        let mut card = v_flex()
            .w_full()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(border)
            .child(header);
        if expanded {
            for content in &call.content {
                card = card.child(render_tool_content(content, foreground, muted));
            }
        }
        card.into_any_element()
    }

    /// The sidebar: a "+ new" action, the live session, and each sibling session
    /// (open / delete). Clicking the live one returns to the live view.
    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let viewing_live = self.viewed.is_none();

        let mut items: Vec<AnyElement> = Vec::new();
        let live_button = Button::new("session-live")
            .label("● live")
            .on_click(cx.listener(|this, _event, _window, cx| {
                let id = this.session.clone();
                this.view_session(id, cx);
            }));
        items.push(
            if viewing_live { live_button.primary() } else { live_button.ghost() }.into_any_element(),
        );
        for meta in &self.sessions {
            let select_id = meta.id.clone();
            let delete_id = meta.id.clone();
            let selected = self.viewed.as_ref().is_some_and(|v| v.id == meta.id);
            let open = Button::new(SharedString::from(format!("session-{}", meta.id)))
                .label(meta.label.clone())
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.view_session(select_id.clone(), cx);
                }));
            let open = if selected { open.primary() } else { open.ghost() };
            let delete = Button::new(SharedString::from(format!("delete-{}", meta.id)))
                .ghost()
                .label("✕")
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.remove_session(delete_id.clone(), cx);
                }));
            items.push(
                h_flex()
                    .gap_1()
                    .child(div().flex_1().child(open))
                    .child(delete)
                    .into_any_element(),
            );
        }

        v_flex()
            .w(px(180.0))
            .h_full()
            .p_2()
            .gap_1()
            .border_r_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().text_color(theme.muted_foreground).child("Sessions"))
                    .child(
                        Button::new("new-session")
                            .ghost()
                            .label("+ new")
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.start_new_session(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("session-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(v_flex().gap_1().children(items)),
            )
            .into_any_element()
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.render_sidebar(cx);

        let main = match &self.viewed {
            // Browsing a past session: read-only history + a way back / continue.
            Some(viewed) => {
                let label: String = viewed.id.as_str().chars().take(8).collect();
                let entries = self.render_entries(&viewed.entries, cx);
                v_flex()
                    .flex_1()
                    .h_full()
                    .p_4()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_sm().child(format!("history · {label} (read-only)")))
                            .child(
                                Button::new("back-to-live")
                                    .label("← live")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.viewed = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("resume-session")
                                    .label("continue ⟳")
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        this.resume_viewed(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("history-events")
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()
                            .child(v_flex().gap_3().children(entries)),
                    )
            }
            // The live session: selectors, streaming timeline, permissions, input.
            None => {
                let header = format!(
                    "{}  ·  {}{}",
                    self.agent,
                    self.session,
                    if self.busy { "  ·  working…" } else { "" }
                );
                let permissions = self.permission_cards(cx);
                let selectors = self.render_selectors(cx);
                let timeline = self.render_entries(&self.live, cx);
                v_flex()
                    .flex_1()
                    .h_full()
                    .p_4()
                    .gap_3()
                    .child(div().text_sm().child(header))
                    .child(v_flex().gap_2().children(selectors))
                    .child(
                        div()
                            .id("chat-events")
                            .flex_1()
                            .w_full()
                            .track_scroll(&self.scroll)
                            .overflow_y_scroll()
                            .child(v_flex().gap_3().children(timeline)),
                    )
                    .child(v_flex().gap_2().children(permissions))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.input)))
                            .child(
                                Button::new("send")
                                    .primary()
                                    .label("Send")
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        this.submit(window, cx)
                                    })),
                            ),
                    )
            }
        };

        h_flex()
            .size_full()
            .child(sidebar)
            .child(main)
            .children(Root::render_notification_layer(window, cx))
    }
}

/// One piece of a tool call's content: text output, or a colored line diff.
fn render_tool_content(content: &ToolCallContent, foreground: Hsla, muted: Hsla) -> AnyElement {
    match content {
        ToolCallContent::Text(text) => div()
            .text_xs()
            .text_color(muted)
            .child(text.clone())
            .into_any_element(),
        ToolCallContent::Diff {
            path,
            old_text,
            new_text,
        } => render_diff(path, old_text.as_deref().unwrap_or(""), new_text, foreground, muted),
    }
}

/// A line diff between `old` and `new`: removals red, additions green, context
/// muted.
fn render_diff(path: &str, old: &str, new: &str, foreground: Hsla, muted: Hsla) -> AnyElement {
    let removed: Hsla = rgb(0xC0392B).into();
    let added: Hsla = rgb(0x27AE60).into();
    let mut lines = v_flex()
        .w_full()
        .child(div().text_xs().text_color(foreground).child(path.to_string()));
    for change in TextDiff::from_lines(old, new).iter_all_changes() {
        let (prefix, color) = match change.tag() {
            ChangeTag::Delete => ("-", removed),
            ChangeTag::Insert => ("+", added),
            ChangeTag::Equal => (" ", muted),
        };
        let text = format!("{prefix}{}", change.value().trim_end_matches('\n'));
        lines = lines.child(div().text_xs().text_color(color).child(text));
    }
    lines.into_any_element()
}

fn render_plan(plan: &Plan, theme: &Theme) -> AnyElement {
    let mut list = v_flex()
        .w_full()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(div().text_sm().text_color(theme.foreground).child("Plan"));
    for entry in &plan.entries {
        list = list.child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!("[{:?}] {}", entry.status, entry.content)),
        );
    }
    list.into_any_element()
}
