//! # agentx-ui — GPUI driving adapter (inbound)
//!
//! The user-facing adapter: dock framework, panels, view-models, reusable
//! widgets, and window chrome. Each panel follows the same shape — `mod.rs`
//! (wiring) + `model.rs` (state and intents) + `view.rs` (pure render) — so the
//! layout is predictable. A `PanelKind` enum replaces string-based dispatch.
//!
//! ## Dependency rule
//! Depends on `agentx-app` + `agentx-domain` (use-cases and types), `agentx-bus`
//! (to observe events), and `gpui`. It must NOT depend on the driven adapters
//! (`agentx-acp`, `agentx-store`) — it only knows the application's use-cases and
//! the domain. Views hold no business logic and never reach for a global service
//! locator. The composition root (`agentx-shell`) wires the adapters in.
//!
//! ## M4b/M5b — the first window
//! [`ChatView`] is the minimal proof that the GPUI shell can drive the rewritten
//! stack: it sends a prompt through [`SessionService`], renders the
//! [`DomainEvent`]s that stream back over the [`EventBus`], and lets the user
//! allow/deny the permission requests an agent raises mid-turn. It deliberately
//! has no dock and no persistence — those arrive in later milestones. The
//! composition root that injects the adapters lives in `examples/window.rs`, so
//! this library stays free of any adapter dependency.

use std::sync::Arc;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Root, Theme,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    text::TextView,
    v_flex,
};

use agentx_app::SessionService;
use agentx_bus::Receiver;
use agentx_domain::{
    AgentId, ContentBlock, DomainEvent, PermissionOutcome, PermissionRequest, Plan, SessionEvent,
    SessionId, SessionInit, SessionMode, SessionModel, SessionStatus, SlashCommand, ToolCall,
    ToolCallContent,
};

/// One row in the conversation: either a rich timeline event or a terse
/// system note (status changes, command/permission updates, errors).
enum Entry {
    Event(SessionEvent),
    Note(SharedString),
}

/// A past session loaded read-only from disk for browsing in the sidebar.
struct ViewedSession {
    id: SessionId,
    entries: Vec<Entry>,
}

/// A single-session chat window over [`SessionService`].
///
/// It owns only view state plus the use-case handle it drives; it never touches
/// an adapter. Streamed output is observed, not requested: a background task
/// folds every [`DomainEvent`] into a line via [`record`](Self::record).
pub struct ChatView {
    service: Arc<SessionService>,
    agent: AgentId,
    session: SessionId,
    /// Modes/models the agent advertised for this session, with the current pick.
    modes: Vec<SessionMode>,
    current_mode: Option<String>,
    models: Vec<SessionModel>,
    current_model: Option<String>,
    /// Slash commands the agent advertises; updated via a notification.
    commands: Vec<SlashCommand>,
    input: Entity<InputState>,
    /// The live session's timeline, accumulated from the bus.
    live: Vec<Entry>,
    /// Sibling sessions on disk, for the sidebar (excludes the live one).
    sessions: Vec<SessionId>,
    /// A past session being browsed read-only; `None` means the live session.
    viewed: Option<ViewedSession>,
    /// Permission requests awaiting the user's allow/deny decision.
    pending: Vec<PermissionRequest>,
    scroll: ScrollHandle,
    busy: bool,
    _subscriptions: Vec<Subscription>,
}

