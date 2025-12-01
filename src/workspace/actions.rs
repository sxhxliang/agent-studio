use agent_client_protocol as acp;
use gpui::*;
use gpui_component::dock::{DockItem, DockPlacement};
use std::sync::Arc;

use crate::{
    dock_panel::DockPanelContainer, utils, AddPanel, AppState, ConversationPanelAcp,
    CreateTaskFromWelcome, NewSessionConversationPanel, ShowConversationPanel, ShowWelcomePanel,
    ToggleDockToggleButton, TogglePanelVisible, WelcomePanel,
};

use super::DockWorkspace;
//   - on_action_add_panel - 添加面板到 dock 区域
//   - on_action_toggle_panel_visible - 切换面板可见性
//   - on_action_toggle_dock_toggle_button - 切换 dock 按钮显示
//   - on_action_open - 打开文件夹选择器
//   - on_action_show_welcome_panel - 显示欢迎面板
//   - on_action_show_conversation_panel - 显示对话面板
//   - on_action_create_task_from_welcome - 从欢迎面板创建任务

impl DockWorkspace {
    /// Helper method to create and add a new ConversationPanelAcp to the center
    pub fn add_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
            window, cx,
        ));

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, DockPlacement::Center, None, window, cx);
        });
    }

    /// Helper method to show ConversationPanelAcp in the current active tab
    /// This will add the panel to the current TabPanel instead of replacing the entire center
    fn show_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let conversation_panel = Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
            window, cx,
        ));
        let conversation_panel = DockPanelContainer::panel::<ConversationPanelAcp>(window, cx);
        let conversation_item =
            DockItem::tab(conversation_panel, &self.dock_area.downgrade(), window, cx);
        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_center(conversation_item, window, cx);
            // Add to current center TabPanel, similar to clicking a file in an editor
            // dock_area.add_panel(conversation_panel, DockPlacement::Center, None, window, cx);
        });
    }
    /// Handle AddPanel action - randomly add a conversation panel to specified dock area
    pub(super) fn on_action_add_panel(
        &mut self,
        action: &AddPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Random pick up a panel to add
        let panel = match rand::random::<usize>() % 2 {
            0 => Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
                window, cx,
            )),
            1 => Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
                window, cx,
            )),
            _ => Arc::new(DockPanelContainer::panel::<ConversationPanelAcp>(
                window, cx,
            )),
        };

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, action.0, None, window, cx);
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

    /// Handle ShowWelcomePanel action - display welcome panel and collapse docks
    pub(super) fn on_action_show_welcome_panel(
        &mut self,
        _: &ShowWelcomePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create WelcomePanel for the center
        let welcome_panel = DockPanelContainer::panel::<WelcomePanel>(window, cx);
        let welcome_item = DockItem::tab(welcome_panel, &self.dock_area.downgrade(), window, cx);

        self.dock_area.update(cx, |dock_area, cx| {
            // Replace center with WelcomePanel
            dock_area.set_center(welcome_item, window, cx);

            // Collapse right and bottom docks if they are open
            if dock_area.is_dock_open(DockPlacement::Right, cx) {
                dock_area.toggle_dock(DockPlacement::Right, window, cx);
            }
            if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
    }

    /// Handle ShowConversationPanel action - display conversation panel
    pub(super) fn on_action_show_conversation_panel(
        &mut self,
        _: &ShowConversationPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_conversation_panel(window, cx);
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
    pub(super) fn on_action_create_task_from_welcome(
        &mut self,
        action: &CreateTaskFromWelcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_name = action.agent_name.clone();
        let task_input = action.task_input.clone();
        let mode = action.mode.clone();

        log::info!(
            "Creating task from welcome: agent={}, mode={}, input={}",
            agent_name,
            mode,
            task_input
        );

        // Check if we have an existing welcome session
        let existing_session = AppState::global(cx).welcome_session().cloned();

        let dock_area = self.dock_area.clone();

        cx.spawn_in(window, async move |_this, window| {
            // Determine session_id: use existing or create new
            let (session_id_str, session_id_obj, agent_handle) =
                if let Some(session) = existing_session {
                    log::info!("Using existing welcome session: {}", session.session_id);

                    // Get agent handle from session's agent_name
                    let agent_handle = window
                        .update(|_, cx| {
                            AppState::global(cx)
                                .agent_manager()
                                .and_then(|m| m.get(&session.agent_name))
                        })
                        .ok()
                        .flatten();

                    let agent_handle = match agent_handle {
                        Some(handle) => handle,
                        None => {
                            log::error!(
                                "Agent not found for existing session: {}",
                                session.agent_name
                            );
                            return;
                        }
                    };

                    // Clone session_id to avoid lifetime issues
                    let session_id_str = session.session_id.clone();
                    let session_id_obj = acp::SessionId::from(session_id_str.clone());

                    (session_id_str, session_id_obj, agent_handle)
                } else {
                    // Fallback: create new session (for compatibility)
                    log::info!("No existing welcome session, creating new session...");

                    let agent_handle = window
                        .update(|_, cx| {
                            AppState::global(cx)
                                .agent_manager()
                                .and_then(|m| m.get(&agent_name))
                        })
                        .ok()
                        .flatten();

                    let agent_handle = match agent_handle {
                        Some(handle) => handle,
                        None => {
                            log::error!("Agent not found: {}", agent_name);
                            return;
                        }
                    };

                    let new_session_req = acp::NewSessionRequest {
                        cwd: std::env::current_dir().unwrap_or_default(),
                        mcp_servers: vec![],
                        meta: None,
                    };

                    let session_id_obj = match agent_handle.new_session(new_session_req).await {
                        Ok(resp) => resp.session_id,
                        Err(e) => {
                            log::error!("Failed to create session: {}", e);
                            return;
                        }
                    };

                    let session_id_str = session_id_obj.to_string();
                    log::info!("New session created: {}", session_id_str);

                    (session_id_str, session_id_obj, agent_handle)
                };

            // Clear the welcome session from AppState
            _ = window.update(|_, cx| {
                AppState::global_mut(cx).clear_welcome_session();
            });

            // 2. Update UI (Create Panel AND Publish Event)
            let session_id_str_clone = session_id_str.clone();
            let task_input_clone = task_input.clone();

            _ = window.update(move |window, cx| {
                // A. Create Panel (Subscribes to bus immediately)
                let conversation_panel =
                    DockPanelContainer::panel_for_session(session_id_str_clone.clone(), window, cx);

                let conversation_item =
                    DockItem::tab(conversation_panel, &dock_area.downgrade(), window, cx);

                dock_area.update(cx, |dock_area, cx| {
                    dock_area.set_center(conversation_item, window, cx);

                    // Collapse others
                    if dock_area.is_dock_open(DockPlacement::Right, cx) {
                        dock_area.toggle_dock(DockPlacement::Right, window, cx);
                    }
                    if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                        dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
                    }
                });

                // B. Publish User Message Event (Panel is now listening)
                use agent_client_protocol_schema as schema;
                let content_block = schema::ContentBlock::from(task_input_clone);
                let content_chunk = schema::ContentChunk::new(content_block);

                let user_event = crate::session_bus::SessionUpdateEvent {
                    session_id: session_id_str_clone,
                    update: Arc::new(schema::SessionUpdate::UserMessageChunk(content_chunk)),
                };

                AppState::global(cx).session_bus.publish(user_event);
            });

            // 3. Send Prompt
            let prompt_req = acp::PromptRequest {
                session_id: session_id_obj,
                prompt: vec![task_input.into()],
                meta: None,
            };

            if let Err(e) = agent_handle.prompt(prompt_req).await {
                log::error!("Failed to send prompt: {}", e);
            }
        })
        .detach();
    }
}
