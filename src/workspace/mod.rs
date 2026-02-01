use anyhow::{Context as _, Result};
use gpui::*;
use gpui_component::Root;
use gpui_component::dock::{DockArea, DockAreaState, DockEvent, DockItem, DockPlacement};
use smol::Timer;
use std::{sync::Arc, time::Duration};

use crate::{
    AppSettings, AppTitleBar, CodeEditorPanel, ConversationPanel, SessionManagerPanel, TaskPanel,
    TerminalPanel,
    core::updater::{UpdateCheckResult, UpdateManager},
    panels::dock_panel::DockPanelContainer,
};

use self::startup::StartupState;

// Action handlers module
pub mod actions;
mod startup;

const MAIN_DOCK_AREA: DockAreaTab = DockAreaTab {
    id: "main-dock",
    version: 5,
};

pub struct DockWorkspace {
    title_bar: Entity<AppTitleBar>,
    dock_area: Entity<DockArea>,
    last_layout_state: Option<DockAreaState>,
    toggle_button_visible: bool,
    _save_layout_task: Option<Task<()>>,
    startup_state: StartupState,
    startup_completed: bool,
    update_checked_on_startup: bool,
}

struct DockAreaTab {
    id: &'static str,
    version: usize,
}

impl DockWorkspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock_area =
            cx.new(|cx| DockArea::new(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx));
        let weak_dock_area = dock_area.downgrade();

        match Self::load_layout(dock_area.clone(), window, cx) {
            Ok(_) => {
                println!("load layout success");
            }
            Err(err) => {
                eprintln!("load layout error: {:?}", err);
                Self::reset_default_layout(weak_dock_area, window, cx);
            }
        };

        cx.subscribe_in(
            &dock_area,
            window,
            |this, dock_area, ev: &DockEvent, window, cx| match ev {
                DockEvent::LayoutChanged => this.save_layout(dock_area, window, cx),
                _ => {}
            },
        )
        .detach();

        cx.on_app_quit({
            let dock_area = dock_area.clone();
            move |_, cx| {
                let state = dock_area.read(cx).dump(cx);
                cx.background_executor().spawn(async move {
                    // Save layout before quitting
                    if let Err(e) = Self::save_state(&state) {
                        log::warn!("Failed to save layout state: {}", e);
                    }
                })
            }
        })
        .detach();

        cx.on_release(|this, cx| {
            this.flush_layout_state(cx);
            crate::themes::save_state(cx);
        })
        .detach();

        let title_bar = cx.new(|cx| {
            AppTitleBar::new("Agent Studio", window, cx)
            // .child({
            //     move |_, cx| {
            //         Button::new("add-panel")
            //             .icon(IconName::LayoutDashboard)
            //             .small()
            //             .ghost()
            //             .dropdown_menu({
            //                 let invisible_panels = AppState::global(cx).invisible_panels.clone();

            //                 move |menu, _, cx| {
            //                     menu.menu(
            //                         "Add Panel to Center",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Center)),
            //                     )
            //                     .separator()
            //                     .menu(
            //                         "Add Panel to Left",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Left)),
            //                     )
            //                     .menu(
            //                         "Add Panel to Right",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Right)),
            //                     )
            //                     .menu(
            //                         "Add Panel to Bottom",
            //                         Box::new(PanelAction::add_conversation(DockPlacement::Bottom)),
            //                     )
            //                     .separator()
            //                     .menu(
            //                         "Show / Hide Dock Toggle Button",
            //                         Box::new(ToggleDockToggleButton),
            //                     )
            //                     .separator()
            //                     .menu_with_check(
            //                         "Sidebar",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("Sidebar")),
            //                         Box::new(TogglePanelVisible(SharedString::from("Sidebar"))),
            //                     )
            //                     .menu_with_check(
            //                         "Dialog",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("Dialog")),
            //                         Box::new(TogglePanelVisible(SharedString::from("Dialog"))),
            //                     )
            //                     .menu_with_check(
            //                         "Accordion",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("Accordion")),
            //                         Box::new(TogglePanelVisible(SharedString::from("Accordion"))),
            //                     )
            //                     .menu_with_check(
            //                         "List",
            //                         !invisible_panels
            //                             .read(cx)
            //                             .contains(&SharedString::from("List")),
            //                         Box::new(TogglePanelVisible(SharedString::from("List"))),
            //                     )
            //                 }
            //             })
            //             .anchor(Corner::TopRight)
            //     }
            // })
        });

        Self {
            dock_area,
            title_bar,
            last_layout_state: None,
            toggle_button_visible: true,
            _save_layout_task: None,
            startup_state: StartupState::new(),
            startup_completed: crate::themes::startup_completed(),
            update_checked_on_startup: false,
        }
    }

    /// Check for updates on startup if auto-check is enabled
    fn maybe_check_updates_on_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_checked_on_startup {
            return;
        }
        self.update_checked_on_startup = true;

        if !AppSettings::global(cx).auto_check_on_startup {
            return;
        }

        log::info!("Auto-checking for updates on startup...");
        let update_manager = UpdateManager::default();

        cx.spawn_in(window, async move |_this, _window| {
            match update_manager.check_for_updates().await {
                UpdateCheckResult::UpdateAvailable(info) => {
                    log::info!("Update available: {}", info.version);
                }
                UpdateCheckResult::NoUpdate => {
                    log::info!("No updates available");
                }
                UpdateCheckResult::Error(err) => {
                    log::warn!("Failed to check for updates: {}", err);
                }
            }
        })
        .detach();
    }

    fn save_layout(
        &mut self,
        dock_area: &Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dock_area = dock_area.clone();
        self._save_layout_task = Some(cx.spawn_in(window, async move |agent_studio, window| {
            Timer::after(Duration::from_secs(10)).await;

            _ = agent_studio.update_in(window, move |this, _, cx| {
                let dock_area = dock_area.read(cx);
                let state = dock_area.dump(cx);

                let last_layout_state = this.last_layout_state.clone();
                if Some(&state) == last_layout_state.as_ref() {
                    return;
                }

                if let Err(e) = Self::save_state(&state) {
                    log::warn!("Failed to save layout state: {}", e);
                }
                this.last_layout_state = Some(state);
            });
        }));
    }

    fn flush_layout_state(&mut self, cx: &mut App) {
        let state = self.dock_area.read(cx).dump(cx);
        if Some(&state) == self.last_layout_state.as_ref() {
            return;
        }
        if let Err(e) = Self::save_state(&state) {
            log::warn!("Failed to save layout state: {}", e);
        }
        self.last_layout_state = Some(state);
    }

    fn save_state(state: &DockAreaState) -> Result<()> {
        println!("Save Docks layout...");
        let json = serde_json::to_string_pretty(state)?;
        let state_file = crate::core::config_manager::get_docks_layout_path();
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(state_file, json)?;
        Ok(())
    }

    fn load_layout(
        dock_area: Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        println!("Load Docks layout...");
        let state_file = crate::core::config_manager::get_docks_layout_path();
        let json = std::fs::read_to_string(state_file)?;
        let state = serde_json::from_str::<DockAreaState>(&json)?;

        // Check if the saved layout version is different from the current version
        // Notify the user and ask if they want to reset the layout to default.
        if state.version != Some(MAIN_DOCK_AREA.version) {
            let answer = window.prompt(
                PromptLevel::Info,
                "The default main layout has been updated.\n\nDo you want to reset the layout to default?",
                None,
                &["Yes", "No"],
                cx,
            );

            let weak_dock_area = dock_area.downgrade();
            cx.spawn_in(window, async move |this, window| {
                if answer.await == Ok(0) {
                    _ = this.update_in(window, |_, window, cx| {
                        Self::reset_default_layout(weak_dock_area, window, cx);
                    });
                }
            })
            .detach();
        }

        dock_area.update(cx, |dock_area, cx| {
            dock_area.load(state, window, cx).context("load layout")?;
            dock_area.set_dock_collapsible(
                Edges {
                    left: true,
                    bottom: true,
                    right: true,
                    ..Default::default()
                },
                window,
                cx,
            );

            Ok::<(), anyhow::Error>(())
        })
    }

    fn reset_default_layout(dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut App) {
        let dock_item = Self::init_default_layout(&dock_area, window, cx);

        let left_panels = DockItem::split_with_sizes(
            Axis::Vertical,
            vec![DockItem::tab(
                DockPanelContainer::panel::<TaskPanel>(window, cx),
                &dock_area,
                window,
                cx,
            )],
            vec![None, Some(px(360.))],
            &dock_area,
            window,
            cx,
        );

        let bottom_panels = DockItem::split_with_sizes(
            Axis::Vertical,
            vec![DockItem::tabs(
                vec![
                    Arc::new(DockPanelContainer::panel::<TerminalPanel>(window, cx)),
                    Arc::new(DockPanelContainer::panel::<SessionManagerPanel>(window, cx)),
                ],
                &dock_area,
                window,
                cx,
            )],
            vec![None],
            &dock_area,
            window,
            cx,
        );

        let right_panels = DockItem::split_with_sizes(
            Axis::Vertical,
            vec![DockItem::tabs(
                vec![
                    Arc::new(DockPanelContainer::panel::<CodeEditorPanel>(window, cx)),
                    // Arc::new(DockPanelContainer::panel::<TerminalPanel>(window, cx)),
                ],
                &dock_area,
                window,
                cx,
            )],
            vec![None],
            &dock_area,
            window,
            cx,
        );

        _ = dock_area.update(cx, |view, cx| {
            view.set_version(MAIN_DOCK_AREA.version, window, cx);
            view.set_center(dock_item, window, cx);
            view.set_left_dock(left_panels, Some(px(350.)), true, window, cx);
            view.set_bottom_dock(bottom_panels, Some(px(200.)), true, window, cx);
            view.set_right_dock(right_panels, Some(px(480.)), true, window, cx);

            if let Err(e) = Self::save_state(&view.dump(cx)) {
                log::warn!("Failed to save layout state: {}", e);
            }
        });
    }

    fn init_default_layout(
        dock_area: &WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> DockItem {
        // Main layout: Left (CodeEditorPanel) and Right (Conversation + Input)
        DockItem::split_with_sizes(
            Axis::Horizontal,
            vec![
                // Left panel: ConversationPanel (ACP-enabled conversation)
                DockItem::tabs(
                    vec![Arc::new(DockPanelContainer::panel::<ConversationPanel>(
                        window, cx,
                    ))],
                    &dock_area,
                    window,
                    cx,
                ),
                // Right panel: Combined conversation and input
                // right_side,
            ],
            vec![None, None],
            &dock_area,
            window,
            cx,
        )
    }

    pub fn new_local(cx: &mut App) -> Task<anyhow::Result<WindowHandle<Root>>> {
        let mut window_size = size(px(1600.0), px(1200.0));
        if let Some(display) = cx.primary_display() {
            let display_size = display.bounds().size;
            window_size.width = window_size.width.min(display_size.width * 0.85);
            window_size.height = window_size.height.min(display_size.height * 0.85);
        }

        let window_bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                #[cfg(not(target_os = "linux"))]
                titlebar: Some(gpui_component::TitleBar::title_bar_options()),
                window_min_size: Some(gpui::Size {
                    width: px(640.0),
                    height: px(480.0),
                }),
                #[cfg(target_os = "linux")]
                window_background: gpui::WindowBackgroundAppearance::Opaque,
                #[cfg(target_os = "linux")]
                window_decorations: Some(gpui::WindowDecorations::Client),
                kind: WindowKind::Normal,
                ..Default::default()
            };

            let window = cx.open_window(options, |window, cx| {
                let agent_studio_view = cx.new(|cx| DockWorkspace::new(window, cx));
                cx.new(|cx| Root::new(agent_studio_view, window, cx))
            })?;

            window
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    window.set_window_title("Agent Studio");
                    cx.on_release(|_, cx| {
                        // exit app
                        cx.quit();
                    })
                    .detach();
                })
                .expect("failed to update window");

            Ok(window)
        })
    }
}