impl ChatView {
    fn new(
        service: Arc<SessionService>,
        mut events: Receiver<DomainEvent>,
        agent: AgentId,
        init: SessionInit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let SessionInit {
            session_id,
            modes,
            current_mode,
            models,
            current_model,
            commands,
        } = init;

        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Type a message, press Enter to send…")
        });

        let subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |this, _input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.submit(window, cx);
                }
            },
        )];

        // Drain the bus into the view. The receiver is created by the caller
        // *before* the agent starts, so events the agent emits during session
        // setup (slash commands, status) are buffered and replayed here rather
        // than lost — broadcast has no replay for late subscribers.
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let _ = cx.update(|cx| {
                    if let Some(view) = this.upgrade() {
                        view.update(cx, |this, cx| {
                            this.record(event);
                            this.scroll.scroll_to_bottom();
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();

        let view = Self {
            service,
            agent,
            session: session_id,
            modes,
            current_mode,
            models,
            current_model,
            commands,
            input,
            live: Vec::new(),
            sessions: Vec::new(),
            viewed: None,
            pending: Vec::new(),
            scroll: ScrollHandle::new(),
            busy: false,
            _subscriptions: subscriptions,
        };
        view.refresh_sessions(cx);
        view
    }

    /// Reload the sibling session list (everything on disk except the live one).
    fn refresh_sessions(&self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let live = self.session.clone();
        cx.spawn(async move |this, cx| {
            let sessions = service.list_sessions().await.unwrap_or_default();
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.sessions = sessions.into_iter().filter(|id| *id != live).collect();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Browse a session: the live one returns to the live view, any other loads
    /// its persisted history read-only.
    fn view_session(&mut self, id: SessionId, cx: &mut Context<Self>) {
        if id == self.session {
            self.viewed = None;
            cx.notify();
            return;
        }
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let history = service.history(&id).await.unwrap_or_default();
            let entries = history
                .into_iter()
                .map(|event| Entry::Event(event.event))
                .collect();
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.viewed = Some(ViewedSession { id, entries });
                        this.scroll.scroll_to_bottom();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Send the current input as a prompt, then clear it. The reply streams back
    /// over the bus, so this only needs to flip the busy flag and report errors.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.busy = true;
        cx.notify();

        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = service
                .send_message(&session, vec![ContentBlock::text(text)])
                .await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.busy = false;
                        if let Err(error) = result {
                            this.note(format!("error › {error}"));
                            this.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Switch the session's mode, updating the local selection optimistically.
    fn choose_mode(&mut self, mode_id: String, cx: &mut Context<Self>) {
        if self.current_mode.as_deref() == Some(mode_id.as_str()) {
            return;
        }
        self.current_mode = Some(mode_id.clone());
        cx.notify();
        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = service.set_mode(&session, &mode_id).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        if let Err(error) = result {
                            this.note(format!("set mode error › {error}"));
                        }
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Switch the session's model, updating the local selection optimistically.
    fn choose_model(&mut self, model_id: String, cx: &mut Context<Self>) {
        if self.current_model.as_deref() == Some(model_id.as_str()) {
            return;
        }
        self.current_model = Some(model_id.clone());
        cx.notify();
        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = service.set_model(&session, &model_id).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        if let Err(error) = result {
                            this.note(format!("set model error › {error}"));
                        }
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// A Mode row and a Model row of buttons (the current pick is highlighted).
    /// Empty when the agent advertises no modes/models.
    fn render_selectors(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if !self.modes.is_empty() {
            let mut buttons = Vec::new();
            for mode in &self.modes {
                let mode_id = mode.id.clone();
                let selected = self.current_mode.as_deref() == Some(mode.id.as_str());
                let button = Button::new(SharedString::from(format!("mode-{}", mode.id)))
                    .label(mode.name.clone())
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.choose_mode(mode_id.clone(), cx);
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
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.choose_model(model_id.clone(), cx);
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

    /// Insert a slash command into the input for the user to complete and send.
    fn insert_command(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            state.insert(&format!("/{name} "), window, cx);
        });
    }

    /// Append a terse system note to the timeline.
    fn note(&mut self, text: impl Into<SharedString>) {
        self.live.push(Entry::Note(text.into()));
    }

    fn record(&mut self, event: DomainEvent) {
        match event {
            DomainEvent::SessionStatusChanged { status, .. } => {
                if matches!(
                    status,
                    SessionStatus::Idle | SessionStatus::Completed | SessionStatus::Failed
                ) {
                    self.busy = false;
                }
                self.note(format!("· {status:?}"));
            }
            DomainEvent::SessionAppended { event, .. } => {
                self.live.push(Entry::Event(event));
            }
            DomainEvent::AgentStatusChanged { agent, status } => {
                self.note(format!("· agent {agent}: {status:?}"));
            }
            DomainEvent::PermissionRequested { request } => {
                self.note(format!("⚠ permission requested: {}", request.tool_call.title));
                self.pending.push(request);
            }
            DomainEvent::SessionCommandsChanged { commands, .. } => {
                if !commands.is_empty() {
                    let names = commands
                        .iter()
                        .map(|command| format!("/{}", command.name))
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.note(format!("· commands: {names}"));
                }
                self.commands = commands;
            }
            DomainEvent::ConfigChanged => {}
        }
    }

    /// Answer a pending permission request and forward the decision to the agent.
    fn resolve(&mut self, permission_id: String, outcome: PermissionOutcome, cx: &mut Context<Self>) {
        let Some(pos) = self
            .pending
            .iter()
            .position(|request| request.id.as_str() == permission_id.as_str())
        else {
            return;
        };
        let session = self.pending.remove(pos).session;
        let decision = match &outcome {
            PermissionOutcome::Selected { .. } => "allowed",
            PermissionOutcome::Cancelled => "denied",
        };
        self.note(format!("· permission {decision}"));
        cx.notify();

        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.resolve_permission(&session, &permission_id, outcome).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        if let Err(error) = result {
                            this.note(format!("permission error › {error}"));
                        }
                        cx.notify();
                    });
                }
            });
        })
        .detach();
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
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.resolve(
                                permission_id.clone(),
                                PermissionOutcome::Selected {
                                    option_id: option_id.clone(),
                                },
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
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.resolve(deny_id.clone(), PermissionOutcome::Cancelled, cx);
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
    /// for system notes. Shared by the live timeline and the read-only history.
    fn render_entries(&self, entries: &[Entry], cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme();
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::Note(text) => div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(text.clone())
                    .into_any_element(),
                Entry::Event(event) => render_event(index, event, theme),
            })
            .collect()
    }

    /// The sidebar: the live session plus each sibling session on disk. Clicking
    /// one browses it; clicking the live one returns to the live view.
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
        for id in &self.sessions {
            let session = id.clone();
            let selected = self.viewed.as_ref().is_some_and(|v| v.id == *id);
            let label: String = id.as_str().chars().take(8).collect();
            let button = Button::new(SharedString::from(format!("session-{id}")))
                .label(label)
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.view_session(session.clone(), cx);
                }));
            items.push(if selected { button.primary() } else { button.ghost() }.into_any_element());
        }

        v_flex()
            .w(px(180.0))
            .h_full()
            .p_2()
            .gap_1()
            .border_r_1()
            .border_color(theme.border)
            .child(div().text_sm().text_color(theme.muted_foreground).child("Sessions"))
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.render_sidebar(cx);

        let main = match &self.viewed {
            // Browsing a past session: read-only history + a way back.
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

        h_flex().size_full().child(sidebar).child(main)
    }
}

/// Open a window hosting a [`ChatView`] for an already-created session.
///
/// `events` must be a receiver subscribed *before* the agent was started, so the
/// view replays the session-setup events (slash commands, status) instead of
/// missing them. The composition root starts the agent, creates the session,
/// then opens the window with its [`SessionInit`].
pub fn open_chat_window(
    service: Arc<SessionService>,
    events: Receiver<DomainEvent>,
    agent: AgentId,
    init: SessionInit,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| ChatView::new(service, events, agent, init, window, cx));
        // The first level on the window must be a `Root`.
        cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
    });
}

