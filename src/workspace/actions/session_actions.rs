use agent_client_protocol as acp;
use gpui::*;
use gpui_component::{
    WindowExt,
    dock::{DockItem, DockPlacement},
    notification::Notification,
};
use std::sync::Arc;

use crate::{
    AppState, ConversationPanel, CreateTaskFromWelcome, NewSessionConversationPanel,
    SendMessageToSession,
    app::actions::CancelSession,
    panels::{DockPanel, dock_panel::DockPanelContainer},
};

use crate::workspace::DockWorkspace;

impl DockWorkspace {
    /// Handle NewSessionConversationPanel action - add a new conversation panel
    pub(in crate::workspace) fn on_action_new_session_conversation_panel(
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
    pub(in crate::workspace) fn on_action_create_task_from_welcome(
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

        let welcome_session = AppState::global(cx).welcome_session().cloned();

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
        let target_workspace_id = action.workspace_id.clone();

        cx.spawn_in(window, async move |_this, window| {
            // Step 1: Get target workspace
            let workspace = if let Some(ws_id) = target_workspace_id {
                log::info!("Using specified workspace: {}", ws_id);
                match workspace_service.get_workspace(&ws_id).await {
                    Some(ws) => ws,
                    None => {
                        log::error!("Specified workspace not found: {}", ws_id);
                        _ = window.update(|window, cx| {
                            struct WorkspaceNotFoundError;
                            let note = Notification::error(format!(
                                "Workspace not found: {}",
                                ws_id
                            ))
                            .id::<WorkspaceNotFoundError>();
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
                        _ = window.update(|window, cx| {
                            struct NoActiveWorkspaceError;
                            let note = Notification::error(
                                "No workspace available. Please create or open a workspace first.",
                            )
                            .id::<NoActiveWorkspaceError>();
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
            let session_id = if let Some(ws) = welcome_session {
                log::info!(
                    "Reusing welcome session {} for agent {}",
                    ws.session_id,
                    ws.agent_name
                );
                ws.session_id
            } else {
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
                    .create_session_with_mcp_and_cwd(
                        &agent_name,
                        mcp_servers,
                        workspace_cwd.clone(),
                    )
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
                        let (error_message, error_details) =
                            if e.to_string().contains("server shut down unexpectedly") {
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
                                    format!(
                                        "Failed to create task: Agent '{}' crashed",
                                        agent_name
                                    ),
                                    details,
                                )
                            } else {
                                (
                                    format!("Failed to create task: {}", e),
                                    e.to_string(),
                                )
                            };

                        log::error!("{}", error_details);

                        _ = window.update(|window, cx| {
                            struct TaskCreationError;
                            let note =
                                Notification::error(error_message).id::<TaskCreationError>();
                            window.push_notification(note, cx);
                        });

                        return;
                    }
                }
            };

            // Step 3: Create WorkspaceTask
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
                    _ = window.update(|window, cx| {
                        struct WorkspaceTaskCreationError;
                        let note = Notification::error(format!(
                            "Failed to create task: {}",
                            e
                        ))
                        .id::<WorkspaceTaskCreationError>();
                        window.push_notification(note, cx);
                    });
                    return;
                }
            };

            if let Err(e) = workspace_service
                .set_task_session(&task.id, session_id.clone())
                .await
            {
                log::error!("Failed to associate session with task: {}", e);
            }

            // Step 4: Clear welcome session and create ConversationPanel
            let session_id_for_send = session_id.clone();
            let task_id = task.id.clone();
            _ = window.update(move |window, cx| {
                AppState::global_mut(cx).clear_welcome_session();

                let conversation_panel = Self::panel_for_session(session_id, window, cx);
                let conversation_item =
                    DockItem::tab(conversation_panel, &dock_area.downgrade(), window, cx);

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

                    if dock_area.is_dock_open(DockPlacement::Right, cx) {
                        dock_area.toggle_dock(DockPlacement::Right, window, cx);
                    }
                    if dock_area.is_dock_open(DockPlacement::Bottom, cx) {
                        dock_area.toggle_dock(DockPlacement::Bottom, window, cx);
                    }
                });

                log::info!("[DockWorkspace] Task created ({})", task_id);
            });

            // Step 5: Build content blocks and send message
            let mut prompt_blocks: Vec<acp::ContentBlock> = Vec::new();
            prompt_blocks.push(task_input.into());
            for (image_content, _filename) in images.iter() {
                prompt_blocks.push(acp::ContentBlock::Image(image_content.clone()));
            }
            log::debug!("Built {} content blocks for prompt", prompt_blocks.len());

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
                    _ = window.update(|window, cx| {
                        struct MessageSendError;
                        let note = Notification::error(format!(
                            "Failed to send message: {}",
                            e
                        ))
                        .id::<MessageSendError>();
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

    /// Handle SendMessageToSession action
    pub(in crate::workspace) fn on_action_send_message_to_session(
        &mut self,
        action: &SendMessageToSession,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = action.session_id.clone();
        let message = action.message.clone();
        let images = action.images.clone();

        log::info!("Sending message to session: {}", session_id);

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
    pub(in crate::workspace) fn on_action_cancel_session(
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

        cx.spawn(async move |_this, cx| {
            log::info!(
                "DockWorkspace: Starting async cancel task for session: {}",
                session_id
            );

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
