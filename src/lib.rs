mod app;
mod assets;
mod components;
pub mod core;
mod i18n;
mod panels;
mod reqwest_client;
mod schemas;
mod utils;
pub mod workspace;

rust_i18n::i18n!("locales", fallback = "en");

pub use app::key_binding;
pub use assets::Assets;
pub use assets::get_default_config;

#[cfg(test)]
mod test_mock_data;

// Re-export from panels module
use crate::panels::{DockPanelContainer, DockPanelState};
pub use panels::{
    AppSettings, CodeEditorPanel, ConversationPanel, SessionManagerPanel, SettingsPanel, TaskPanel,
    TerminalPanel, ToolCallDetailPanel, WelcomePanel,
};

// Re-export from core module
pub use core::{
    agent::{AgentHandle, AgentManager, PermissionStore},
    config::{AgentProcessConfig, Config},
    event_bus::{
        PermissionBusContainer, PermissionRequestEvent, SessionUpdateBusContainer,
        SessionUpdateEvent,
    },
};

// Re-export from app module
pub use app::app_state::{AppState, WelcomeSession};
pub use app::{
    actions::{
        About, AddAgent, AddSessionToList, CancelSession, CloseWindow, CreateTaskFromWelcome, Info,
        NewSessionConversationPanel, Open, PanelAction, Quit, ReloadAgentConfig, RemoveAgent,
        RestartAgent, SelectFont, SelectLocale, SelectRadius, SelectScrollbarShow,
        SelectedAgentTask, SendMessageToSession, SetUploadDir, ShowPanelInfo, Tab, TabPrev,
        TestAction, ToggleDockToggleButton, TogglePanelVisible, ToggleSearch, UpdateAgent,
    },
    app_menus, menu, system_tray, themes, title_bar,
};
use gpui::{
    AnyView, App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Pixels, Render,
    SharedString, Size, Styled, Window, WindowBounds, WindowKind, WindowOptions, div, px, size,
};
// Re-export from other modules
pub use menu::UIMenu;
pub use schemas::{conversation_schema, task_schema};
pub use title_bar::AppTitleBar;

// Export components
pub use components::{
    AgentMessage, AgentMessageData, AgentMessageMeta, AgentMessageView, AgentTodoList,
    AgentTodoListView, ChatInputBox, DiffSummary, DiffSummaryData, FileChangeStats,
    PermissionRequest, PermissionRequestView, PlanMeta, StatusIndicator, ToolCallItem,
    ToolCallItemView, UserMessage, UserMessageData, UserMessageView,
};

// Re-export ACP types for convenience
pub use agent_client_protocol::{
    Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus, ToolCall, ToolCallContent, ToolCallId,
    ToolCallStatus, ToolKind,
};

use gpui_component::{
    Root, TitleBar,
    dock::{PanelInfo, register_panel},
    v_flex,
};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

const PANEL_NAME: &str = "DockPanelContainer";

pub fn create_new_window<F, E>(title: &str, crate_view_fn: F, cx: &mut App)
where
    E: Into<AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    create_new_window_with_size(title, None, crate_view_fn, cx);
}

pub fn create_new_window_with_size<F, E>(
    title: &str,
    window_size: Option<Size<Pixels>>,
    crate_view_fn: F,
    cx: &mut App,
) where
    E: Into<AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    let mut window_size = window_size.unwrap_or(size(px(1600.0), px(1200.0)));
    if let Some(display) = cx.primary_display() {
        let display_size = display.bounds().size;
        window_size.width = window_size.width.min(display_size.width * 0.85);
        window_size.height = window_size.height.min(display_size.height * 0.85);
    }
    let window_bounds = Bounds::centered(None, window_size, cx);
    let title = SharedString::from(title.to_string());

    cx.spawn(async move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            window_min_size: Some(gpui::Size {
                width: px(480.),
                height: px(320.),
            }),
            kind: WindowKind::Normal,
            #[cfg(target_os = "linux")]
            window_background: gpui::WindowBackgroundAppearance::Opaque,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
        };

        let window = cx
            .open_window(options, |window, cx| {
                let view = crate_view_fn(window, cx);
                let root = cx.new(|cx| DockRoot::new(title.clone(), view, window, cx));

                cx.new(|cx| Root::new(root, window, cx))
            })
            .expect("failed to open window");

        window
            .update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title(&title);
            })
            .expect("failed to update window");

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

struct DockRoot {
    title_bar: Entity<AppTitleBar>,
    view: AnyView,
}

impl DockRoot {
    pub fn new(
        title: impl Into<SharedString>,
        view: impl Into<AnyView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_bar = cx.new(|cx| AppTitleBar::new(title, window, cx));
        Self {
            title_bar,
            view: view.into(),
        }
    }
}

impl Render for DockRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .child(
                v_flex()
                    .size_full()
                    .child(self.title_bar.clone())
                    .child(div().flex_1().overflow_hidden().child(self.view.clone())),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub fn init(cx: &mut App) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("gpui_component=trace".parse().unwrap()),
        )
        .init();

    gpui_component::init(cx);
    AppState::init(cx);
    themes::init(cx);
    i18n::init(cx);
    panels::code_editor::init();
    menu::init(cx);
    key_binding::init(cx);

    let http_client =
        std::sync::Arc::new(reqwest_client::ReqwestClient::user_agent("agentx-studio").unwrap());
    cx.set_http_client(http_client);

    cx.on_action(|_: &Quit, cx: &mut App| {
        cx.quit();
    });

    // Register agent config action handlers
    cx.on_action(workspace::actions::add_agent);
    cx.on_action(workspace::actions::update_agent);
    cx.on_action(workspace::actions::remove_agent);
    cx.on_action(workspace::actions::restart_agent);
    cx.on_action(workspace::actions::reload_agent_config);
    cx.on_action(workspace::actions::set_upload_dir);
    cx.on_action(workspace::actions::change_config_path);

    register_panel(cx, PANEL_NAME, |_, _, info, window, cx| {
        let agent_state = match info {
            PanelInfo::Panel(value) => DockPanelState::from_value(value.clone()),
            _ => {
                unreachable!("Invalid PanelInfo: {:?}", info)
            }
        };

        Box::new(DockPanelContainer::panel_from_state(
            &agent_state,
            window,
            cx,
        ))
    });

    cx.activate(true);
}
