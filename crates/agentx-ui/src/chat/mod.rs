//! `ChatView` state and intents — the view-model half of the chat panel.
//!
//! This module owns the struct, its construction, and every *intent* (the
//! methods that drive [`SessionService`] and mutate view state). The pure
//! rendering lives in the sibling [`view`] module. Intents are `private` but
//! visible to `view` (a descendant module), which is how the render closures
//! call back into them.

mod sessions;
mod view;

pub use sessions::SessionsPanel;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Root, WindowExt as _,
    input::{InputEvent, InputState},
    notification::Notification,
};

use agentx_app::SessionService;
use agentx_bus::Receiver;
use agentx_domain::{
    AgentId, ContentBlock, DomainEvent, PermissionOutcome, PermissionRequest, PersistedEvent,
    SessionConfigOption, SessionEvent, SessionId, SessionInit, SessionMode, SessionStatus,
    SlashCommand,
};

/// One row in the conversation: a rich timeline event, a terse system note, or
/// an error (rendered prominently).
pub(crate) enum Entry {
    Event(SessionEvent),
    Note(SharedString),
    Error(SharedString),
}

/// A past session loaded read-only from disk for browsing in the sidebar.
pub(crate) struct ViewedSession {
    id: SessionId,
    entries: Vec<Entry>,
}

/// A sidebar entry for a sibling session: its id plus a display label (the first
/// user message, or a short id fallback).
pub(crate) struct SessionMeta {
    id: SessionId,
    label: SharedString,
}

/// A single-session chat window over [`SessionService`].
///
/// It owns only view state plus the use-case handle it drives; it never touches
/// an adapter. Streamed output is observed, not requested: a background task
/// folds every [`DomainEvent`] into the timeline via [`record`](Self::record).
pub struct ChatView {
    service: Arc<SessionService>,
    agent: AgentId,
    session: SessionId,
    /// The working directory, needed to resume a session.
    cwd: PathBuf,
    /// ACP config options (model / mode / …) the agent advertised, each carrying
    /// its own current value. Preferred over [`modes`](Self::modes) when present.
    config_options: Vec<SessionConfigOption>,
    /// Legacy mode list, used as a fallback for agents that advertise modes but
    /// no config options, with the current pick.
    modes: Vec<SessionMode>,
    current_mode: Option<String>,
    /// Slash commands the agent advertises; updated via a notification.
    commands: Vec<SlashCommand>,
    input: Entity<InputState>,
    /// The panel's focus handle (required by the dock `Panel` trait).
    focus_handle: FocusHandle,
    /// The live session's timeline, accumulated from the bus.
    live: Vec<Entry>,
    /// Sibling sessions on disk, for the sidebar (excludes the live one).
    sessions: Vec<SessionMeta>,
    /// A past session being browsed read-only; `None` means the live session.
    viewed: Option<ViewedSession>,
    /// Tool-call ids whose detail is expanded.
    expanded: HashSet<String>,
    /// Thought rows expanded by timeline index.
    expanded_thoughts: HashSet<usize>,
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
        cwd: PathBuf,
        init: SessionInit,
        initial_prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let SessionInit {
            session_id,
            config_options,
            modes,
            current_mode,
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
            cwd,
            config_options,
            modes,
            current_mode,
            commands,
            input,
            focus_handle: cx.focus_handle(),
            live: Vec::new(),
            sessions: Vec::new(),
            viewed: None,
            expanded: HashSet::new(),
            expanded_thoughts: HashSet::new(),
            pending: Vec::new(),
            scroll: ScrollHandle::new(),
            busy: false,
            _subscriptions: subscriptions,
        };
        view.refresh_sessions(cx);

