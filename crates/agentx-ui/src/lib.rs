//! # agentx-ui — GPUI driving adapter (inbound)
//!
//! The user-facing adapter: dock framework, panels, view-models, reusable
//! widgets, and window chrome. Each panel follows the same shape — `mod.rs`
//! (wiring) + `model.rs` (state and intents) + `view.rs` (pure render) — so the
//! layout is predictable. A `PanelKind` enum replaces string-based dispatch.
//!
//! ## Dependency rule
//! Depends on `agentx-app` + `agentx-domain` (use-cases and types), `agentx-bus`
//! (to observe events), `agentx-acp-ui` (pure presentational ACP widgets), and
//! `gpui`. It must NOT depend on the driven adapters (`agentx-acp`,
//! `agentx-store`) — it only knows the application's use-cases and the domain.
//! Views hold no business logic and never reach for a global service locator.
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
    ActiveTheme as _, Root,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};

use agentx_app::SessionService;
use agentx_bus::Receiver;
use agentx_domain::{
    AgentId, ContentBlock, DomainEvent, PermissionOutcome, PermissionRequest, SessionEvent,
    SessionId, SessionInit, SessionMode, SessionModel, SessionStatus, SlashCommand,
};

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
    lines: Vec<SharedString>,
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

        Self {
            service,
            agent,
            session: session_id,
            modes,
            current_mode,
            models,
            current_model,
            commands,
            input,
            lines: Vec::new(),
            pending: Vec::new(),
            scroll: ScrollHandle::new(),
            busy: false,
            _subscriptions: subscriptions,
        }
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
                            this.lines.push(format!("error › {error}").into());
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
                            this.lines.push(format!("set mode error › {error}").into());
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
                            this.lines.push(format!("set model error › {error}").into());
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

    fn record(&mut self, event: DomainEvent) {
        match event {
            DomainEvent::SessionStatusChanged { status, .. } => {
                if matches!(
                    status,
                    SessionStatus::Idle | SessionStatus::Completed | SessionStatus::Failed
                ) {
                    self.busy = false;
                }
                self.lines.push(format!("· {status:?}").into());
            }
            DomainEvent::SessionAppended { event, .. } => {
                self.lines.push(render_event(&event).into());
            }
            DomainEvent::AgentStatusChanged { agent, status } => {
                self.lines.push(format!("· agent {agent}: {status:?}").into());
            }
            DomainEvent::PermissionRequested { request } => {
                self.lines
                    .push(format!("⚠ permission requested: {}", request.tool_call.title).into());
                self.pending.push(request);
            }
            DomainEvent::SessionCommandsChanged { commands, .. } => {
                if !commands.is_empty() {
                    let names = commands
                        .iter()
                        .map(|command| format!("/{}", command.name))
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.lines.push(format!("· commands: {names}").into());
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
        self.lines.push(format!("· permission {decision}").into());
        cx.notify();

        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let result = service.resolve_permission(&session, &permission_id, outcome).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        if let Err(error) = result {
                            this.lines.push(format!("permission error › {error}").into());
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
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = format!(
            "{}  ·  {}{}",
            self.agent,
            self.session,
            if self.busy { "  ·  working…" } else { "" }
        );
        let permissions = self.permission_cards(cx);
        let selectors = self.render_selectors(cx);

        v_flex()
            .size_full()
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
                    .child(
                        v_flex()
                            .gap_1()
                            .children(self.lines.iter().map(|line| div().child(line.clone()))),
                    ),
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

fn render_event(event: &SessionEvent) -> String {
    match event {
        SessionEvent::UserMessage { content } => format!("you › {}", plain_text(content)),
        SessionEvent::AgentMessage { content } => format!("agent › {}", plain_text(content)),
        SessionEvent::AgentThought { text } => format!("thinking › {text}"),
        SessionEvent::ToolCall(call) => format!("tool › {} [{:?}]", call.title, call.status),
        SessionEvent::Plan(plan) => format!("plan › {} entries", plan.entries.len()),
        SessionEvent::Stopped { reason } => format!("— end ({reason:?})"),
    }
}

fn plain_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(ContentBlock::as_plain_text)
        .collect::<Vec<_>>()
        .join("")
}
