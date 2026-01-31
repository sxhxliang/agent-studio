use agent_client_protocol as acp;
use gpui::*;
use gpui_component::{
    WindowExt, dock::{
        DockItem, DockPlacement, Panel, PanelInfo, PanelState, PanelView, TabPanel,
    }, notification::Notification
};
use std::sync::Arc;

use crate::{
    AppState, ConversationPanel, CreateTaskFromWelcome, NewSessionConversationPanel,
    OpenSessionManager, PanelAction, SendMessageToSession, SessionManagerPanel, SettingsPanel,
    ToggleDockToggleButton, TogglePanelVisible, WelcomePanel,
    app::actions::{
        AddAgent, CancelSession, ChangeConfigPath, PanelCommand, PanelKind, ReloadAgentConfig,
        RemoveAgent, RestartAgent, SetUploadDir, Submit, UpdateAgent,
    },
    panels::{
        DockPanel,
        dock_panel::{DockPanelContainer, DockPanelState},
    },
    title_bar::OpenSettings,
    utils,
};

use super::DockWorkspace;
//   - on_action_panel_action - 添加/展示面板
//   - on_action_toggle_panel_visible - 切换面板可见性
//   - on_action_toggle_dock_toggle_button - 切换 dock 按钮显示
//   - on_action_open - 打开文件夹选择器
//   - on_action_create_task_from_welcome - 从欢迎面板创建任务

