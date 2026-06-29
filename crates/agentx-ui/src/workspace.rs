//! The dock shell: hosts welcome, conversation, and side panels in a DockArea.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::*;
use gpui_component::{
    ActiveTheme as _, Root,
    dock::{DockArea, DockItem, PanelView},
};

use agentx_app::{ConfigService, FileService, SessionService, WorkspaceService};
use agentx_bus::EventBus;
use agentx_domain::{AgentRegistry, Config, DomainEvent};

use crate::chat::{ChatView, SessionsPanel};
use crate::panels::{SessionManagerPanel, TaskPanel, WelcomeLaunch, WelcomePanel};

/// The window's root content: a dock area whose center starts as the migrated
/// welcome panel, then switches to the conversation panel after a task starts.
pub struct Workspace {
    dock_area: Entity<DockArea>,
}

impl Workspace {
    #[allow(clippy::too_many_arguments)] // Composition root wires the adapter ports here.
    pub fn new(
        registry: Arc<dyn AgentRegistry>,
        service: Arc<SessionService>,
        workspace_service: Arc<WorkspaceService>,
        config_service: Arc<ConfigService>,
        file_service: Arc<FileService>,
        bus: EventBus,
        config: Config,
        cwd: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let dock_area = cx.new(|cx| DockArea::new("agentx-main", Some(1), window, cx));
        let weak_dock = dock_area.downgrade();

        let launch_handler = {
            let weak_dock = weak_dock.clone();
            let registry = registry.clone();
            let service = service.clone();
            let workspace_service = workspace_service.clone();
            let config_service = config_service.clone();
            let file_service = file_service.clone();
            Arc::new(
                move |launch: WelcomeLaunch, window: &mut Window, cx: &mut App| {
                    if let Some(dock_area) = weak_dock.upgrade() {
                        show_chat_layout(
                            &dock_area,
                            service.clone(),
                            registry.clone(),
                            workspace_service.clone(),
                            config_service.clone(),
                            file_service.clone(),
                            launch,
                            window,
                            cx,
                        );
                    }
                },
            )
        };

        let welcome = cx.new(|cx| {
            WelcomePanel::new(
                registry,
                service.clone(),
                workspace_service.clone(),
                config_service.clone(),
                file_service,
                bus,
                config,
                cwd,
                launch_handler,
                window,
                cx,
            )
        });
        let tasks =
            cx.new(|cx| TaskPanel::new(workspace_service, config_service, None, window, cx));

        let center = DockItem::tab(welcome, &weak_dock, window, cx);
        let left = DockItem::tabs(
            vec![Arc::new(tasks) as Arc<dyn PanelView>],
            &weak_dock,
            window,
            cx,
        );
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
            area.set_left_dock(left, Some(px(280.)), true, window, cx);
        });
        Self { dock_area }
    }

    pub(crate) fn with_chat(
        chat: Entity<ChatView>,
        sessions: Entity<SessionsPanel>,
        manager: Entity<SessionManagerPanel>,
        tasks: Entity<TaskPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let dock_area = cx.new(|cx| DockArea::new("agentx-main", Some(1), window, cx));
        let weak = dock_area.downgrade();
        let center = DockItem::tab(chat, &weak, window, cx);
        let left = DockItem::tabs(
            vec![
                Arc::new(tasks) as Arc<dyn PanelView>,
                Arc::new(sessions) as Arc<dyn PanelView>,
                Arc::new(manager) as Arc<dyn PanelView>,
            ],
            &weak,
            window,
            cx,
        );
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
            area.set_left_dock(left, Some(px(280.)), true, window, cx);
        });
        Self { dock_area }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.dock_area.clone()
    }
}

fn show_chat_layout(
    dock_area: &Entity<DockArea>,
    service: Arc<SessionService>,
    registry: Arc<dyn AgentRegistry>,
    workspace_service: Arc<WorkspaceService>,
    config_service: Arc<ConfigService>,
    file_service: Arc<FileService>,
    launch: WelcomeLaunch,
    window: &mut Window,
    cx: &mut App,
) {
    let WelcomeLaunch {
        agent,
        cwd,
        init,
        events,
        initial_content,
    } = launch;
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

    let weak = dock_area.downgrade();
    let center = DockItem::tab(chat, &weak, window, cx);
    let left = DockItem::tabs(
        vec![
            Arc::new(tasks) as Arc<dyn PanelView>,
            Arc::new(sessions) as Arc<dyn PanelView>,
            Arc::new(manager) as Arc<dyn PanelView>,
        ],
        &weak,
        window,
        cx,
    );
    dock_area.update(cx, |area, cx| {
        area.set_center(center, window, cx);
        area.set_left_dock(left, Some(px(280.)), true, window, cx);
    });
}

#[allow(clippy::too_many_arguments)] // Public composition entry mirrors the app ports.
pub fn open_workspace_window(
    registry: Arc<dyn AgentRegistry>,
    service: Arc<SessionService>,
    workspace_service: Arc<WorkspaceService>,
    config_service: Arc<ConfigService>,
    file_service: Arc<FileService>,
    bus: EventBus,
    config: Config,
    cwd: PathBuf,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        let workspace = cx.new(|cx| {
            Workspace::new(
                registry,
                service,
                workspace_service,
                config_service,
                file_service,
                bus,
                config,
                cwd,
                window,
                cx,
            )
        });
        cx.new(|cx| Root::new(workspace, window, cx).bg(cx.theme().background))
    });
}