impl Render for DockWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_startup_initialized(window, cx);

        if self.startup_state.is_complete() && !self.startup_completed {
            crate::themes::set_startup_completed(true);
            self.startup_completed = true;
        }

        // Check for updates on startup (after startup wizard is complete)
        if self.startup_completed {
            self.maybe_check_updates_on_startup(window, cx);
        }

        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        let content = if self.startup_completed || self.startup_state.is_complete() {
            self.dock_area.clone().into_any_element()
        } else {
            self.render_startup(cx)
        };

        div()
            .id("agent_studio-workspace")
            .on_action(cx.listener(Self::on_action_panel_action))
            .on_action(cx.listener(Self::on_action_toggle_panel_visible))
            .on_action(cx.listener(Self::on_action_toggle_dock_toggle_button))
            .on_action(cx.listener(Self::on_action_open_setting_panel))
            .on_action(cx.listener(Self::on_action_open_session_manager))
            .on_action(cx.listener(Self::on_action_new_session_conversation_panel))
            .on_action(cx.listener(Self::on_action_create_task_from_welcome))
            .on_action(cx.listener(Self::on_action_send_message_to_session))
            .on_action(cx.listener(Self::on_action_cancel_session))
            .on_action(cx.listener(Self::on_action_open))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(self.title_bar.clone())
            .child(content)
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub fn open_new(
    cx: &mut App,
    init: impl FnOnce(&mut Root, &mut Window, &mut Context<Root>) + 'static + Send,
) -> Task<()> {
    let task: Task<std::result::Result<WindowHandle<Root>, anyhow::Error>> =
        DockWorkspace::new_local(cx);
    cx.spawn(async move |cx| {
        if let Some(root) = task.await.ok() {
            root.update(cx, |workspace, window, cx| init(workspace, window, cx))
                .expect("failed to init workspace");
        }
    })
}
