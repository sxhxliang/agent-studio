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
//! ## M4b — the first window
//! [`ChatView`] is the minimal proof that the GPUI shell can drive the rewritten
//! stack: it sends a prompt through [`SessionService`] and renders the
//! [`DomainEvent`]s that stream back over the [`EventBus`]. It deliberately has
//! no dock, no persistence, and no permission UI — those arrive in later
//! milestones. The composition root that injects the adapters lives in
//! `examples/window.rs`, so this library stays free of any adapter dependency.

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
use agentx_bus::EventBus;
use agentx_domain::{AgentId, ContentBlock, DomainEvent, SessionEvent, SessionId, SessionStatus};

/// A single-session chat window over [`SessionService`].
///
/// It owns only view state plus the use-case handle it drives; it never touches
/// an adapter. Streamed output is observed, not requested: a background task
/// folds every [`DomainEvent`] into a line via [`record`](Self::record).
pub struct ChatView {
    service: Arc<SessionService>,
    agent: AgentId,
    session: SessionId,
    input: Entity<InputState>,
    lines: Vec<SharedString>,
    scroll: ScrollHandle,
    busy: bool,
    _subscriptions: Vec<Subscription>,
}

impl ChatView {
    fn new(
        service: Arc<SessionService>,
        bus: EventBus,
        agent: AgentId,
        session: SessionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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

        // Bridge the bus into the view: each domain event becomes a line. The
        // task ends when the window (and thus the entity) goes away.
        let mut rx = bus.subscribe::<DomainEvent>();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
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
            session,
            input,
            lines: Vec::new(),
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
            DomainEvent::PermissionRequested { permission, .. } => {
                self.lines.push(
                    format!("⚠ permission requested ({permission}); approval UI arrives in M5")
                        .into(),
                );
            }
            DomainEvent::ConfigChanged => {}
        }
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

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(div().text_sm().child(header))
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
/// The session must exist before this is called — the composition root starts
/// the agent and creates the session, then opens the window.
pub fn open_chat_window(
    service: Arc<SessionService>,
    bus: EventBus,
    agent: AgentId,
    session: SessionId,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| ChatView::new(service, bus, agent, session, window, cx));
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