        // An initial prompt from the launcher is sent as the first turn, once the
        // view exists as an entity so `submit` can drive it.
        if let Some(text) = initial_prompt.filter(|t| !t.trim().is_empty()) {
            cx.spawn_in(window, async move |this, cx| {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.input
                        .update(cx, |state, cx| state.set_value(text, window, cx));
                    this.submit(window, cx);
                });
            })
            .detach();
        }
        view
    }

    /// Reload the sibling session list (everything on disk except the live one),
    /// each labelled by its first user message. Newest first (the store orders
    /// by modification time).
    fn refresh_sessions(&self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let live = self.session.clone();
        cx.spawn(async move |this, cx| {
            let ids = service.list_sessions().await.unwrap_or_default();
            let mut metas = Vec::new();
            for id in ids {
                if id == live {
                    continue;
                }
                let history = service.history(&id).await.unwrap_or_default();
                let label = session_label(&id, &history);
                metas.push(SessionMeta { id, label });
            }
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.sessions = metas;
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

    /// Start a fresh session and switch the live view to it.
    fn start_new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let agent = self.agent.clone();
        let service = self.service.clone();
        let cwd = self.cwd.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.new_session(&agent, &cwd, &[]).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(init) => {
                        this.session = init.session_id;
                        this.live = Vec::new();
                        this.viewed = None;
                        this.config_options = init.config_options;
                        this.modes = init.modes;
                        this.current_mode = init.current_mode;
                        this.commands = init.commands;
                        this.note("· new session");
                        this.refresh_sessions(cx);
                    }
                    Err(error) => {
                        this.report_error(format!("new session failed › {error}"), window, cx)
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete a session's persisted history and drop it from the sidebar.
    fn remove_session(&mut self, id: SessionId, cx: &mut Context<Self>) {
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let _ = service.delete_session(&id).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.sessions.retain(|meta| meta.id != id);
                        if this.viewed.as_ref().is_some_and(|v| v.id == id) {
                            this.viewed = None;
                        }
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Reconnect the agent to the session being browsed and make it the live
    /// session, so the user can continue it. Agents that don't support
    /// resumption surface an error note instead.
    fn resume_viewed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(viewed) = self.viewed.as_ref() else {
            return;
        };
        let id = viewed.id.clone();
        let agent = self.agent.clone();
        let service = self.service.clone();
        let cwd = self.cwd.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.resume_session(&agent, &id, &cwd, &[]).await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(init) => {
                        let label: String =
                            init.session_id.as_str().chars().take(8).collect();
                        let entries =
                            this.viewed.take().map(|v| v.entries).unwrap_or_default();
                        this.session = init.session_id;
                        this.live = entries;
                        this.config_options = init.config_options;
                        this.modes = init.modes;
                        this.current_mode = init.current_mode;
                        this.commands = init.commands;
                        this.note(format!("· resumed {label}"));
                        this.refresh_sessions(cx);
                        this.scroll.scroll_to_bottom();
                    }
                    Err(error) => {
                        this.report_error(format!("resume failed › {error}"), window, cx);
                    }
                }
                cx.notify();
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
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .send_message(&session, vec![ContentBlock::text(text)])
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                if let Err(error) = result {
                    this.report_error(format!("error › {error}"), window, cx);
                    this.scroll.scroll_to_bottom();
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Switch the session's mode, updating the local selection optimistically.
    fn choose_mode(&mut self, mode_id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_mode.as_deref() == Some(mode_id.as_str()) {
            return;
        }
        self.current_mode = Some(mode_id.clone());
        cx.notify();
        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.set_mode(&session, &mode_id).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.report_error(format!("set mode error › {error}"), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Change one of the session's config options, updating the local selection
    /// optimistically. The agent confirms by republishing the full option set as
    /// [`DomainEvent::SessionConfigChanged`], handled in [`record`](Self::record).
    fn choose_config_option(
        &mut self,
        config_id: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(option) = self.config_options.iter_mut().find(|o| o.id == config_id) else {
            return;
        };
        if option.current_value == value {
            return;
        }
        option.current_value = value.clone();
        cx.notify();
        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.set_config_option(&session, &config_id, &value).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.report_error(format!("set option error › {error}"), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Insert a slash command into the input for the user to complete and send.
    fn insert_command(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |state, cx| {
            state.insert(&format!("/{name} "), window, cx);
        });
    }

    /// Toggle whether a tool call's detail is expanded.
    fn toggle_tool(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        cx.notify();
    }

    /// Toggle whether an agent-thought row's detail is expanded.
    fn toggle_thought(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.expanded_thoughts.remove(&index) {
            self.expanded_thoughts.insert(index);
        }
        cx.notify();
    }

    /// Answer a pending permission request and forward the decision to the agent.
    fn resolve(
        &mut self,
        permission_id: String,
        outcome: PermissionOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        cx.spawn_in(window, async move |this, cx| {
            let result = service.resolve_permission(&session, &permission_id, outcome).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.report_error(format!("permission error › {error}"), window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Append a terse system note to the timeline.
    fn note(&mut self, text: impl Into<SharedString>) {
        self.live.push(Entry::Note(text.into()));
    }

    /// Append a prominent error line to the timeline.
    fn error_note(&mut self, text: impl Into<SharedString>) {
        self.live.push(Entry::Error(text.into()));
    }

    /// Report an error: a red line in the timeline plus a transient toast.
    fn report_error(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = text.into();
        self.error_note(text.clone());
        window.push_notification(Notification::error(text), cx);
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
            DomainEvent::SessionConfigChanged { options, .. } => {
                self.config_options = options;
            }
            DomainEvent::ConfigChanged => {}
        }
    }
}

/// Open a window hosting a [`ChatView`] for an already-created session.
///
/// `events` must be a receiver subscribed *before* the agent was started, so the
/// view replays the session-setup events (slash commands, status) instead of
/// missing them. The composition root starts the agent, creates the session,
/// then opens the window with its [`SessionInit`]. `initial_prompt`, if set, is
/// sent as the first turn (the launcher uses this to forward a typed message).
pub fn open_chat_window(
    service: Arc<SessionService>,
    events: Receiver<DomainEvent>,
    agent: AgentId,
    cwd: PathBuf,
    init: SessionInit,
    initial_prompt: Option<String>,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(900.0), px(680.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let chat = cx
            .new(|cx| ChatView::new(service, events, agent, cwd, init, initial_prompt, window, cx));
        let sessions = cx.new(|cx| SessionsPanel::new(chat.clone(), cx));
        let workspace = cx.new(|cx| crate::workspace::Workspace::new(chat, sessions, window, cx));
        // The first level on the window must be a `Root`.
        cx.new(|cx| Root::new(workspace, window, cx).bg(cx.theme().background))
    });
}

/// The plain-text concatenation of a message's content blocks.
pub(crate) fn plain_text(content: &[ContentBlock]) -> String {
    content
        .iter()
        .filter_map(ContentBlock::as_plain_text)
        .collect::<Vec<_>>()
        .join("")
}

/// A sidebar label for a session: its first user message's first line
/// (truncated), or a short id when there is no message yet.
pub(crate) fn session_label(id: &SessionId, history: &[PersistedEvent]) -> SharedString {
    for event in history {
        if let SessionEvent::UserMessage { content } = &event.event {
            let text = plain_text(content);
            let line = text.lines().next().unwrap_or("").trim();
            if !line.is_empty() {
                return truncate(line, 28).into();
            }
        }
    }
    id.as_str().chars().take(8).collect::<String>().into()
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() > max {
        let mut out: String = text.chars().take(max).collect();
        out.push('…');
        out
    } else {
        text.to_string()
    }
}
