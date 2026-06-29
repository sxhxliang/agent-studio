//! `ChatView` state and intents — the view-model half of the chat panel.
//!
//! This module owns the struct, its construction, and every *intent* (the
//! methods that drive [`SessionService`] and mutate view state). The pure
//! rendering lives in the sibling [`view`] module. Intents are `private` but
//! visible to `view` (a descendant module), which is how the render closures
//! call back into them.

mod acp_bridge;
mod sessions;
mod view;

pub use sessions::SessionsPanel;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use chrono::{DateTime, Utc};
use gpui::*;
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Root, WindowExt as _,
    input::{InputEvent, InputState},
    notification::Notification,
    spinner::Spinner,
};

use agentx_acp_ui::{
    AcpMessageStream, AcpMessageStreamOptions, DiffSummaryOptions,
    PermissionRequest as AcpPermissionRequest, PermissionRequestOptions, PermissionRequestView,
    ToolCallItemOptions,
};
use agentx_app::{ConfigService, FileService, SessionService, WorkspaceService};
use agentx_bus::Receiver;
use agentx_domain::{
    AgentId, AgentRegistry, ContentBlock, DomainEvent, PermissionOutcome, PermissionRequest,
    PersistedEvent, SessionConfigOption, SessionEvent, SessionId, SessionInit, SessionMode,
    SessionStatus, SlashCommand,
};

use crate::components::{CodeSelection, FileItem, ImageAttachment};
use crate::panels::{SessionManagerPanel, TaskPanel};

const AUTO_SCROLL_THRESHOLD_PX: f32 = 120.0;

/// One row in the conversation: a rich timeline event, a terse system note, or
/// an error (rendered prominently).
#[derive(Clone)]
pub(crate) enum Entry {
    Event(SessionEvent),
    Note(SharedString),
    Error(SharedString),
}

/// A past session loaded read-only from disk for browsing in the sidebar.
pub(crate) struct ViewedSession {
    id: SessionId,
    entries: Vec<Entry>,
    stream: Entity<AcpMessageStream>,
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
    file_service: Arc<FileService>,
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
    /// ACP-compatible message stream that recreates the legacy conversation UI.
    message_stream: Entity<AcpMessageStream>,
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
    /// File suggestions for the active `@`-mention, fed to the composer.
    file_suggestions: Vec<FileItem>,
    /// Files attached via `@`-mention, sent as resource links on submit.
    selected_files: Vec<String>,
    /// Images pasted into the composer.
    pasted_images: Vec<ImageAttachment>,
    /// Code snippets forwarded from an editor surface.
    code_selections: Vec<CodeSelection>,
    scroll: ScrollHandle,
    session_status: SessionStatus,
    last_active: DateTime<Utc>,
    message_count: usize,
    busy: bool,
    _subscriptions: Vec<Subscription>,
}

impl ChatView {
    fn create_message_stream(cx: &mut Context<Self>) -> Entity<AcpMessageStream> {
        let tool_call_options =
            ToolCallItemOptions::default().on_open_detail(Arc::new(|tool_call, _window, cx| {
                crate::open_tool_call_detail_window(
                    acp_bridge::acp_tool_call_to_domain(tool_call),
                    cx,
                );
            }));
        let diff_summary_options = DiffSummaryOptions {
            on_open_tool_call: Some(Arc::new(|tool_call, _window, cx| {
                crate::open_tool_call_detail_window(
                    acp_bridge::acp_tool_call_to_domain(tool_call),
                    cx,
                );
            })),
        };

        cx.new(|_| {
            AcpMessageStream::with_options(AcpMessageStreamOptions {
                agent_icon_provider: Arc::new(|_| Icon::new(IconName::Bot)),
                tool_call_item_options: tool_call_options,
                diff_summary_options,
            })
        })
    }

    fn should_auto_scroll(&self) -> bool {
        let max_offset = self.scroll.max_offset().y;
        let offset = self.scroll.offset().y;
        let distance_to_bottom = max_offset + offset;
        distance_to_bottom <= px(AUTO_SCROLL_THRESHOLD_PX)
    }

    fn reset_message_stream(&mut self, cx: &mut Context<Self>) {
        self.message_stream = Self::create_message_stream(cx);
    }

