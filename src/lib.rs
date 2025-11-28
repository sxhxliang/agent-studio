pub mod acp_client;
pub mod gui_client;
mod app;
mod chat_input;
mod code_editor;
mod components;
mod config;
mod conversation;
mod conversation_acp;
pub mod dock_panel;
mod schemas;
mod session_bus;
mod settings_window;
mod task_data;
mod task_list;
mod task_turn_view;
mod welcome_panel;
pub mod workspace;
pub mod collapsible_story;

use std::sync::Arc;

use crate::{
    acp_client::{AgentManager, PermissionStore},
    dock_panel::{DockPanel, DockPanelContainer, DockPanelState},
    session_bus::SessionUpdateBusContainer,
};
pub use app::{app_menus, menu, themes, title_bar};
pub use chat_input::ChatInputPanel;
pub use code_editor::CodeEditorPanel;
pub use config::{AgentProcessConfig, Config, Settings};
pub use conversation::ConversationPanel;
pub use conversation_acp::ConversationPanelAcp;
use gpui::{
    actions, div, prelude::FluentBuilder as _, px, size, Action, AnyElement, AnyView, App,
    AppContext, Bounds, Context, Entity, Global, IntoElement, KeyBinding, ParentElement, Pixels,
    Render, SharedString, Size, Styled, Window, WindowBounds, WindowKind, WindowOptions,
};
pub use menu::UIMenu;
pub use schemas::{conversation_schema, task_schema};
use serde::Deserialize;
pub use settings_window::SettingsWindow;
pub use task_list::ListTaskPanel;
pub use task_turn_view::CollapsibleEventTurn;
pub use title_bar::AppTitleBar;
pub use welcome_panel::WelcomePanel;

// Export components
pub use components::{
    AgentMessage, AgentMessageData, AgentMessageMeta, AgentMessageView, AgentTodoList,
    AgentTodoListView, PlanMeta, ToolCallItem, ToolCallItemView, ToolCallStatusExt, ToolKindExt,
    UserMessage, UserMessageData, UserMessageView,
};

// Re-export ACP types for convenience
pub use agent_client_protocol_schema::{
    Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus, ToolCall, ToolCallContent, ToolCallId,
    ToolCallStatus, ToolKind,
};

use gpui_component::{
    dock::{register_panel, PanelControl, PanelInfo},
    scroll::ScrollbarShow,
    v_flex, Root, TitleBar,
};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectScrollbarShow(ScrollbarShow);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectLocale(SharedString);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectFont(usize);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectRadius(usize);

actions!(
    story,
    [
        About,
        Open,
        Quit,
        CloseWindow,
        ToggleSearch,
        TestAction,
        Tab,
        TabPrev,
        ShowPanelInfo,
        ShowWelcomePanel,
        ShowConversationPanel
    ]
);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct CreateTaskFromWelcome(pub SharedString);

const PANEL_NAME: &str = "DockPanelContainer";

pub struct AppState {
    pub invisible_panels: Entity<Vec<SharedString>>,
    agent_manager: Option<Arc<AgentManager>>,
    permission_store: Option<Arc<PermissionStore>>,
    pub session_bus: SessionUpdateBusContainer,
}

impl AppState {
    fn init(cx: &mut App) {
        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
            agent_manager: None,
            permission_store: None,
            session_bus: SessionUpdateBusContainer::new(),
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Set the AgentManager after async initialization
    pub fn set_agent_manager(&mut self, manager: Arc<AgentManager>) {
        log::info!("Setting AgentManager with {} agents", manager.list_agents().len());
        self.agent_manager = Some(manager);
    }

    /// Set the PermissionStore
    pub fn set_permission_store(&mut self, store: Arc<PermissionStore>) {
        log::info!("Setting PermissionStore");
        self.permission_store = Some(store);
    }

    /// Get a reference to the AgentManager if initialized
    pub fn agent_manager(&self) -> Option<&Arc<AgentManager>> {
        self.agent_manager.as_ref()
    }

    /// Get the PermissionStore if set
    pub fn permission_store(&self) -> Option<&Arc<PermissionStore>> {
        self.permission_store.as_ref()
    }
}

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
            window_background: gpui::WindowBackgroundAppearance::Transparent,
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

impl Global for AppState {}

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
    code_editor::init();
    menu::init(cx);

    cx.bind_keys([
        KeyBinding::new("/", ToggleSearch, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", Open, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-o", Open, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-f4", Quit, None),
    ]);

    cx.on_action(|_: &Quit, cx: &mut App| {
        cx.quit();
    });

    register_panel(cx, PANEL_NAME, |_, _, info, window, cx| {
        let story_state = match info {
            PanelInfo::Panel(value) => DockPanelState::from_value(value.clone()),
            _ => {
                unreachable!("Invalid PanelInfo: {:?}", info)
            }
        };

        let view = cx.new(|cx| {
            let (title, description, closable, zoomable, story, on_active) =
                create_panel_view(&story_state.story_klass, window, cx);

            let mut container = DockPanelContainer::new(window, cx)
                .story(story, story_state.story_klass)
                .on_active(on_active);

            cx.on_focus_in(
                &container.focus_handle,
                window,
                |this: &mut DockPanelContainer, _, _| {
                    println!("DockPanelContainer focus in: {}", this.name);
                },
            )
            .detach();

            container.name = title.into();
            container.description = description.into();
            container.closable = closable;
            container.zoomable = zoomable;
            container
        });
        Box::new(view)
    });

    cx.activate(true);
}

fn create_panel_view(
    story_klass: &SharedString,
    window: &mut Window,
    cx: &mut App,
) -> (
    &'static str,
    &'static str,
    bool,
    Option<PanelControl>,
    AnyView,
    fn(AnyView, bool, &mut Window, &mut App),
) {
    macro_rules! story {
        ($klass:tt) => {
            (
                $klass::title(),
                $klass::description(),
                $klass::closable(),
                $klass::zoomable(),
                $klass::view(window, cx).into(),
                $klass::on_active_any,
            )
        };
    }

    match story_klass.to_string().as_str() {
        "ListTaskPanel" => story!(ListTaskPanel),
        "CodeEditorPanel" => story!(CodeEditorPanel),
        "ConversationPanel" => story!(ConversationPanel),
        "ConversationPanelAcp" => story!(ConversationPanelAcp),
        "ChatInputPanel" => story!(ChatInputPanel),
        "WelcomePanel" => story!(WelcomePanel),
        _ => {
            unreachable!("Invalid story klass: {}", story_klass)
        }
    }
}
