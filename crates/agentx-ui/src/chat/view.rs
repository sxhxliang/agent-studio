//! `ChatView` rendering — the pure-view half of the chat panel.
//!
//! Every method here only reads state and builds elements; the click handlers
//! call back into the intents defined on [`ChatView`] in the parent module.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Root, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex,
    skeleton::Skeleton,
    spinner::Spinner,
    v_flex,
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

    fn render_loading_skeleton(&self, cx: &mut Context<Self>) -> AnyElement {
        if !matches!(
            self.session_status,
            SessionStatus::Running | SessionStatus::Pending
        ) {
            return v_flex().into_any_element();
        }

        let current_todo = self.message_stream.read(cx).current_todo_in_progress();
        let (status_icon, status_color) = match self.session_status {
            SessionStatus::Running => (IconName::Loader, cx.theme().primary),
            SessionStatus::Pending => (IconName::LoaderCircle, cx.theme().warning),
            _ => return v_flex().into_any_element(),
        };
        let duration = chrono::Utc::now().signed_duration_since(self.last_active);
        let total_seconds = duration.num_seconds().max(0) as u64;
        let elapsed_time = format!(
            "{:02}:{:02}:{:02}",
            total_seconds / 3600,
            (total_seconds % 3600) / 60,
            total_seconds % 60
        );

        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Spinner::new()
                            .icon(status_icon)
                            .with_size(gpui_component::Size::Medium)
                            .color(status_color),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .flex_1()
                            .when_some(current_todo, |this, todo| {
                                this.child(
                                    h_flex()
                                        .items_center()
                                        .gap_1p5()
                                        .px_2()
                                        .py_1()
                                        .rounded(cx.theme().radius)
                                        .bg(cx.theme().muted.opacity(0.5))
                                        .child(
                                            Icon::new(IconName::Check)
                                                .size(px(12.))
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .max_w(px(400.))
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .child(todo),
                                        ),
                                )
                            })
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        Icon::new(IconName::Info)
                                            .size(px(12.))
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(elapsed_time),
                                    ),
                            ),
                    ),
            )
            .child(
                h_flex().gap_3().child(div().w(px(24.))).child(
                    v_flex()
                        .flex_1()
                        .gap_2()
                        .child(
                            Skeleton::new()
                                .w_full()
                                .max_w(px(480.))
                                .h(px(16.))
                                .rounded(cx.theme().radius),
                        )
                        .child(
                            Skeleton::new()
                                .w_full()
                                .max_w(px(420.))
                                .h(px(16.))
                                .rounded(cx.theme().radius),
                        )
                        .child(
                            Skeleton::new()
                                .w_full()
                                .max_w(px(360.))
                                .h(px(16.))
                                .rounded(cx.theme().radius),
                        ),
                ),
            )
            .into_any_element()
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = match &self.viewed {
            // Browsing a past session: read-only history + a way back / continue.
            Some(viewed) => {
                let label: String = viewed.id.as_str().chars().take(8).collect();
                v_flex()
                    .size_full()
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_2()
                            .items_center()
                            .px_4()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
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
                            .track_scroll(&self.scroll)
                            .overflow_y_scroll()
                            .child(
                                v_flex()
                                    .p_4()
                                    .gap_3()
                                    .bg(cx.theme().background)
                                    .child(viewed.stream.clone()),
                            ),
                    )
                    .into_any_element()
            }
            // The live session: selectors, streaming timeline, permissions, input.
            None => {
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
                let is_empty = self.message_stream.read(cx).is_empty();
                let message_list = v_flex()
                    .p_4()
                    .gap_3()
                    .bg(cx.theme().background)
                    .child(self.message_stream.clone())
                    .child(self.render_loading_skeleton(cx));
                let composer = ChatInputBox::new("chat-composer", self.input.clone())
                    .pasted_images(self.pasted_images.clone())
                    .code_selections(self.code_selections.clone())
                    .selected_files(self.selected_files.clone())
                    .session_status(Some(self.session_status))
                    .disabled(matches!(
                        self.session_status,
                        SessionStatus::Closed | SessionStatus::Failed
                    ))
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
                    .on_paste({
                        let view = view.clone();
                        move |_window, cx| {
                            view.update(cx, |this, cx| this.handle_paste(cx));
                        }
                    })
                    .on_remove_image({
                        let view = view.clone();
                        move |index, _window, cx| {
                            let index = *index;
                            view.update(cx, |this, cx| this.remove_image(index, cx));
                        }
                    })
                    .on_remove_code_selection({
                        let view = view.clone();
                        move |index, _window, cx| {
                            let index = *index;
                            view.update(cx, |this, cx| this.remove_code_selection(index, cx));
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
                    .id("messages")
                    .size_full()
                    .child(
                        div()
                            .id("conversation-scroll-container")
                            .flex_1()
                            .w_full()
                            .track_scroll(&self.scroll)
                            .overflow_y_scroll()
                            .size_full()
                            .when(is_empty, |this| {
                                this.child(
                                    div()
                                        .size_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div()
                                                .text_color(cx.theme().muted_foreground)
                                                .text_sm()
                                                .child("No messages yet"),
                                        ),
                                )
                            })
                            .when(!is_empty, |this| this.pb_3().child(message_list)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w_full()
                            .bg(cx.theme().background)
                            .p_1()
                            .child(composer),
                    )
                    .into_any_element()
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
        "ConversationPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Conversation"
    }
}