/// Render one timeline event. Agent/user messages render as Markdown; tool
/// calls and plans as bordered cards; thoughts and the stop marker as muted text.
fn render_event(index: usize, event: &SessionEvent, theme: &Theme) -> AnyElement {
    match event {
        SessionEvent::UserMessage { content } => v_flex()
            .w_full()
            .gap_1()
            .child(div().text_xs().text_color(theme.muted_foreground).child("you"))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.foreground)
                    .child(plain_text(content)),
            )
            .into_any_element(),
        SessionEvent::AgentMessage { content } => v_flex()
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
            .into_any_element(),
        SessionEvent::AgentThought { text } => div()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(format!("💭 {text}"))
            .into_any_element(),
        SessionEvent::ToolCall(call) => render_tool_call(call, theme),
        SessionEvent::Plan(plan) => render_plan(plan, theme),
        SessionEvent::Stopped { reason } => div()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(format!("— end ({reason:?})"))
            .into_any_element(),
    }
}

fn render_tool_call(call: &ToolCall, theme: &Theme) -> AnyElement {
    let mut card = v_flex()
        .w_full()
        .gap_1()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(div().text_sm().text_color(theme.foreground).child(format!(
            "{:?} · {} [{:?}]",
            call.kind, call.title, call.status
        )));
    for content in &call.content {
        let text = match content {
            ToolCallContent::Text(text) => text.clone(),
            ToolCallContent::Diff { path, new_text, .. } => format!("{path}\n{new_text}"),
        };
        card = card.child(div().text_xs().text_color(theme.muted_foreground).child(text));
    }
    card.into_any_element()
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

fn plain_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(ContentBlock::as_plain_text)
        .collect::<Vec<_>>()
        .join("")
}
