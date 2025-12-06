use agent_client_protocol as acp;
use gpui::*;
use gpui_component::dock::{DockItem, DockPlacement};
use rand;
use std::sync::Arc;

use crate::{
    app::actions::{Paste, Submit},
    panels::{dock_panel::DockPanelContainer, DockPanel},
    title_bar::OpenSettings,
    utils, AddPanel, AppState, ConversationPanel, CreateTaskFromWelcome,
    NewSessionConversationPanel, SettingsPanel, ShowConversationPanel, ShowToolCallDetail,
    ShowWelcomePanel, ToggleDockToggleButton, TogglePanelVisible, WelcomePanel,
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
    pub(super) fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        // println!("Submitted URL: {}", self.content);
        // cx.emit(UrlInputEvent::SubmitRequested);
    }

    pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("Pasting from clipboard in workspace");
        if let Some(clipboard_item) = cx.read_from_clipboard() {
            let mut image_paths = Vec::new();

            for entry in clipboard_item.entries().iter() {
                if let ClipboardEntry::Image(image) = entry {
                    // Handle image entry
                    log::info!("Pasted image {:?}", image.format);
                    // Clone the image data to move into the async closure
                    let image = image.clone();

                    // Write image to temporary file synchronously in a spawn
                    let (tx, rx) = std::sync::mpsc::channel();
                    cx.spawn_in(window, async move |_this, _window| {
                        match utils::file::write_image_to_temp_file(&image).await {
                            Ok(temp_path) => {
                                log::info!("Image written to temporary file: {}", temp_path);
                                let _ = tx.send(Some(temp_path));
                            }
                            Err(e) => {
                                log::error!("Failed to write image to temporary file: {}", e);
                                let _ = tx.send(None);
                            }
                        }
                    })
                    .detach();

                    // Try to receive the path (non-blocking for now, will be collected later)
                    if let Ok(Some(path)) = rx.recv() {
                        image_paths.push(path);
                    }
                }
            }

            // Process text if needed
            if let Some(text_content) = clipboard_item.text() {
                log::info!("Pasted text: {}", text_content);
                // TODO: Write text to temporary file if needed, or pass directly to chat input
            }
        }
    }
    /// Helper method to create and add a new ConversationPanel to the center
    pub fn add_conversation_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx));

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.add_panel(panel, DockPlacement::Center, None, window, cx);
        });
    }

    /// Helper method to show ConversationPanel in the current active tab
    /// This will add the panel to the current TabPanel instead of replacing the entire center
    ///
    /// If session_id is provided, it will load the conversation history for that session
    /// Otherwise, it will create a new conversation panel with mock data
    fn show_conversation_panel(
        &mut self,
        session_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let conversation_panel = if let Some(session_id) = session_id {
            // Create panel for specific session (will load history automatically)
            DockPanelContainer::panel_for_session(session_id, window, cx)
        } else {
            // Create new panel without session (mock data)
            DockPanelContainer::panel::<ConversationPanel>(window, cx)
        };

        let conversation_item =
            DockItem::tab(conversation_panel, &self.dock_area.downgrade(), window, cx);

        let conversation_dock = DockItem::split_with_sizes(
            Axis::Horizontal,
            vec![conversation_item],
            vec![None, None],
            &self.dock_area.downgrade(),
            window,
            cx,
        );

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_center(conversation_dock, window, cx);
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
            0 => Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx)),
            1 => Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx)),
            _ => Arc::new(DockPanelContainer::panel::<ConversationPanel>(window, cx)),
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
                // let acp_image = acp::ImageContent::new(data, mime_type)
                // let acp_image = acp::ImageContent {
                //     annotations: None,
                //     data: image_content.data.clone(),
                //     mime_type: image_content.mime_type.clone(),
                //     uri: image_content.uri.clone(),
                //     meta: None,
                // };
                prompt_blocks.push(acp::ContentBlock::Image(image_content.clone()));
            }
            log::debug!("Built {} content blocks for prompt", prompt_blocks.len());

            // Step 5: Now send the message - panel is subscribed and will receive it
            match message_service
                .send_message_to_session(&agent_name, &session_id_for_send, prompt_blocks)
                .await
            {
                Ok(_) => {
                    log::info!(
                        "Message sent successfully to session {}",
                        session_id_for_send
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
}
