//! `WelcomeView` state and intents — the launcher half of the welcome panel.
//!
//! The launcher replaces the old CLI-argument flow: instead of naming an agent
//! on the command line, the user picks one here, optionally types a first
//! message, and starts a chat. Starting lazily boots the agent subprocess (via
//! [`AgentRegistry`]), opens a session (via [`SessionService`]), and hands off to
//! a [`ChatView`] window — forwarding the typed message as the first turn.
//!
//! Recent sessions are listed too; resuming one reconnects the *currently
//! selected* agent to it, because a stored timeline does not record which agent
//! produced it.
//!
//! As with the chat panel, this module owns the struct, its construction, and
//! the intents; the pure rendering lives in the sibling [`view`] module.

mod view;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Root, WindowExt as _,
    input::{InputEvent, InputState},
    notification::Notification,
};

use agentx_app::{SessionService, WorkspaceService};
use agentx_bus::EventBus;
use agentx_domain::{AgentId, AgentRegistry, AgentStatus, Config, DomainEvent, SessionId};

/// A recent session shown in the launcher: its id plus a display label (the
/// first user message, or a short id fallback).
pub(crate) struct RecentSession {
    id: SessionId,
    label: SharedString,
}

/// The launcher window's content: choose an agent, optionally type a first
/// message, then start or resume a chat.
///
/// It holds the two domain ports it drives ([`AgentRegistry`] to boot agents,
/// `SessionService` to open sessions), the bus (so each launch can subscribe a
/// fresh event stream *before* the agent starts), and the loaded [`Config`] that
/// supplies the agent list and proxy.
pub struct WelcomeView {
    registry: Arc<dyn AgentRegistry>,
    service: Arc<SessionService>,
    workspace_service: Arc<WorkspaceService>,
    bus: EventBus,
    config: Config,
    cwd: PathBuf,
    /// Configured agent ids, sorted for stable display.
    agents: Vec<AgentId>,
    /// The agent the next launch will use; defaults to the first configured one.
    selected: Option<AgentId>,
    /// Recent sessions on disk, newest first.
    recent: Vec<RecentSession>,
    /// The optional first-message composer.
    input: Entity<InputState>,
    /// A launch is in flight; the controls are disabled until it resolves.
    launching: bool,
    /// A transient error from the last launch, shown beneath the controls.
    status: Option<SharedString>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl WelcomeView {
    fn new(
        registry: Arc<dyn AgentRegistry>,
        service: Arc<SessionService>,
        workspace_service: Arc<WorkspaceService>,
        bus: EventBus,
        config: Config,
        cwd: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut agents: Vec<AgentId> = config
            .agents
            .keys()
            .map(|name| AgentId::from(name.clone()))
            .collect();
        agents.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let selected = agents.first().cloned();

        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Describe your task…  (optional, press Enter to start)")
        });
        let subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |this, _input, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.start_chat(window, cx);
                }
            },
        )];

        let view = Self {
            registry,
            service,
            workspace_service,
            bus,
            config,
            cwd,
            agents,
            selected,
            recent: Vec::new(),
            input,
            launching: false,
            status: None,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        view.refresh_recent(cx);
        view
    }

    /// Reload the recent-session list (everything on disk), each labelled by its
    /// first user message. Newest first (the store orders by modification time).
    fn refresh_recent(&self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let ids = service.list_sessions().await.unwrap_or_default();
            let mut recent = Vec::new();
            for id in ids {
                let history = service.history(&id).await.unwrap_or_default();
                let label = crate::chat::session_label(&id, &history);
                recent.push(RecentSession { id, label });
            }
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.recent = recent;
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Choose the agent the next launch will use.
    fn select_agent(&mut self, agent: AgentId, cx: &mut Context<Self>) {
        self.selected = Some(agent);
        self.status = None;
        cx.notify();
    }

    /// Start a fresh chat with the selected agent, forwarding any typed message.
    fn start_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = self.input.read(cx).value().to_string();
        let prompt = (!prompt.trim().is_empty()).then_some(prompt);
        self.launch(None, prompt, window, cx);
    }

    /// Resume a recent session, reconnecting the selected agent to it.
    fn resume_recent(&mut self, session: SessionId, window: &mut Window, cx: &mut Context<Self>) {
        self.launch(Some(session), None, window, cx);
    }

    /// Boot the selected agent if needed, open the session (new or `resume`),
    /// then open a chat window for it. The event receiver is subscribed *before*
    /// the agent is touched, so the chat replays session-setup events instead of
    /// missing them (the broadcast bus has no replay for late subscribers).
    fn launch(
        &mut self,
        resume: Option<SessionId>,
        prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.launching {
            return;
        }
        let Some(agent) = self.selected.clone() else {
            self.status = Some("select an agent first".into());
            cx.notify();
            return;
        };
        let Some(agent_config) = self.config.agents.get(agent.as_str()).cloned() else {
            self.status = Some(format!("agent `{agent}` is not configured").into());
            cx.notify();
            return;
        };

        self.launching = true;
        self.status = None;
        // Clear the composer; the typed text travels with the launch.
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();

        let registry = self.registry.clone();
        let service = self.service.clone();
        let workspace_service = self.workspace_service.clone();
        let bus = self.bus.clone();
        let proxy = self.config.proxy.clone();
        let cwd = self.cwd.clone();
        // Subscribe before the agent is started, so events emitted during session
        // setup (slash commands, config options, status) are buffered for the
        // chat window rather than lost.
        let events = bus.subscribe::<DomainEvent>();

        cx.spawn_in(window, async move |this, cx| {
            // Boot the agent unless it is already running.
            let running = registry
                .agents()
                .into_iter()
                .any(|d| d.id == agent && !matches!(d.status, AgentStatus::Unavailable { .. }));
            let started = if running {
                Ok(())
            } else {
                let _ = registry.set_proxy(proxy).await;
                registry.add_agent(agent.clone(), agent_config).await
            };

            let result = match started {
                Ok(()) => match &resume {
                    Some(session) => service.resume_session(&agent, session, &cwd, &[]).await,
                    None => service.new_session(&agent, &cwd, &[]).await,
                },
                Err(error) => Err(error),
            };

            let _ = this.update_in(cx, |this, window, cx| {
                this.launching = false;
                match result {
                    Ok(init) => {
                        crate::open_chat_window(
                            service,
                            registry,
                            workspace_service,
                            events,
                            agent,
                            cwd,
                            init,
                            prompt,
                            cx,
                        );
                        this.refresh_recent(cx);
                    }
                    Err(error) => {
                        let text: SharedString = format!("launch failed › {error}").into();
                        this.status = Some(text.clone());
                        window.push_notification(Notification::error(text), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// Open the launcher window: an agent picker, a first-message composer, and a
/// recent-session list.
///
/// This is the app's entry surface (the composition root opens it at startup).
/// Each agent is booted lazily on first launch, so opening this window starts no
/// subprocesses by itself.
pub fn open_welcome_window(
    registry: Arc<dyn AgentRegistry>,
    service: Arc<SessionService>,
    workspace_service: Arc<WorkspaceService>,
    bus: EventBus,
    config: Config,
    cwd: PathBuf,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(620.0), px(680.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let welcome = cx.new(|cx| {
            WelcomeView::new(
                registry,
                service,
                workspace_service,
                bus,
                config,
                cwd,
                window,
                cx,
            )
        });
        // The first level on the window must be a `Root`.
        cx.new(|cx| Root::new(welcome, window, cx).bg(cx.theme().background))
    });
}