impl DockWorkspace {
    pub(super) fn submit(&mut self, _: &Submit, _: &mut Window, _cx: &mut Context<Self>) {
        // println!("Submitted URL: {}", self.content);
        // cx.emit(UrlInputEvent::SubmitRequested);
    }
    /// Helper method to create and add a new ConversationPanel to the center
    pub fn add_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx));

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, DockPlacement::Center, None, window, cx);
        });
    }

    /// Helper method to show ConversationPanel in the active center tab
    ///
    /// If session_id is provided, it will load the conversation history for that session.
    /// Otherwise, it will create a new conversation panel with mock data.
    fn show_conversation_panel(
        &mut self,
        session_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(session_id) = session_id.as_deref().filter(|id| !id.is_empty()) {
            if self.activate_existing_session_panel(session_id, window, cx) {
                return;
            }

            let session_id = session_id.to_string();

            Self::resume_session_if_needed(&session_id, cx);

            self.dock_area.update(cx, |dock_area, cx| {
                let conversation_panel =
                    DockPanelContainer::panel_for_session(session_id.clone(), window, cx);
                dock_area.add_panel(
                    Arc::new(conversation_panel),
                    DockPlacement::Center,
                    None,
                    window,
                    cx,
                );
            });
            return;
        }

        self.dock_area.update(cx, |dock_area, cx| {
            let selection = Self::find_focused_tab_panel(dock_area.center(), window, cx)
                .or_else(|| Self::find_first_tab_panel(dock_area.center(), cx));

            if let Some((_, active_panel)) = selection {
                if let Ok(container) = active_panel.view().downcast::<DockPanelContainer>() {
                    container.update(cx, |container, cx| {
                        container.replace_with_conversation_session(session_id.clone(), window, cx);
                    });
                    return;
                }
            }

            let conversation_panel = if let Some(session_id) = session_id.clone() {
                DockPanelContainer::panel_for_session(session_id, window, cx)
            } else {
                DockPanelContainer::panel::<ConversationPanel>(window, cx)
            };
            dock_area.add_panel(
                Arc::new(conversation_panel),
                DockPlacement::Center,
                None,
                window,
                cx,
            );
        });
    }

    fn resume_session_if_needed(session_id: &str, cx: &mut Context<Self>) {
        let agent_service = AppState::global(cx).agent_service().cloned();
        if let Some(agent_service) = agent_service {
            let session_id_clone = session_id.to_string();
            cx.spawn(async move |_this, _cx| {
                if let Some(agent_name) = agent_service.get_agent_for_session(&session_id_clone) {
                    log::info!(
                        "Resuming session {} for agent {}",
                        session_id_clone,
                        agent_name
                    );
                    match agent_service
                        .resume_session(&agent_name, &session_id_clone)
                        .await
                    {
                        Ok(_) => {
                            log::info!("Successfully resumed session {}", session_id_clone);
                        }
                        Err(e) => {
                            log::warn!("Failed to resume session {}: {}", session_id_clone, e);
                        }
                    }
                } else {
                    log::warn!(
                        "No agent found for session {}, skipping resume",
                        session_id_clone
                    );
                }
            })
            .detach();
        }
    }

    fn find_focused_tab_panel(
        item: &DockItem,
        window: &Window,
        cx: &App,
    ) -> Option<(Entity<TabPanel>, Arc<dyn PanelView>)> {
        match item {
            DockItem::Tabs { view, .. } => {
                let active_panel = view.read(cx).active_panel(cx)?;
                if active_panel.focus_handle(cx).contains_focused(window, cx) {
                    Some((view.clone(), active_panel))
                } else {
                    None
                }
            }
            DockItem::Split { items, .. } => items
                .iter()
                .find_map(|item| Self::find_focused_tab_panel(item, window, cx)),
            DockItem::Tiles { .. } | DockItem::Panel { .. } => None,
        }
    }

    fn find_first_tab_panel(
        item: &DockItem,
        cx: &App,
    ) -> Option<(Entity<TabPanel>, Arc<dyn PanelView>)> {
        match item {
            DockItem::Tabs { view, .. } => view
                .read(cx)
                .active_panel(cx)
                .map(|active_panel| (view.clone(), active_panel)),
            DockItem::Split { items, .. } => items
                .iter()
                .find_map(|item| Self::find_first_tab_panel(item, cx)),
            DockItem::Tiles { .. } | DockItem::Panel { .. } => None,
        }
    }

    fn activate_existing_session_panel(
        &mut self,
        session_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        log::debug!(
            "Searching existing session panel: session_id={}",
            session_id
        );
        let items = self.dock_area.read(cx).center().clone();
        let found = Self::activate_session_in_item(&items, session_id, window, cx);
        if found {
            log::debug!(
                "Activated existing session panel: session_id={}",
                session_id
            );
        } else {
            log::debug!("No existing session panel found: session_id={}", session_id);
        }
        found
    }

    fn activate_session_in_item(
        item: &DockItem,
        session_id: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        match item {
            DockItem::Tabs { view, .. } => {
                let tab_state = view.read(cx).dump(cx);
                let active_ix = tab_state.info.active_index().unwrap_or(0);

                for (ix, child_state) in tab_state.children.iter().enumerate() {
                    if !Self::panel_state_contains_session(child_state, session_id) {
                        continue;
                    }

                    log::debug!(
                        "Found session panel in tabs: session_id={} active_ix={} target_ix={}",
                        session_id,
                        active_ix,
                        ix
                    );

                    if ix != active_ix {
                        let _ = item.clone().active_index(ix, cx);
                    }

                    if let Some(active_panel) = view.read(cx).active_panel(cx) {
                        active_panel.focus_handle(cx).focus(window, cx);
                    }
                    let _ = view.update(cx, |_, cx| {
                        cx.notify();
                    });
                    return true;
                }

                false
            }
            DockItem::Split { items, .. } => items
                .iter()
                .any(|item| Self::activate_session_in_item(item, session_id, window, cx)),
            DockItem::Panel { view, .. } => {
                if Self::panel_matches_session(view, session_id, cx) {
                    view.set_active(true, window, cx);
                    view.focus_handle(cx).focus(window, cx);
                    return true;
                }
                false
            }
            DockItem::Tiles { .. } => false,
        }
    }

    fn panel_state_contains_session(panel_state: &PanelState, session_id: &str) -> bool {
        match &panel_state.info {
            PanelInfo::Panel(value) => {
                let dock_state = DockPanelState::from_value(value.clone());
                if dock_state.agent_studio_klass.as_ref() == "ConversationPanel"
                    && dock_state.session_id.as_deref() == Some(session_id)
                {
                    return true;
                }
            }
            _ => {}
        }

        panel_state
            .children
            .iter()
            .any(|child| Self::panel_state_contains_session(child, session_id))
    }

    fn panel_matches_session(panel: &Arc<dyn PanelView>, session_id: &str, cx: &App) -> bool {
        let panel_id = panel.panel_id(cx);
        let Ok(container) = panel.view().downcast::<DockPanelContainer>() else {
            log::debug!(
                "Panel is not DockPanelContainer: session_id={} panel_id={:?}",
                session_id,
                panel_id
            );
            return false;
        };

        let container = container.read(cx);
        let Some(agent_studio_klass) = container.agent_studio_klass.as_ref() else {
            log::debug!(
                "Panel has no agent_studio klass: session_id={} panel_id={:?}",
                session_id,
                panel_id
            );
            return false;
        };

        if agent_studio_klass.as_ref() != "ConversationPanel" {
            log::debug!(
                "Panel agent_studio klass mismatch: session_id={} panel_id={:?} agent_studio_klass={}",
                session_id,
                panel_id,
                agent_studio_klass.as_ref()
            );
            return false;
        }

        let Some(agent_studio) = container.agent_studio.clone() else {
            log::debug!(
                "Conversation panel missing agent_studio: session_id={} panel_id={:?}",
                session_id,
                panel_id
            );
            return false;
        };

        let Ok(conversation) = agent_studio.downcast::<ConversationPanel>() else {
            log::debug!(
                "Conversation panel downcast failed: session_id={} panel_id={:?}",
                session_id,
                panel_id
            );
            return false;
        };

        let panel_session_id = conversation.read(cx).session_id();
        if panel_session_id.as_deref() == Some(session_id) {
            return true;
        }

        log::debug!(
            "Conversation panel session mismatch: session_id={} panel_id={:?} panel_session_id={:?}",
            session_id,
            panel_id,
            panel_session_id
        );
        false
    }

    fn activate_existing_session_manager_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(bottom_dock) = self.dock_area.read(cx).bottom_dock().cloned() else {
            return false;
        };
        let panel = bottom_dock.read(cx).panel().clone();
        Self::activate_panel_by_klass(&panel, SessionManagerPanel::klass(), window, cx)
    }

    fn activate_panel_by_klass(
        item: &DockItem,
        klass: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        match item {
            DockItem::Tabs { view, .. } => {
                let tab_state = view.read(cx).dump(cx);
                let active_ix = tab_state.info.active_index().unwrap_or(0);

                for (ix, child_state) in tab_state.children.iter().enumerate() {
                    if !Self::panel_state_matches_klass(child_state, klass) {
                        continue;
                    }

                    if ix != active_ix {
                        let _ = item.clone().active_index(ix, cx);
                    }

                    if let Some(active_panel) = view.read(cx).active_panel(cx) {
                        active_panel.focus_handle(cx).focus(window, cx);
                    }
                    let _ = view.update(cx, |_, cx| {
                        cx.notify();
                    });
                    return true;
                }

                false
            }
            DockItem::Split { items, .. } => items
                .iter()
                .any(|item| Self::activate_panel_by_klass(item, klass, window, cx)),
            DockItem::Panel { view, .. } => {
                if Self::panel_matches_klass(view, klass, cx) {
                    view.set_active(true, window, cx);
                    view.focus_handle(cx).focus(window, cx);
                    return true;
                }
                false
            }
            DockItem::Tiles { .. } => false,
        }
    }

    fn panel_state_matches_klass(panel_state: &PanelState, klass: &str) -> bool {
        match &panel_state.info {
            PanelInfo::Panel(value) => {
                let dock_state = DockPanelState::from_value(value.clone());
                if dock_state.agent_studio_klass.as_ref() == klass {
                    return true;
                }
            }
            _ => {}
        }

        panel_state
            .children
            .iter()
            .any(|child| Self::panel_state_matches_klass(child, klass))
    }

    fn panel_matches_klass(panel: &Arc<dyn PanelView>, klass: &str, cx: &App) -> bool {
        let Ok(container) = panel.view().downcast::<DockPanelContainer>() else {
            return false;
        };

        let container = container.read(cx);
        container
            .agent_studio_klass
            .as_ref()
            .is_some_and(|panel_klass| panel_klass.as_ref() == klass)
    }
    /// Handle PanelAction - add/show panels with unified parameters
    pub(super) fn on_action_panel_action(
        &mut self,
        action: &PanelAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &action.0 {
            PanelCommand::Add { panel, placement } => match panel {
                PanelKind::Conversation { session_id } => {
                    self.add_conversation_panel_to(session_id.clone(), *placement, window, cx);
                }
                PanelKind::Terminal { working_directory } => {
                    self.add_terminal_panel_to(working_directory.clone(), *placement, window, cx);
                }
                PanelKind::CodeEditor { working_directory } => {
                    self.add_code_editor_panel_to(
                        working_directory.clone(),
                        *placement,
                        window,
                        cx,
                    );
                }
                PanelKind::Welcome { workspace_id } => {
                    self.add_welcome_panel_to(workspace_id.clone(), *placement, window, cx);
                }
                PanelKind::ToolCallDetail {
                    tool_call_id: _,
                    tool_call,
                } => {
                    self.show_tool_call_detail_panel((**tool_call).clone(), window, cx);
                }
            },
            PanelCommand::Show(panel) => match panel {
                PanelKind::Conversation { session_id } => {
                    self.show_conversation_panel(session_id.clone(), window, cx);
                }
                PanelKind::Terminal { working_directory } => {
                    self.add_terminal_panel_to(
                        working_directory.clone(),
                        DockPlacement::Bottom,
                        window,
                        cx,
                    );
                }
                PanelKind::CodeEditor { working_directory } => {
                    self.add_code_editor_panel_to(
                        working_directory.clone(),
                        DockPlacement::Right,
                        window,
                        cx,
                    );
                }
                PanelKind::Welcome { workspace_id } => {
                    self.show_welcome_panel(workspace_id.clone(), window, cx);
                }
                PanelKind::ToolCallDetail {
                    tool_call_id: _,
                    tool_call,
                } => {
                    self.show_tool_call_detail_panel((**tool_call).clone(), window, cx);
                }
            },
        }
    }

    fn add_conversation_panel_to(
        &mut self,
        session_id: Option<String>,
        placement: DockPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = session_id.filter(|id| !id.is_empty());
        if let Some(session_id) = session_id {
            if self.activate_existing_session_panel(&session_id, window, cx) {
                return;
            }

            Self::resume_session_if_needed(&session_id, cx);

            let panel = Arc::new(Self::panel_for_session(session_id, window, cx));
            self.dock_area.update(cx, |dock_area, cx| {
                // Check if dock is open BEFORE adding panel
                let was_dock_open = dock_area.is_dock_open(placement, cx);

                // Add panel to dock
                dock_area.add_panel(panel, placement, None, window, cx);

                // If dock was closed, toggle it to open it
                if !was_dock_open {
                    dock_area.toggle_dock(placement, window, cx);
                    log::debug!("Auto-expanded {:?} dock for conversation panel", placement);
                }
            });
            return;
        }

        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx));
        self.dock_area.update(cx, |dock_area, cx| {
            // Check if dock is open BEFORE adding panel
            let was_dock_open = dock_area.is_dock_open(placement, cx);

            // Add panel to dock
            dock_area.add_panel(panel, placement, None, window, cx);

            // If dock was closed, toggle it to open it
            if !was_dock_open {
                dock_area.toggle_dock(placement, window, cx);
                log::debug!("Auto-expanded {:?} dock for conversation panel", placement);
            }
        });
    }

    fn add_welcome_panel_to(
        &mut self,
        workspace_id: Option<String>,
        placement: DockPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create WelcomePanel with optional workspace_id
        let panel = if let Some(workspace_id) = workspace_id {
            Arc::new(DockPanelContainer::panel_for_workspace(
                workspace_id,
                window,
                cx,
            ))
        } else {
            Arc::new(DockPanelContainer::panel::<WelcomePanel>(window, cx))
        };

        self.dock_area.update(cx, |dock_area, cx| {
            // Check if dock is open BEFORE adding panel
            let was_dock_open = dock_area.is_dock_open(placement, cx);

            // Add panel to dock
            dock_area.add_panel(panel, placement, None, window, cx);
            // Collapse right and bottom docks if they are open
            if dock_area.is_dock_open(DockPlacement::Right, cx) {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
            // If dock was closed, toggle it to open it
            if !was_dock_open {
                dock_area.toggle_dock(placement, window, cx);
                log::debug!("Auto-expanded {:?} dock for welcome panel", placement);
            }
        });
    }

    fn add_terminal_panel_to(
        &mut self,
        working_directory: Option<std::path::PathBuf>,
        placement: DockPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = if let Some(working_directory) = working_directory {
            // 使用指定的工作目录创建终端面板
            Arc::new(DockPanelContainer::panel_for_terminal_with_cwd(
                working_directory,
                window,
                cx,
            ))
        } else {
            // 使用默认工作目录创建终端面板
            Arc::new(DockPanelContainer::panel::<crate::TerminalPanel>(
                window, cx,
            ))
        };

        self.dock_area.update(cx, |dock_area, cx| {
            // Check if dock is open BEFORE adding panel
            let was_dock_open = dock_area.is_dock_open(placement, cx);

            // Add panel to dock
            dock_area.add_panel(panel, placement, None, window, cx);

            // If dock was closed, toggle it to open it
            if !was_dock_open {
                dock_area.toggle_dock(placement, window, cx);
                log::debug!("Auto-expanded {:?} dock for terminal panel", placement);
            }
        });
    }

    fn add_code_editor_panel_to(
        &mut self,
        working_directory: Option<std::path::PathBuf>,
        placement: DockPlacement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = if let Some(working_directory) = working_directory {
            // 使用指定的工作目录创建代码编辑器面板
            Arc::new(DockPanelContainer::panel_for_code_editor_with_cwd(
                working_directory,
                window,
                cx,
            ))
        } else {
            // 使用默认工作目录创建代码编辑器面板
            Arc::new(DockPanelContainer::panel::<crate::CodeEditorPanel>(
                window, cx,
            ))
        };

        self.dock_area.update(cx, |dock_area, cx| {
            // Check if dock is open BEFORE adding panel
            let was_dock_open = dock_area.is_dock_open(placement, cx);

            // Add panel to dock
            dock_area.add_panel(panel, placement, None, window, cx);

            // If dock was closed, toggle it to open it
            if !was_dock_open {
                dock_area.toggle_dock(placement, window, cx);
                log::debug!("Auto-expanded {:?} dock for code editor panel", placement);
            }
        });
    }

    /// Handle TogglePanelVisible action - show/hide panels in the UI
    pub(super) fn on_action_toggle_panel_visible(
        &mut self,
        action: &TogglePanelVisible,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel_name = action.0.clone();
        let invisible_panels = AppState::global(cx).invisible_panels.clone();
        invisible_panels.update(cx, |names, cx| {
            if names.contains(&panel_name) {
                names.retain(|id| id != &panel_name);
            } else {
                names.push(panel_name);
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Handle ToggleDockToggleButton action - show/hide dock toggle buttons
    pub(super) fn on_action_toggle_dock_toggle_button(
        &mut self,
        _: &ToggleDockToggleButton,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_button_visible = !self.toggle_button_visible;

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_toggle_button_visible(self.toggle_button_visible, cx);
        });
    }

    /// Handle Open action - open folder picker and print selected path
    pub(super) fn on_action_open(
        &mut self,
        _: &crate::Open,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |_this, _cx| {
            utils::pick_and_log_folder("Open Project Folder", "Menu").await;
        })
        .detach();
    }
    pub(super) fn on_action_open_setting_panel(
        &mut self,
        _action: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!("Adding new Settings panel");
        let panel = Arc::new(DockPanelContainer::panel::<SettingsPanel>(window, cx));

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, DockPlacement::Center, None, window, cx);
        });
    }

    pub(super) fn on_action_open_session_manager(
        &mut self,
        _: &OpenSessionManager,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.activate_existing_session_manager_panel(window, cx) {
            self.dock_area.update(cx, |dock_area, cx| {
                if !dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                    dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
                }
            });
            return;
        }

        let panel = Arc::new(DockPanelContainer::panel::<SessionManagerPanel>(window, cx));
        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, DockPlacement::Bottom, None, window, cx);
            if !dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
    }
    fn show_welcome_panel(
        &mut self,
        workspace_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create WelcomePanel for the center with optional workspace_id
        let welcome_panel = if let Some(workspace_id) = &workspace_id {
            DockPanelContainer::panel_for_workspace(workspace_id.clone(), window, cx)
        } else {
            DockPanelContainer::panel::<WelcomePanel>(window, cx)
        };
        let welcome_item = DockItem::tab(welcome_panel, &self.dock_area.downgrade(), window, cx);

        let welcome_dock = DockItem::split_with_sizes(
            Axis::Horizontal,
            vec![welcome_item],
            vec![None, None],
            &self.dock_area.downgrade(),
            window,
            cx,
        );

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_center(welcome_dock, window, cx);

            // Collapse right and bottom docks if they are open
            if dock_area.is_dock_open(DockPlacement::Right, cx) {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
    }

    fn show_tool_call_detail_panel(
        &mut self,
        tool_call: crate::ToolCall,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::debug!("show_tool_call_detail_panel called");

        let panel = Arc::new(DockPanelContainer::panel_for_tool_call_detail(
            tool_call, window, cx,
        ));

        self.dock_area.update(cx, |dock_area, cx| {
            // Check if right dock is open BEFORE adding panel
            let was_dock_open = dock_area.is_dock_open(DockPlacement::Right, cx);
            log::debug!("Right dock open before add_panel: {}", was_dock_open);

            // Add panel to right dock
            dock_area.add_panel(panel, DockPlacement::Right, None, window, cx);

            // Always ensure the right dock is open after adding panel
            // If it was closed, toggle it to open it
            if !was_dock_open {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
                log::debug!("Toggled right dock to open");
            }

            log::debug!(
                "Added ToolCallDetail panel, right dock is now open: {}",
                dock_area.is_dock_open(DockPlacement::Right, cx)
            );
        });
    }

    /// Handle NewSessionConversationPanel action - add a new conversation panel
    pub(super) fn on_action_new_session_conversation_panel(
        &mut self,
        _action: &NewSessionConversationPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!("Adding new session conversation panel");
        self.add_conversation_panel(window, cx);
    }

    /// Handle CreateTaskFromWelcome action - create a new agent task from welcome panel
    /// Uses MessageService to handle session creation, event publishing, and prompt sending
    pub(super) fn on_action_create_task_from_welcome(
        &mut self,
        action: &CreateTaskFromWelcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_name = action.agent_name.clone();
        let task_input = action.task_input.clone();
        let mode = action.mode.clone();
        let images = action.images.clone();

        log::info!(
            "Creating task from welcome: agent={}, mode={}, input={}, images={}",
            agent_name,
            mode,
            task_input,
            images.len()
        );

        // Check for existing welcome session (created by WelcomePanel)
        let welcome_session = AppState::global(cx).welcome_session().cloned();

        // Get services
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("AgentService not initialized");
                return;
            }
        };

        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("MessageService not initialized");
                return;
            }
        };

        let agent_config_service = AppState::global(cx).agent_config_service().cloned();

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("WorkspaceService not initialized");
                return;
            }
        };

        let dock_area = self.dock_area.clone();

        // Get workspace_id from action or use active workspace
        let target_workspace_id = action.workspace_id.clone();

        cx.spawn_in(window, async move |_this, window| {
            // Step 1: Get target workspace (from action) or active workspace
            let workspace = if let Some(ws_id) = target_workspace_id {
                log::info!("Using specified workspace: {}", ws_id);
                match workspace_service.get_workspace(&ws_id).await {
                    Some(ws) => ws,
                    None => {
                        log::error!("Specified workspace not found: {}", ws_id);

                        // Show error notification
                        _ = window.update(|window, cx| {
                            struct WorkspaceNotFoundError;
                            let note = Notification::error(
                                format!("Workspace not found: {}", ws_id)
                            ).id::<WorkspaceNotFoundError>();
                            window.push_notification(note, cx);
                        });

                        return;
                    }
                }
            } else {
                log::info!("Using active workspace");
                match workspace_service.get_active_workspace().await {
                    Some(ws) => ws,
                    None => {
                        log::error!("No active workspace available");

                        // Show error notification
                        _ = window.update(|window, cx| {
                            struct NoActiveWorkspaceError;
                            let note = Notification::error(
                                "No workspace available. Please create or open a workspace first."
                            ).id::<NoActiveWorkspaceError>();
                            window.push_notification(note, cx);
                        });

                        return;
                    }
                }
            };

            let workspace_id = workspace.id.clone();
            let workspace_cwd = workspace.path.clone();

            log::info!(
                "Creating task in workspace: {} ({}), cwd: {:?}",
                workspace.name,
                workspace_id,
                workspace_cwd
            );

            // Step 2: Get or reuse session
            // IMPORTANT: Reuse welcome_session if it exists (created by WelcomePanel)
            // This ensures we use the same agent process that's already running
            let session_id = if let Some(ws) = welcome_session {
                log::info!(
                    "Reusing welcome session {} for agent {}",
                    ws.session_id,
                    ws.agent_name
                );
                ws.session_id
            } else {
                // No welcome session, create new one with workspace cwd
                let mcp_servers = if let Some(service) = agent_config_service {
                    service
                        .list_mcp_servers()
                        .await
                        .into_iter()
                        .filter(|(_, config)| config.enabled)
                        .map(|(name, config)| config.to_acp_mcp_server(name))
                        .collect()
                } else {
                    Vec::new()
                };

                log::info!(
                    "Creating new session for agent '{}' with cwd: {:?}",
                    agent_name,
                    workspace_cwd
                );

                match agent_service
                    .create_session_with_mcp_and_cwd(&agent_name, mcp_servers, workspace_cwd.clone())
                    .await
                {
                    Ok(session_id) => {
                        log::info!(
                            "Created new session {} for agent {}",
                            session_id,
                            agent_name
                        );
                        session_id
                    }
                    Err(e) => {
                        let (error_message, error_details) = if e.to_string().contains("server shut down unexpectedly") {
                            let details = format!(
                                "Agent '{}' process crashed during session creation. \
                                Possible reasons:\n\
                                1. npx/@zed-industries/claude-code-acp is not installed (run: npm install -g @zed-industries/claude-code-acp)\n\
                                2. Working directory '{}' does not exist or is not accessible\n\
                                3. Node.js is not properly installed or configured\n\
                                4. The agent binary has bugs or incompatibilities\n\n\
                                Original error: {}",
                                agent_name,
                                workspace_cwd.display(),
                                e
                            );
                            (
                                format!("Failed to create task: Agent '{}' crashed", agent_name),
                                details
                            )
                        } else {
                            (
                                format!("Failed to create task: {}", e),
                                e.to_string()
                            )
                        };

                        log::error!("{}", error_details);

                        // Show error notification to user
                        _ = window.update(|window, cx| {
                            struct TaskCreationError;
                            let note = Notification::error(error_message)
                                .id::<TaskCreationError>();
                            window.push_notification(note, cx);
                        });

                        return;
                    }
                }
            };

            // Step 3: Create WorkspaceTask

            // Create task in workspace
            let task = match workspace_service
                .create_task(
                    &workspace_id,
                    task_input.clone(),
                    agent_name.clone(),
                    mode.clone(),
                )
                .await
            {
                Ok(task) => {
                    log::info!(
                        "Created workspace task: {} in workspace: {}",
                        task.name,
                        workspace_id
                    );
                    task
                }
                Err(e) => {
                    log::error!("Failed to create workspace task: {}", e);

                    // Show error notification
                    _ = window.update(|window, cx| {
                        struct WorkspaceTaskCreationError;
                        let note = Notification::error(
                            format!("Failed to create task: {}", e)
                        ).id::<WorkspaceTaskCreationError>();
                        window.push_notification(note, cx);
                    });

                    return;
                }
            };

            // Associate session with task
            if let Err(e) = workspace_service
                .set_task_session(&task.id, session_id.clone())
                .await
            {
                log::error!("Failed to associate session with task: {}", e);
            }

            // Step 3: Clear welcome session and create ConversationPanel
            // Panel will subscribe to session updates BEFORE we send the message
            let session_id_for_send = session_id.clone();
            let task_id = task.id.clone();
            _ = window.update(move |window, cx| {
                // Clear welcome session
                AppState::global_mut(cx).clear_welcome_session();

                // Create panel - this subscribes to the session
                let conversation_panel = Self::panel_for_session(session_id, window, cx);

                let conversation_item =
                    DockItem::tab(conversation_panel, &dock_area.downgrade(), window, cx);

                // Wrap in split_with_sizes to ensure proper StackPanel hierarchy
                // This is required for zoom functionality and proper layout persistence
                let conversation_dock = DockItem::split_with_sizes(
                    Axis::Horizontal,
                    vec![conversation_item],
                    vec![None],
                    &dock_area.downgrade(),
                    window,
                    cx,
                );

                dock_area.update(cx, |dock_area, cx| {
                    dock_area.set_center(conversation_dock, window, cx);

                    // Collapse right and bottom docks
                    if dock_area.is_dock_open(DockPlacement::Right, cx) {
                        dock_area.toggle_dock(DockPlacement::Right, window, cx);
                    }
                    if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                        dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
                    }
                });

                // Task created - TaskPanel will pick it up via periodic refresh
                log::info!("[DockWorkspace] Task created ({})", task_id);
            });

            // Step 4: Build content blocks from text and images
            let mut prompt_blocks: Vec<acp::ContentBlock> = Vec::new();

            // Add text content
            prompt_blocks.push(task_input.into());

            // Add image contents - convert schema::ImageContent to agent_client_protocol::ImageContent
            for (image_content, _filename) in images.iter() {
                prompt_blocks.push(acp::ContentBlock::Image(image_content.clone()));
            }
            log::debug!("Built {} content blocks for prompt", prompt_blocks.len());

            // Step 5: Now send the message - panel is subscribed and will receive it
            match message_service
                .send_message_to_session(&agent_name, &session_id_for_send, prompt_blocks)
                .await
            {
                Ok(response) => {
                    log::info!(
                        "Message sent successfully to session {}, Response: {:?}",
                        session_id_for_send,
                        response
                    );
                }
                Err(e) => {
                    log::error!("Failed to send message: {}", e);

                    // Show error notification
                    _ = window.update(|window, cx| {
                        struct MessageSendError;
                        let note = Notification::error(
                            format!("Failed to send message: {}", e)
                        ).id::<MessageSendError>();
                        window.push_notification(note, cx);
                    });
                }
            }
        })
        .detach();
    }
    /// Create a panel specifically for a session (ConversationPanel only)
    pub fn panel_for_session(
        session_id: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<DockPanelContainer> {
        use crate::ConversationPanel;

        let name = ConversationPanel::title();
        let description = ConversationPanel::description();
        let agent_studio = ConversationPanel::view_for_session(session_id, window, cx);
        let agent_studio_klass = ConversationPanel::klass();

        let view = cx.new(|cx| {
            let mut agent_studio = DockPanelContainer::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(ConversationPanel::on_active_any);
            agent_studio.focus_handle = cx.focus_handle();
            agent_studio.closable = ConversationPanel::closable();
            agent_studio.zoomable = ConversationPanel::zoomable();
            agent_studio.name = name.into();
            agent_studio.description = description.into();
            agent_studio.title_bg = ConversationPanel::title_bg();
            agent_studio.paddings = ConversationPanel::paddings();
            agent_studio
        });

        view
    }

    /// Handle SendMessageToSession action - send a user message to an agent session
    /// This separates the Agent execution logic from the ConversationPanel UI component
    pub(super) fn on_action_send_message_to_session(
        &mut self,
        action: &SendMessageToSession,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = action.session_id.clone();
        let message = action.message.clone();
        let images = action.images.clone();

        log::info!("Sending message to session: {}", session_id);

        // Spawn async task to send the message
        cx.spawn(async move |_this, cx| {
            let agent_service = cx.update(|cx| AppState::global(cx).agent_service().cloned());

            let message_service = cx.update(|cx| AppState::global(cx).message_service().cloned());

            let (agent_service, message_service) = match (agent_service, message_service) {
                (Some(agent_service), Some(message_service)) => (agent_service, message_service),
                _ => {
                    log::error!("AgentService or MessageService not initialized");
                    return;
                }
            };

            let agent_name = match agent_service.get_agent_for_session(&session_id) {
                Some(agent_name) => agent_name,
                None => {
                    log::error!(
                        "Cannot send message: no agent found for session {}",
                        session_id
                    );
                    return;
                }
            };

            // Build prompt with text and images
            let mut prompt_blocks: Vec<acp::ContentBlock> = Vec::new();
            prompt_blocks.push(message.clone().into());
            for (image_content, _filename) in images.iter() {
                prompt_blocks.push(acp::ContentBlock::Image(image_content.clone()));
            }

            log::debug!(
                "Sending prompt to agent {} for session {}",
                agent_name,
                session_id
            );

            match message_service
                .send_message_to_session(&agent_name, &session_id, prompt_blocks)
                .await
            {
                Ok(_response) => {
                    log::info!("Prompt sent successfully to session: {}", session_id);
                }
                Err(e) => {
                    log::error!("Failed to send prompt to session {}: {}", session_id, e);
                }
            }
        })
        .detach();
    }

    /// Handle CancelSession action - cancel an ongoing session operation
    pub(super) fn on_action_cancel_session(
        &mut self,
        action: &CancelSession,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = action.session_id.clone();

        log::info!(
            "DockWorkspace: Received CancelSession action for session: {}",
            session_id
        );

        // Spawn async task to cancel the session
        cx.spawn(async move |_this, cx| {
            log::info!(
                "DockWorkspace: Starting async cancel task for session: {}",
                session_id
            );

            // Get AgentService to cancel the session
            let agent_service = cx.update(|cx| AppState::global(cx).agent_service().cloned());

            if let Some(agent_service) = agent_service {
                log::info!("DockWorkspace: Got AgentService");

                match agent_service.cancel_session_by_id(&session_id).await {
                    Ok(()) => {
                        log::info!(
                            "DockWorkspace: Session {} cancelled successfully",
                            session_id
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "DockWorkspace: Failed to cancel session {}: {}",
                            session_id,
                            e
                        );
                    }
                }
            } else {
                log::error!("DockWorkspace: AgentService not available");
            }
        })
        .detach();
    }
}

