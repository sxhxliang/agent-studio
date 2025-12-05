mod app;
mod components;
mod core;
mod i18n;
mod panels;
mod schemas;
mod task_data;
mod utils;
pub mod workspace;
pub use app::key_binding;

#[cfg(test)]
mod test_mock_data;


// Re-export from panels module
use crate::panels::{DockPanel, DockPanelContainer, DockPanelState};
pub use panels::{
    CodeEditorPanel, ConversationPanel, SessionManagerPanel, SettingsPanel,
    TaskPanel, WelcomePanel,
};

// Re-export from core module
pub use core::{
    agent::{AgentHandle, AgentManager, PermissionStore},
    config::{AgentProcessConfig, Config, Settings},
    event_bus::{
        PermissionBusContainer, PermissionRequestEvent, SessionUpdateBusContainer,
        SessionUpdateEvent,
    },
};

// Re-export from app module
pub use app::app_state::{AppState, WelcomeSession};
pub use app::{
    actions::{
        About, AddPanel, AddSessionPanel, AddSessionToList, CloseWindow, CreateTaskFromWelcome,
        Info, NewSessionConversationPanel, Open, Quit, SelectFont, SelectLocale, SelectRadius,
        SelectScrollbarShow, SelectedAgentTask, ShowConversationPanel, ShowPanelInfo,
        ShowWelcomePanel, Tab, TabPrev, TestAction, ToggleDockToggleButton, TogglePanelVisible,
        ToggleSearch,
    },
    app_menus, menu, themes, title_bar,
};
use gpui::{
    div, px, size, AnyView, App, AppContext, Bounds, Context, Entity, IntoElement, KeyBinding,
    ParentElement, Pixels, Render, SharedString, Size, Styled, Window, WindowBounds, WindowKind,
    WindowOptions,
};
// Re-export from other modules
pub use menu::UIMenu;
pub use schemas::{conversation_schema, task_schema};
pub use title_bar::AppTitleBar;

// Export components
pub use components::{
    AgentMessage, AgentMessageData, AgentMessageMeta, AgentMessageView, AgentTodoList,
    AgentTodoListView, ChatInputBox, PermissionOptionData, PermissionOptionKind, PermissionRequest,
    PermissionRequestView, PlanMeta, ToolCallItem, ToolCallItemView, ToolCallStatusExt,
    ToolKindExt, UserMessage, UserMessageData, UserMessageView,
};

// Re-export ACP types for convenience
pub use agent_client_protocol_schema::{
    Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus, ToolCall, ToolCallContent, ToolCallId,
    ToolCallStatus, ToolKind,
};

use gpui_component::{
    dock::{register_panel, PanelControl, PanelInfo},
    v_flex, Root, TitleBar,
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
            let (title, description, closable, zoomable, story, on_active, paddings) =
                create_panel_view(&story_state.story_klass, window, cx);

            let mut container = DockPanelContainer::new(cx)
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
            container.paddings = paddings;
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
    Pixels,
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
                $klass::paddings(),
            )
        };
    }

    match story_klass.to_string().as_str() {
        "TaskPanel" => story!(TaskPanel),
        "CodeEditorPanel" => story!(CodeEditorPanel),
        "ConversationPanel" => story!(ConversationPanel),
        "SessionManagerPanel" => story!(SessionManagerPanel),
        "WelcomePanel" => story!(WelcomePanel),
        "SettingsPanel" => story!(SettingsPanel),
        _ => {
            unreachable!("Invalid story klass: {}", story_klass)
        }
    }
}
