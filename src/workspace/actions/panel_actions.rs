use gpui::*;
use gpui_component::{
    WindowExt,
    dock::{DockItem, DockPlacement, Panel, PanelInfo, PanelState, PanelView, TabPanel},
};
use std::sync::Arc;

use crate::{
    AppState, ConversationPanel, OpenSessionManager, PanelAction, SessionManagerPanel,
    SettingsPanel, ToggleDockToggleButton, TogglePanelVisible, WelcomePanel,
    app::actions::{PanelCommand, PanelKind, Submit},
    panels::{
        DockPanel,
        dock_panel::{DockPanelContainer, DockPanelState},
    },
    title_bar::OpenSettings,
    utils,
};

use crate::workspace::DockWorkspace;

impl DockWorkspace {
    pub(in crate::workspace) fn submit(
        &mut self,
        _: &Submit,
        _: &mut Window,
        _cx: &mut Context<Self>,
    ) {
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
    pub(in crate::workspace) fn show_conversation_panel(
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

    pub(in crate::workspace) fn resume_session_if_needed(session_id: &str, cx: &mut Context<Self>) {
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
    pub(in crate::workspace) fn on_action_panel_action(
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
                let was_dock_open = dock_area.is_dock_open(placement, cx);
                dock_area.add_panel(panel, placement, None, window, cx);
                if !was_dock_open {
                    dock_area.toggle_dock(placement, window, cx);
                    log::debug!("Auto-expanded {:?} dock for conversation panel", placement);
                }
            });
            return;
        }

        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx));
        self.dock_area.update(cx, |dock_area, cx| {
            let was_dock_open = dock_area.is_dock_open(placement, cx);
            dock_area.add_panel(panel, placement, None, window, cx);
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
            let was_dock_open = dock_area.is_dock_open(placement, cx);
            dock_area.add_panel(panel, placement, None, window, cx);
            if dock_area.is_dock_open(DockPlacement::Right, cx) {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
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
            Arc::new(DockPanelContainer::panel_for_terminal_with_cwd(
                working_directory,
                window,
                cx,
            ))
        } else {
            Arc::new(DockPanelContainer::panel::<crate::TerminalPanel>(
                window, cx,
            ))
        };

        self.dock_area.update(cx, |dock_area, cx| {
            let was_dock_open = dock_area.is_dock_open(placement, cx);
            dock_area.add_panel(panel, placement, None, window, cx);
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
            Arc::new(DockPanelContainer::panel_for_code_editor_with_cwd(
                working_directory,
                window,
                cx,
            ))
        } else {
            Arc::new(DockPanelContainer::panel::<crate::CodeEditorPanel>(
                window, cx,
            ))
        };

        self.dock_area.update(cx, |dock_area, cx| {
            let was_dock_open = dock_area.is_dock_open(placement, cx);
            dock_area.add_panel(panel, placement, None, window, cx);
            if !was_dock_open {
                dock_area.toggle_dock(placement, window, cx);
                log::debug!("Auto-expanded {:?} dock for code editor panel", placement);
            }
        });
    }

    /// Handle TogglePanelVisible action - show/hide panels in the UI
    pub(in crate::workspace) fn on_action_toggle_panel_visible(
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
    pub(in crate::workspace) fn on_action_toggle_dock_toggle_button(
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
    pub(in crate::workspace) fn on_action_open(
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

    pub(in crate::workspace) fn on_action_open_setting_panel(
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

    pub(in crate::workspace) fn on_action_open_session_manager(
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

    pub(in crate::workspace) fn show_welcome_panel(
        &mut self,
        workspace_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

            if dock_area.is_dock_open(DockPlacement::Right, cx) {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
    }

    pub(in crate::workspace) fn show_tool_call_detail_panel(
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
            let was_dock_open = dock_area.is_dock_open(DockPlacement::Right, cx);
            log::debug!("Right dock open before add_panel: {}", was_dock_open);

            dock_area.add_panel(panel, DockPlacement::Right, None, window, cx);

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
}