    pub(crate) fn new(
        service: Arc<SessionService>,
        file_service: Arc<FileService>,
        mut events: Receiver<DomainEvent>,
        agent: AgentId,
        cwd: PathBuf,
        init: SessionInit,
        initial_content: Option<Vec<ContentBlock>>,
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
        let message_stream = Self::create_message_stream(cx);

        let subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.submit(window, cx),
                InputEvent::Change => this.on_input_change(cx),
                _ => {}
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
                            let should_scroll = this.should_auto_scroll();
                            this.record(event, cx);
                            if should_scroll {
                                this.scroll.scroll_to_bottom();
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();

        let view = Self {
            service,
            file_service,
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
            message_stream,
            sessions: Vec::new(),
            viewed: None,
            expanded: HashSet::new(),
            expanded_thoughts: HashSet::new(),
            pending: Vec::new(),
            file_suggestions: Vec::new(),
            selected_files: Vec::new(),
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            scroll: ScrollHandle::new(),
            session_status: SessionStatus::Pending,
            last_active: Utc::now(),
            message_count: 0,
            busy: false,
            _subscriptions: subscriptions,
        };
        view.refresh_sessions(cx);

        // Initial content from the launcher/welcome panel is sent as the first
        // turn once the view exists as an entity, so streamed events have a
        // receiver attached before the prompt starts.
        if let Some(content) = initial_content.filter(|content| !content.is_empty()) {
            cx.spawn_in(window, async move |this, cx| {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.send_content(content, window, cx);
                });
            })
            .detach();
        }
        view
    }

    /// The id of the live session (the one new prompts go to).
    pub(crate) fn current_session(&self) -> SessionId {
        self.session.clone()
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
    pub(crate) fn view_session(&mut self, id: SessionId, cx: &mut Context<Self>) {
        if id == self.session {
            self.viewed = None;
            cx.notify();
            return;
        }
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let history = service.history(&id).await.unwrap_or_default();
            let entries: Vec<Entry> = history
                .into_iter()
                .map(|event| Entry::Event(event.event))
                .collect();
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        let stream = Self::create_message_stream(cx);
                        this.replay_entries_to_stream(&stream, &id, &entries, cx);
                        this.viewed = Some(ViewedSession {
                            id,
                            entries,
                            stream,
                        });
                        this.scroll.scroll_to_bottom();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Start a fresh session and switch the live view to it.
    pub(crate) fn start_new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                        this.reset_message_stream(cx);
                        this.viewed = None;
                        this.config_options = init.config_options;
                        this.modes = init.modes;
                        this.current_mode = init.current_mode;
                        this.commands = init.commands;
                        this.session_status = SessionStatus::Pending;
                        this.last_active = Utc::now();
                        this.message_count = 0;
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
                        let label: String = init.session_id.as_str().chars().take(8).collect();
                        let entries = this.viewed.take().map(|v| v.entries).unwrap_or_default();
                        this.session = init.session_id;
                        this.live = entries.clone();
                        this.reset_message_stream(cx);
                        this.replay_entries_to_stream(
                            &this.message_stream.clone(),
                            &id,
                            &entries,
                            cx,
                        );
                        this.config_options = init.config_options;
                        this.modes = init.modes;
                        this.current_mode = init.current_mode;
                        this.commands = init.commands;
                        this.session_status = SessionStatus::Idle;
                        this.last_active = Utc::now();
                        this.message_count = entries.len();
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
        if text.trim().is_empty()
            && self.pasted_images.is_empty()
            && self.code_selections.is_empty()
            && self.selected_files.is_empty()
        {
            return;
        }
        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));

        // Send text plus every attachment the legacy composer supported.
        let mut content = Vec::new();
        if !text.trim().is_empty() {
            content.push(ContentBlock::text(text));
        }
        for image in self.pasted_images.drain(..) {
            content.push(ContentBlock::Image {
                mime_type: image.mime_type,
                data: image.data,
            });
        }
        for selection in self.code_selections.drain(..) {
            content.push(ContentBlock::text(format_code_selection_as_context(
                &selection,
            )));
        }
        for path in self.selected_files.drain(..) {
            let name = std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_string())
                .unwrap_or_else(|| path.clone());
            content.push(ContentBlock::ResourceLink {
                name,
                uri: path,
                mime_type: None,
            });
        }
        self.send_content(content, window, cx);
    }

    pub(crate) fn send_content(
        &mut self,
        content: Vec<ContentBlock>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || content.is_empty() {
            return;
        }
        self.file_suggestions.clear();
        self.busy = true;
        cx.notify();

        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.send_message(&session, content).await;
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

    /// On every keystroke, if the cursor is in an `@`-mention, fetch matching
    /// files from the workspace and feed them to the composer's suggestion list.
    fn on_input_change(&mut self, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        let Some((_, query)) = active_mention(&value) else {
            if !self.file_suggestions.is_empty() {
                self.file_suggestions.clear();
                cx.notify();
            }
            return;
        };
        let service = self.file_service.clone();
        let cwd = self.cwd.clone();
        cx.spawn(async move |this, cx| {
            let entries = service.list_files(&cwd, &query).await.unwrap_or_default();
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.file_suggestions = entries
                            .into_iter()
                            .map(|entry| {
                                FileItem::new(entry.name, entry.relative_path, entry.is_dir)
                            })
                            .collect();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Confirm a file suggestion: replace the `@query` token with the path and
    /// remember the file so it is sent as a resource link.
    fn apply_file_mention(&mut self, file: FileItem, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        if let Some((at, _)) = active_mention(&value) {
            let mut path = file.relative_path.clone();
            if file.is_folder && !path.ends_with('/') {
                path.push('/');
            }
            let new_value = format!("{}@{} ", &value[..at], path);
            self.input
                .update(cx, |state, cx| state.set_value(new_value, window, cx));
        }
        if !self.selected_files.contains(&file.relative_path) {
            self.selected_files.push(file.relative_path);
        }
        self.file_suggestions.clear();
        cx.notify();
    }

    /// Drop a mentioned file (its chip's close button).
    fn remove_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.selected_files.len() {
            self.selected_files.remove(index);
            cx.notify();
        }
    }

    fn remove_image(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.pasted_images.len() {
            self.pasted_images.remove(index);
            cx.notify();
        }
    }

    fn remove_code_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.code_selections.len() {
            self.code_selections.remove(index);
            cx.notify();
        }
    }

    fn handle_paste(&mut self, cx: &mut Context<Self>) {
        let Some(clipboard_item) = cx.read_from_clipboard() else {
            return;
        };

        let mut changed = false;
        for entry in clipboard_item.entries() {
            if let ClipboardEntry::Image(image) = entry {
                self.pasted_images.push(image_attachment(image.clone()));
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    /// Cancel the in-flight turn. The agent stops; the streamed status change
    /// returns the session to Idle, which clears `busy` in [`record`](Self::record).
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.busy {
            return;
        }
        let service = self.service.clone();
        let session = self.session.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service.cancel(&session).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.report_error(format!("cancel error › {error}"), window, cx);
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
            let result = service
                .set_config_option(&session, &config_id, &value)
                .await;
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
            let result = service
                .resolve_permission(&session, &permission_id, outcome)
                .await;
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

    fn record(&mut self, event: DomainEvent, cx: &mut Context<Self>) {
        match event {
            DomainEvent::SessionStatusChanged { session, status } if session == self.session => {
                self.session_status = status;
                self.last_active = Utc::now();
                self.busy = status.is_busy();
                if matches!(
                    status,
                    SessionStatus::Idle | SessionStatus::Completed | SessionStatus::Failed
                ) {
                    self.message_stream.update(cx, |stream, cx| {
                        stream.mark_last_complete(cx);
                        stream.add_diff_summary_if_needed(cx);
                    });
                }
            }
            DomainEvent::SessionAppended { session, event } if session == self.session => {
                self.apply_session_event_to_live_stream(&event, cx);
                self.live.push(Entry::Event(event));
                self.message_count += 1;
            }
            DomainEvent::PermissionRequested { request } if request.session == self.session => {
                self.add_permission_request(request, cx);
            }
            DomainEvent::SessionCommandsChanged { session, commands }
                if session == self.session =>
            {
                let update = acp_bridge::commands_to_update(&commands);
                self.message_stream.update(cx, |stream, cx| {
                    stream.process_update(
                        update,
                        Some(self.session.as_str()),
                        Some(self.agent.as_str()),
                        cx,
                    );
                });
                self.commands = commands;
            }
            DomainEvent::SessionConfigChanged { session, options } if session == self.session => {
                self.config_options = options;
            }
            DomainEvent::AgentStatusChanged { .. }
            | DomainEvent::SessionStatusChanged { .. }
            | DomainEvent::SessionAppended { .. }
            | DomainEvent::PermissionRequested { .. }
            | DomainEvent::SessionCommandsChanged { .. }
            | DomainEvent::SessionConfigChanged { .. }
            | DomainEvent::ConfigChanged
            | DomainEvent::WorkspaceAdded { .. }
            | DomainEvent::WorkspaceRemoved { .. }
            | DomainEvent::TaskAdded { .. }
            | DomainEvent::TaskRemoved { .. }
            | DomainEvent::TaskStatusChanged { .. } => {}
        }
    }

    fn apply_session_event_to_live_stream(&mut self, event: &SessionEvent, cx: &mut Context<Self>) {
        self.apply_session_event_to_stream(&self.message_stream.clone(), &self.session, event, cx);
    }

    fn replay_entries_to_stream(
        &self,
        stream: &Entity<AcpMessageStream>,
        session: &SessionId,
        entries: &[Entry],
        cx: &mut Context<Self>,
    ) {
        for entry in entries {
            if let Entry::Event(event) = entry {
                self.apply_session_event_to_stream(stream, session, event, cx);
            }
        }
        stream.update(cx, |stream, cx| {
            stream.add_diff_summary_if_needed(cx);
        });
    }

    fn apply_session_event_to_stream(
        &self,
        stream: &Entity<AcpMessageStream>,
        session: &SessionId,
        event: &SessionEvent,
        cx: &mut Context<Self>,
    ) {
        for update in acp_bridge::session_event_to_updates(event) {
            stream.update(cx, |stream, cx| {
                stream.process_update(
                    update,
                    Some(session.as_str()),
                    Some(self.agent.as_str()),
                    cx,
                );
            });
        }
        if matches!(event, SessionEvent::Stopped { .. }) {
            stream.update(cx, |stream, cx| {
                stream.mark_last_complete(cx);
            });
        }
    }

    fn add_permission_request(&mut self, request: PermissionRequest, cx: &mut Context<Self>) {
        let service = self.service.clone();
        let session = request.session.clone();
        let (tool_call, options) = acp_bridge::permission_request_to_acp(&request);
        let handler: agentx_acp_ui::PermissionResponseHandler =
            Arc::new(move |permission_id, response, cx| {
                let service = service.clone();
                let session = session.clone();
                let outcome = acp_bridge::permission_response_to_outcome(response);
                cx.spawn(async move |_this, _cx| {
                    if let Err(error) = service
                        .resolve_permission(&session, &permission_id, outcome)
                        .await
                    {
                        log::error!("permission response failed: {error}");
                    }
                })
                .detach();
            });
        let item = cx.new(|_| {
            AcpPermissionRequest::with_options(
                request.id.to_string(),
                request.session.to_string(),
                &tool_call,
                options,
                PermissionRequestOptions {
                    on_response: Some(handler),
                },
            )
        });
        let view = cx.new(|_| PermissionRequestView::from_entity(item));
        self.pending.push(request);
        self.message_stream.update(cx, |stream, cx| {
            stream.add_permission_request(view, cx);
        });
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
    registry: Arc<dyn AgentRegistry>,
    workspace_service: Arc<WorkspaceService>,
    config_service: Arc<ConfigService>,
    file_service: Arc<FileService>,
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

    let initial_content = initial_prompt.map(|prompt| vec![ContentBlock::text(prompt)]);
    let _ = cx.open_window(options, |window, cx| {
        let chat = cx.new(|cx| {
            ChatView::new(
                service.clone(),
                file_service,
                events,
                agent,
                cwd,
                init,
                initial_content,
                window,
                cx,
            )
        });
        let sessions = cx.new(|cx| SessionsPanel::new(chat.clone(), cx));
        let manager =
            cx.new(|cx| SessionManagerPanel::new(service.clone(), registry, chat.clone(), cx));
        let tasks = cx.new(|cx| {
            TaskPanel::new(
                workspace_service,
                config_service,
                Some(chat.clone()),
                window,
                cx,
            )
        });
        let workspace = cx.new(|cx| {
            crate::workspace::Workspace::with_chat(chat, sessions, manager, tasks, window, cx)
        });
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

/// If the cursor sits in an `@`-mention (the text after the last `@` contains no
/// whitespace), return the `@`'s byte offset and the query after it.
pub(crate) fn active_mention(value: &str) -> Option<(usize, String)> {
    let at = value.rfind('@')?;
    let after = &value[at + 1..];
    if after.contains(char::is_whitespace) {
        return None;
    }
    Some((at, after.to_string()))
}

pub(crate) fn format_code_selection_as_context(selection: &CodeSelection) -> String {
    let line_info = if selection.start_line == selection.end_line {
        format!("Line {}", selection.start_line)
    } else {
        format!("Lines {}-{}", selection.start_line, selection.end_line)
    };

    format!(
        "```\n// File: {} ({})\n\n```",
        selection.file_path, line_info
    )
}

pub(crate) fn image_attachment(image: Image) -> ImageAttachment {
    let mime_type = mime_type_for_format(image.format).to_string();
    let extension = extension_for_format(image.format);
    let filename = format!(
        "pasted-image-{}.{}",
        Utc::now().timestamp_millis(),
        extension
    );
    let data = base64::engine::general_purpose::STANDARD.encode(image.bytes());
    ImageAttachment {
        filename,
        mime_type,
        data,
    }
}

fn mime_type_for_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Webp => "image/webp",
        ImageFormat::Gif => "image/gif",
        ImageFormat::Svg => "image/svg+xml",
        ImageFormat::Bmp => "image/bmp",
        ImageFormat::Tiff => "image/tiff",
        ImageFormat::Ico => "image/icon",
        ImageFormat::Pnm => "image/x-portable-anymap",
    }
}

fn extension_for_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        ImageFormat::Ico => "ico",
        ImageFormat::Pnm => "pnm",
    }
}
