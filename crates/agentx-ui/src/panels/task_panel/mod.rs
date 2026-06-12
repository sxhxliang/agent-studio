//! The task panel — workspaces and their tasks, shown as a Tree or a Timeline.
//!
//! A container over [`WorkspaceService`] (when wired) that opens a task's session
//! in the [`ChatView`]. Both deps are optional so the preview gallery can render
//! it with mock data and no backing service. `mod.rs` holds state + intents;
//! [`view`] renders.

mod view;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use gpui::*;
use gpui_component::input::{InputEvent, InputState};

use agentx_app::{ConfigService, WorkspaceService};
use agentx_domain::{AgentId, SessionId, SessionStatus, TaskId, WorkspaceId};

use crate::chat::ChatView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Tree,
    Timeline,
}

pub(crate) struct TaskVm {
    pub id: TaskId,
    pub name: String,
    pub agent: AgentId,
    pub session: Option<SessionId>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_message: Option<String>,
}

pub(crate) struct WorkspaceGroupVm {
    pub id: WorkspaceId,
    pub name: String,
    pub tasks: Vec<TaskVm>,
    pub expanded: bool,
}

pub struct TaskPanel {
    service: Option<Arc<WorkspaceService>>,
    config_service: Option<Arc<ConfigService>>,
    chat: Option<Entity<ChatView>>,
    groups: Vec<WorkspaceGroupVm>,
    view_mode: ViewMode,
    selected: Option<TaskId>,
    search: Entity<InputState>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl TaskPanel {
    pub fn new(
        service: Arc<WorkspaceService>,
        config_service: Arc<ConfigService>,
        chat: Entity<ChatView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let panel = Self::build(Some(service), Some(config_service), Some(chat), window, cx);
        panel.load(cx);
        panel
    }

    /// A preview instance with mock data and no backing service (gallery only).
    pub fn preview(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self::build(None, None, None, window, cx);
        panel.groups = mock_groups();
        panel
    }

    fn build(
        service: Option<Arc<WorkspaceService>>,
        config_service: Option<Arc<ConfigService>>,
        chat: Option<Entity<ChatView>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tasks…"));
        let subscriptions =
            vec![
                cx.subscribe_in(&search, window, |_, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                }),
            ];
        Self {
            service,
            config_service,
            chat,
            groups: Vec::new(),
            view_mode: ViewMode::Tree,
            selected: None,
            search,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Reload workspaces + tasks from the service, preserving collapse state.
    fn load(&self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let data = service.workspaces_with_tasks().await.unwrap_or_default();
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.rebuild(data);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn rebuild(&mut self, data: Vec<(agentx_domain::Workspace, Vec<agentx_app::TaskView>)>) {
        let prev: HashMap<WorkspaceId, bool> = self
            .groups
            .iter()
            .map(|g| (g.id.clone(), g.expanded))
            .collect();
        self.groups = data
            .into_iter()
            .map(|(workspace, tasks)| WorkspaceGroupVm {
                expanded: prev.get(&workspace.id).copied().unwrap_or(true),
                id: workspace.id,
                name: workspace.name,
                tasks: tasks
                    .into_iter()
                    .map(|view| TaskVm {
                        id: view.task.id,
                        name: view.task.name,
                        agent: view.task.agent,
                        session: view.task.session,
                        status: view.task.status,
                        created_at: view.task.created_at,
                        last_message: view.last_message,
                    })
                    .collect(),
            })
            .collect();
    }

    fn toggle_workspace(&mut self, id: WorkspaceId, cx: &mut Context<Self>) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.id == id) {
            group.expanded = !group.expanded;
            cx.notify();
        }
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_mode = mode;
        cx.notify();
    }

    fn select_task(&mut self, id: TaskId, cx: &mut Context<Self>) {
        self.selected = Some(id);
        cx.notify();
    }

    /// Open a task's session in the chat, if it has one.
    fn open_task(&mut self, session: SessionId, cx: &mut Context<Self>) {
        if let Some(chat) = self.chat.clone() {
            chat.update(cx, |chat, cx| chat.view_session(session, cx));
        }
    }

    fn remove_task(&mut self, id: TaskId, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.groups
                .iter_mut()
                .for_each(|g| g.tasks.retain(|t| t.id != id));
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let _ = service.remove_task(&id).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.load(cx);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Pick a folder and add it as a workspace, then refresh.
    fn add_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await else {
                return;
            };
            let _ = service.add_workspace(folder.path().to_path_buf()).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.load(cx);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Open the settings window (when wired to a config service).
    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(config_service) = self.config_service.clone() {
            crate::panels::open_settings_window(config_service, cx);
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load(cx);
    }
}

/// Mock workspaces/tasks for the preview gallery.
fn mock_groups() -> Vec<WorkspaceGroupVm> {
    let now = Utc::now();
    let task = |name: &str, agent: &str, status, mins: i64, msg: Option<&str>| TaskVm {
        id: TaskId::generate(),
        name: name.to_string(),
        agent: AgentId::from(agent),
        session: Some(SessionId::generate()),
        status,
        created_at: now - chrono::Duration::minutes(mins),
        last_message: msg.map(|m| m.to_string()),
    };
    vec![
        WorkspaceGroupVm {
            id: WorkspaceId::generate(),
            name: "agent-studio".to_string(),
            expanded: true,
            tasks: vec![
                task(
                    "Refactor the composer",
                    "claude",
                    SessionStatus::Running,
                    3,
                    Some("Replacing the inline composer with ChatInputBox"),
                ),
                task(
                    "Fix session list ordering",
                    "codex",
                    SessionStatus::Completed,
                    90,
                    Some("Sorted sessions by modification time"),
                ),
            ],
        },
        WorkspaceGroupVm {
            id: WorkspaceId::generate(),
            name: "docs-site".to_string(),
            expanded: false,
            tasks: vec![task(
                "Write the README",
                "gemini",
                SessionStatus::Idle,
                1500,
                None,
            )],
        },
    ]
}
