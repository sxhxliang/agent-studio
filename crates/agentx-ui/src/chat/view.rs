//! `ChatView` rendering — the pure-view half of the chat panel.
//!
//! Every method here only reads state and builds elements; the click handlers
//! call back into the intents defined on [`ChatView`] in the parent module.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Root, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};

use agentx_domain::{PermissionOutcome, SessionEvent, SessionStatus, SlashCommand, ToolCall};

use crate::components::{
    ChatInputBox, permission_option_button, render_agent_message, render_agent_thought,
    render_permission_request_card, render_plan, render_session_end, render_timeline_error,
    render_timeline_note, render_tool_call_item, render_user_message,
};

use super::{ChatView, Entry};

impl ChatView {
    /// One row of buttons per advertised selector (config options preferred,
    /// legacy modes as a fallback) plus a Commands row. The current pick in each
    /// row is highlighted. Empty when the agent advertises none.
    fn render_selectors(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if !self.config_options.is_empty() {
            // ACP's unified selectors: one labelled row per option, a button per
            // value. Changing one calls back into `choose_config_option`.
            for option in &self.config_options {
                let mut buttons = Vec::new();
                for value in &option.values {
                    let config_id = option.id.clone();
                    let value_id = value.value.clone();
                    let selected = option.current_value == value.value;
                    let button = Button::new(SharedString::from(format!(
                        "cfg-{}-{}",
                        option.id, value.value
                    )))
                    .label(value.name.clone())
                    .on_click(cx.listener(
                        move |this, _event, window, cx| {
                            this.choose_config_option(
                                config_id.clone(),
                                value_id.clone(),
                                window,
                                cx,
                            );
                        },
                    ));
                    let button = if selected {
                        button.primary()
                    } else {
                        button.ghost()
                    };
                    buttons.push(button.into_any_element());
                }
                rows.push(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().text_sm().child(option.name.clone()))
                        .children(buttons)
                        .into_any_element(),
                );
            }
        } else if !self.modes.is_empty() {
            let mut buttons = Vec::new();
            for mode in &self.modes {
                let mode_id = mode.id.clone();
                let selected = self.current_mode.as_deref() == Some(mode.id.as_str());
                let button = Button::new(SharedString::from(format!("mode-{}", mode.id)))
                    .label(mode.name.clone())
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.choose_mode(mode_id.clone(), window, cx);
                    }));
                let button = if selected {
                    button.primary()
                } else {
                    button.ghost()
                };
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
        let theme = cx.theme();
        let mut cards = Vec::new();
        for request in &self.pending {
            let permission_id = request.id.to_string();
            let mut buttons = Vec::new();
            for option in &request.options {
                let permission_id = permission_id.clone();
                let option_id = option.id.clone();
                buttons.push(
                    permission_option_button(
                        SharedString::from(format!("perm-{permission_id}-{option_id}")),
                        option.label.clone(),
                        option.kind,
                    )
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
            let deny_button = Button::new(SharedString::from(format!("perm-{permission_id}-deny")))
                .label("Deny")
                .icon(Icon::new(IconName::CircleX))
                .ghost()
                .small()
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.resolve(deny_id.clone(), PermissionOutcome::Cancelled, window, cx);
                }))
                .into_any_element();
            cards.push(render_permission_request_card(
                request,
                buttons,
                deny_button,
                theme,
            ));
        }
        cards
    }

    /// Render a list of timeline entries: rich elements for events, muted lines
    /// for notes, red lines for errors. Shared by live + read-only history.
    fn render_entries(&self, entries: &[Entry], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut items = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            items.push(match entry {
                Entry::Note(text) => render_timeline_note(text.clone(), cx.theme()),
                Entry::Error(text) => render_timeline_error(text.clone()),
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
            SessionEvent::UserMessage { content } => render_user_message(content, cx.theme()),
            SessionEvent::AgentMessage { content } => render_agent_message(
                SharedString::from(format!("agent-{index}")),
                "Agent",
                content,
                cx.theme(),
            ),
            SessionEvent::AgentThought { text } => {
                let open = self.expanded_thoughts.contains(&index);
                let toggle = (!text.is_empty()).then(|| {
                    Button::new(SharedString::from(format!("thought-toggle-{index}")))
                        .icon(if open {
                            Icon::new(IconName::ChevronUp)
                        } else {
                            Icon::new(IconName::ChevronDown)
                        })
                        .ghost()
                        .xsmall()
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.toggle_thought(index, cx);
                        }))
                        .into_any_element()
                });
                render_agent_thought(text, open, toggle, cx.theme())
            }
            SessionEvent::ToolCall(call) => self.render_tool_call(call, cx),
            SessionEvent::Plan(plan) => {
                let theme = cx.theme();
                render_plan(plan, theme)
            }
            SessionEvent::Stopped { reason } => {
                render_session_end(format!("{reason:?}"), cx.theme())
            }
        }
    }

    /// A collapsible tool-call card: a clickable header, plus its content (text
    /// output or a colored diff) when expanded.
    fn render_tool_call(&self, call: &ToolCall, cx: &mut Context<Self>) -> AnyElement {
        let expanded = self.expanded.contains(&call.id);
        let id = call.id.clone();
        let detail_call = call.clone();
        let toggle = (!call.content.is_empty()).then(|| {
            Button::new(SharedString::from(format!("tool-call-{}-toggle", call.id)))
                .icon(if expanded {
                    Icon::new(IconName::ChevronUp)
                } else {
                    Icon::new(IconName::ChevronDown)
                })
                .ghost()
                .xsmall()
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.toggle_tool(id.clone(), cx);
                }))
                .into_any_element()
        });
        let detail = Button::new(SharedString::from(format!("tool-call-{}-detail", call.id)))
            .ghost()
            .xsmall()
            .icon(Icon::new(IconName::Info))
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                crate::open_tool_call_detail_window(detail_call.clone(), cx);
            }))
            .into_any_element();

        render_tool_call_item(call, expanded, toggle, Some(detail), cx.theme())
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = match &self.viewed {
            // Browsing a past session: read-only history + a way back / continue.
            Some(viewed) => {
                let label: String = viewed.id.as_str().chars().take(8).collect();
                let entries = self.render_entries(&viewed.entries, cx);
                v_flex()
                    .size_full()
                    .p_4()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("history · {label} (read-only)")),
                            )
                            .child(Button::new("back-to-live").label("← live").on_click(
                                cx.listener(|this, _event, _window, cx| {
                                    this.viewed = None;
                                    cx.notify();
                                }),
                            ))
                            .child(Button::new("resume-session").label("continue ⟳").on_click(
                                cx.listener(|this, _event, window, cx| {
                                    this.resume_viewed(window, cx);
                                }),
                            )),
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
                let theme = cx.theme();
                let foreground = theme.foreground;
                let muted = theme.muted_foreground;
                let border = theme.border;
                let primary = theme.primary;
                let card_bg = theme.background;
                let session_label: String = self.session.as_str().chars().take(8).collect();
                let agent_name = self.agent.to_string();
                let busy = self.busy;
                let permissions = self.permission_cards(cx);
                let selectors = self.render_selectors(cx);
                let timeline = self.render_entries(&self.live, cx);

                let header = h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(IconName::Bot).small().text_color(primary))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child(agent_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("· {session_label}")),
                    )
                    .when(busy, |this| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(
                                    Icon::new(IconName::LoaderCircle)
                                        .xsmall()
                                        .text_color(primary),
                                )
                                .child(div().text_xs().text_color(muted).child("working…")),
                        )
                    });

                let view = cx.entity();
                let value = self.input.read(cx).value();
                let typing_command = value.starts_with('/');
                let command_query = value.trim_start_matches('/').to_lowercase();
                let command_suggestions: Vec<SlashCommand> = if typing_command {
                    self.commands
                        .iter()
                        .filter(|command| command.name.to_lowercase().starts_with(&command_query))
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };
                let composer = ChatInputBox::new("chat-composer", self.input.clone())
                    .session_status(Some(if busy {
                        SessionStatus::Running
                    } else {
                        SessionStatus::Idle
                    }))
                    .agent_status_text(if busy { "working…" } else { "ready" })
                    .show_command_suggestions(typing_command && !command_suggestions.is_empty())
                    .command_suggestions(command_suggestions)
                    .file_suggestions(self.file_suggestions.clone())
                    .selected_files(self.selected_files.clone())
                    .on_file_select({
                        let view = view.clone();
                        move |file, window, cx| {
                            let file = file.clone();
                            view.update(cx, |this, cx| this.apply_file_mention(file, window, cx));
                        }
                    })
                    .on_remove_file({
                        let view = view.clone();
                        move |index, _window, cx| {
                            let index = *index;
                            view.update(cx, |this, cx| this.remove_file(index, cx));
                        }
                    })
                    .on_send({
                        let view = view.clone();
                        move |_event, window, cx| {
                            view.update(cx, |this, cx| this.submit(window, cx));
                        }
                    })
                    .on_cancel(move |_event, window, cx| {
                        view.update(cx, |this, cx| this.cancel(window, cx));
                    });

                v_flex()
                    .size_full()
                    .p_4()
                    .gap_3()
                    .child(header)
                    .when(!selectors.is_empty(), |this| {
                        this.child(v_flex().gap_2().children(selectors))
                    })
                    .child(
                        div()
                            .id("chat-events")
                            .flex_1()
                            .w_full()
                            .track_scroll(&self.scroll)
                            .overflow_y_scroll()
                            .child(v_flex().gap_3().children(timeline)),
                    )
                    .when(!permissions.is_empty(), |this| {
                        this.child(v_flex().gap_2().children(permissions))
                    })
                    .child(composer)
            }
        };

        div()
            .size_full()
            .child(main)
            .children(Root::render_notification_layer(window, cx))
    }
}

impl EventEmitter<PanelEvent> for ChatView {}

impl Focusable for ChatView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ChatView {
    fn panel_name(&self) -> &'static str {
        "ChatView"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.agent.to_string()
    }
}