// ============================================================================
// Agent Configuration Action Handlers
// ============================================================================

pub fn add_agent(action: &AddAgent, cx: &mut App) {
    let agent_config_service = match AppState::global(cx).agent_config_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("AgentConfigService not initialized");
            return;
        }
    };

    let name = action.name.clone();
    let config = crate::core::config::AgentProcessConfig {
        command: action.command.clone(),
        args: action.args.clone(),
        env: action.env.clone(),
        nodejs_path: None,
    };

    let _ = cx
        .spawn(
            async move |_cx| match agent_config_service.add_agent(name.clone(), config).await {
                Ok(()) => {
                    log::info!("Successfully added agent: {}", name);
                }
                Err(e) => {
                    log::error!("Failed to add agent '{}': {}", name, e);
                }
            },
        )
        .detach();
}

pub fn update_agent(action: &UpdateAgent, cx: &mut App) {
    let agent_config_service = match AppState::global(cx).agent_config_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("AgentConfigService not initialized");
            return;
        }
    };

    let name = action.name.clone();
    let config = crate::core::config::AgentProcessConfig {
        command: action.command.clone(),
        args: action.args.clone(),
        env: action.env.clone(),
        nodejs_path: None,
    };

    let _ = cx
        .spawn(
            async move |_cx| match agent_config_service.update_agent(&name, config).await {
                Ok(()) => {
                    log::info!("Successfully updated agent: {}", name);
                }
                Err(e) => {
                    log::error!("Failed to update agent '{}': {}", name, e);
                }
            },
        )
        .detach();
}

