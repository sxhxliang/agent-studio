use agent_client_protocol as acp;
use gpui::*;
use gpui_component::dock::{
    DockItem, DockPlacement, Panel, PanelInfo, PanelState, PanelView, TabPanel
};
use std::sync::Arc;

use crate::{
    AddPanel, AddSessionPanel, AppState, ConversationPanel, CreateTaskFromWelcome,
    NewSessionConversationPanel, SendMessageToSession, SettingsPanel, ShowConversationPanel,
    ShowToolCallDetail, ShowWelcomePanel, ToggleDockToggleButton, TogglePanelVisible, WelcomePanel,
    app::actions::{
        AddAgent, CancelSession, ChangeConfigPath, ReloadAgentConfig, RemoveAgent, RestartAgent,
        SetUploadDir, Submit, UpdateAgent,
    },
    panels::{DockPanel, dock_panel::{DockPanelContainer, DockPanelState}},
    title_bar::OpenSettings,
    utils,
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
        if let Some(session_id) = session_id.as_deref() {
            if self.activate_existing_session_panel(session_id, window, cx) {
                return;
            }

            let session_id = session_id.to_string();
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
            let selection =
                Self::find_focused_tab_panel(dock_area.items(), window, cx)
                    .or_else(|| Self::find_first_tab_panel(dock_area.items(), cx));

            if let Some((_, active_panel)) = selection {
                if let Ok(container) = active_panel.view().downcast::<DockPanelContainer>() {
                    container.update(cx, |container, cx| {
                        container
                            .replace_with_conversation_session(session_id.clone(), window, cx);
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

    fn find_focused_tab_panel(
        item: &DockItem,
        window: &Window,
        cx: &App,
    ) -> Option<(Entity<TabPanel>, Arc<dyn PanelView>)> {
        match item {
            DockItem::Tabs { view, .. } => {
                let active_panel = view.read(cx).active_panel(cx)?;
                if active_panel
                    .focus_handle(cx)
                    .contains_focused(window, cx)
                {
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
        let items = self.dock_area.read(cx).items().clone();
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
            DockItem::Tabs {
                view,
                ..
            } => {
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
            DockItem::Split { items, .. } => items.iter().any(|item| {
                Self::activate_session_in_item(item, session_id, window, cx)
            }),
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
                if dock_state.story_klass.as_ref() == "ConversationPanel"
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

    fn panel_matches_session(
        panel: &Arc<dyn PanelView>,
        session_id: &str,
        cx: &App,
    ) -> bool {
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
        let Some(story_klass) = container.story_klass.as_ref() else {
            log::debug!(
                "Panel has no story klass: session_id={} panel_id={:?}",
                session_id,
                panel_id
            );
            return false;
        };

        if story_klass.as_ref() != "ConversationPanel" {
            log::debug!(
                "Panel story klass mismatch: session_id={} panel_id={:?} story_klass={}",
                session_id,
                panel_id,
                story_klass.as_ref()
            );
            return false;
        }

        let Some(story) = container.story.clone() else {
            log::debug!(
                "Conversation panel missing story: session_id={} panel_id={:?}",
                session_id,
                panel_id
            );
            return false;
        };

        let Ok(conversation) = story.downcast::<ConversationPanel>() else {
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
    /// Handle AddPanel action - randomly add a conversation panel to specified dock area
    pub(super) fn on_action_add_panel(
        &mut self,
        action: &AddPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Random pick up a panel to add
        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx));

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, action.0, None, window, cx);
        });
    }

    /// Handle AddSessionPanel action - add a conversation panel for a specific session
    pub(super) fn on_action_add_session_panel(
        &mut self,
        action: &AddSessionPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !action.session_id.is_empty()
            && self.activate_existing_session_panel(&action.session_id, window, cx)
        {
            return;
        }

        let panel = if action.session_id.is_empty() {
            Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx))
        } else {
            Arc::new(Self::panel_for_session(action.session_id.clone(), window, cx))
        };

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, action.placement, None, window, cx);
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
    /// Handle ShowWelcomePanel action - display welcome panel and collapse docks
    pub(super) fn on_action_show_welcome_panel(
        &mut self,
        action: &ShowWelcomePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Create WelcomePanel for the center with optional workspace_id
        let welcome_panel = if let Some(workspace_id) = &action.workspace_id {
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

    /// Handle ShowToolCallDetail action - display tool call detail panel in right dock
    pub(super) fn on_action_show_tool_call_detail_panel(
        &mut self,
        action: &ShowToolCallDetail,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::debug!("on_action_show_tool_call_detail_panel called");

        let panel = Arc::new(DockPanelContainer::panel_for_tool_call_detail(
            action.tool_call.clone(),
            window,
            cx,
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

    /// Handle ShowConversationPanel action - display conversation panel
    pub(super) fn on_action_show_conversation_panel(
        &mut self,
        action: &ShowConversationPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_conversation_panel(action.session_id.clone(), window, cx);
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

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("WorkspaceService not initialized");
                return;
            }
        };

        let dock_area = self.dock_area.clone();

        cx.spawn_in(window, async move |_this, window| {
            // Step 1: Get or reuse session
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
                // No welcome session, create new one
                match agent_service.create_session(&agent_name).await {
                    Ok(session_id) => {
                        log::info!(
                            "Created new session {} for agent {}",
                            session_id,
                            agent_name
                        );
                        session_id
                    }
                    Err(e) => {
                        log::error!("Failed to create session: {}", e);
                        return;
                    }
                }
            };

            // Step 2: Create WorkspaceTask
            // Get active workspace
            let workspace = match workspace_service.get_active_workspace().await {
                Some(ws) => ws,
                None => {
                    log::error!("No active workspace available");
                    return;
                }
            };

            let workspace_id = workspace.id.clone();

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

                dock_area.update(cx, |dock_area, cx| {
                    dock_area.set_center(conversation_item, window, cx);

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
        let story = ConversationPanel::view_for_session(session_id, window, cx);
        let story_klass = ConversationPanel::klass();

        let view = cx.new(|cx| {
            let mut story = DockPanelContainer::new(cx)
                .story(story.into(), story_klass)
                .on_active(ConversationPanel::on_active_any);
            story.focus_handle = cx.focus_handle();
            story.closable = ConversationPanel::closable();
            story.zoomable = ConversationPanel::zoomable();
            story.name = name.into();
            story.description = description.into();
            story.title_bg = ConversationPanel::title_bg();
            story.paddings = ConversationPanel::paddings();
            story
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
            // Step 1: Immediately publish user message to session bus for instant UI feedback
            use std::sync::Arc;

            // Create user message chunk
            let content_block = acp::ContentBlock::from(message.clone());
            let content_chunk = acp::ContentChunk::new(content_block);

            let user_event = crate::core::event_bus::session_bus::SessionUpdateEvent {
                session_id: session_id.clone(),
                agent_name: None,
                update: Arc::new(acp::SessionUpdate::UserMessageChunk(content_chunk)),
            };

            // Publish to session bus
            cx.update(|cx| {
                AppState::global(cx).session_bus.publish(user_event);
            })
            .ok();
            log::info!("Published user message to session bus: {}", session_id);

            // Step 2: Get agent handle and send prompt
            let agent_manager = cx
                .update(|cx| AppState::global(cx).agent_manager().cloned())
                .ok()
                .flatten();

            let agent_handle: Option<std::sync::Arc<crate::AgentHandle>> =
                if let Some(manager) = agent_manager {
                    // Get the first available agent
                    let agents = manager.list_agents().await;
                    if let Some(name) = agents.first() {
                        manager.get(name).await
                    } else {
                        None
                    }
                } else {
                    None
                };

            if let Some(agent_handle) = agent_handle {
                // Build prompt with text and images
                let mut prompt_blocks: Vec<acp::ContentBlock> = Vec::new();

                // Add text content
                prompt_blocks.push(message.clone().into());

                // Add image contents
                for (image_content, _filename) in images.iter() {
                    prompt_blocks.push(acp::ContentBlock::Image(image_content.clone()));
                }
                log::debug!("---------> Sending prompt: {:?}", prompt_blocks);

                // Send the prompt
                let request = acp::PromptRequest::new(
                    acp::SessionId::from(session_id.to_string()),
                    prompt_blocks,
                );

                match agent_handle.prompt(request).await {
                    Ok(_response) => {
                        log::info!("Prompt sent successfully to session: {}", session_id);
                    }
                    Err(e) => {
                        log::error!("Failed to send prompt to session {}: {}", session_id, e);
                    }
                }
            } else {
                log::error!("No agent handle available");
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

            // Get AgentService to find which agent owns this session
            let agent_service = cx
                .update(|cx| AppState::global(cx).agent_service().cloned())
                .ok()
                .flatten();

            if let Some(agent_service) = agent_service {
                log::info!("DockWorkspace: Got AgentService");

                // List all sessions to find the agent name
                let sessions = agent_service.list_sessions();
                log::info!("DockWorkspace: Found {} total sessions", sessions.len());

                if let Some(session_info) = sessions.iter().find(|s| s.session_id == session_id) {
                    let agent_name = session_info.agent_name.clone();
                    log::info!(
                        "DockWorkspace: Found session {} belongs to agent: {}",
                        session_id,
                        agent_name
                    );

                    // Cancel the session
                    match agent_service.cancel_session(&agent_name, &session_id).await {
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
                    log::error!("DockWorkspace: Session {} not found in list", session_id);
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