pub fn remove_agent(action: &RemoveAgent, cx: &mut App) {
    let agent_config_service = match AppState::global(cx).agent_config_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("AgentConfigService not initialized");
            return;
        }
    };

    let name = action.name.clone();
    let _ = cx
        .spawn(async move |_cx| {
            // Check if agent has active sessions
            if agent_config_service.has_active_sessions(&name).await {
                log::warn!(
                    "Agent '{}' has active sessions. User should confirm removal.",
                    name
                );
                // In a full implementation, we'd show a confirmation dialog here
                // For now, we'll proceed with removal
            }

            match agent_config_service.remove_agent(&name).await {
                Ok(()) => {
                    log::info!("Successfully removed agent: {}", name);
                }
                Err(e) => {
                    log::error!("Failed to remove agent '{}': {}", name, e);
                }
            }
        })
        .detach();
}

pub fn restart_agent(action: &RestartAgent, cx: &mut App) {
    let agent_config_service = match AppState::global(cx).agent_config_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("AgentConfigService not initialized");
            return;
        }
    };

    let name = action.name.clone();

    let _ = cx
        .spawn(
            async move |_cx| match agent_config_service.restart_agent(&name).await {
                Ok(()) => {
                    log::info!("Successfully restarted agent: {}", name);
                }
                Err(e) => {
                    log::error!("Failed to restart agent '{}': {}", name, e);
                }
            },
        )
        .detach();
}

pub fn reload_agent_config(_action: &ReloadAgentConfig, cx: &mut App) {
    let agent_config_service = match AppState::global(cx).agent_config_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("AgentConfigService not initialized");
            return;
        }
    };

    let _ = cx
        .spawn(
            async move |_cx| match agent_config_service.reload_from_file().await {
                Ok(()) => {
                    log::info!("Successfully reloaded agent configuration");
                }
                Err(e) => {
                    log::error!("Failed to reload agent configuration: {}", e);
                }
            },
        )
        .detach();
}

pub fn set_upload_dir(action: &SetUploadDir, cx: &mut App) {
    let agent_config_service = match AppState::global(cx).agent_config_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("AgentConfigService not initialized");
            return;
        }
    };

    let path = action.path.clone();

    let _ = cx
        .spawn(
            async move |_cx| match agent_config_service.set_upload_dir(path.clone()).await {
                Ok(()) => {
                    log::info!("Successfully set upload directory to: {:?}", path);
                }
                Err(e) => {
                    log::error!("Failed to set upload directory: {}", e);
                }
            },
        )
        .detach();
}

pub fn change_config_path(action: &ChangeConfigPath, cx: &mut App) {
    let new_path = action.path.clone();

    // Validate that the file exists
    if !new_path.exists() {
        log::error!("Config file does not exist: {:?}", new_path);
        return;
    }

    // Read and validate the config file
    let config_result = std::fs::read_to_string(&new_path);
    match config_result {
        Ok(json) => {
            // Try to parse as Config to validate format
            match serde_json::from_str::<crate::core::config::Config>(&json) {
                Ok(_config) => {
                    log::info!("Config file validated successfully: {:?}", new_path);

                    // Update config path in AppState
                    AppState::global_mut(cx).set_config_path(new_path.clone());

                    // Reload the configuration from the new file
                    // Note: This requires restarting the application or reinitializing AgentConfigService
                    // For now, we'll just log a message asking the user to restart
                    log::warn!(
                        "Config path changed to: {:?}. Please restart the application to apply changes.",
                        new_path
                    );

                    // Alternatively, trigger a reload if the service supports it
                    if let Some(service) = AppState::global(cx).agent_config_service() {
                        let service = service.clone();
                        cx.spawn(async move |_cx| match service.reload_from_file().await {
                            Ok(()) => {
                                log::info!("Successfully reloaded configuration from new file");
                            }
                            Err(e) => {
                                log::error!("Failed to reload configuration: {}", e);
                            }
                        })
                        .detach();
                    }
                }
                Err(e) => {
                    log::error!("Invalid config file format: {}", e);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to read config file: {}", e);
        }
    }
}
