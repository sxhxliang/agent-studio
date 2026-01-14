use gpui::{
    App, AppContext, ClipboardEntry, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Window, px,
};
use rust_i18n::t;
use std::collections::HashSet;

use gpui_component::{
    ActiveTheme, IndexPath, StyledExt,
    input::InputState,
    list::ListState,
    select::{SelectEvent, SelectState},
    v_flex,
};

use agent_client_protocol::{self as acp, AvailableCommand, ImageContent};

use crate::{
    AppState, CreateTaskFromWelcome, WelcomeSession,
    app::actions::AddCodeSelection,
    components::{
        AgentItem, ChatInputBox, FileItem, FilePickerDelegate, ModeSelectItem, ModelSelectItem,
    },
    core::{config::McpServerConfig, services::AgentSessionInfo},
};

// File picker delegate is now imported from components module

const MAX_FILE_SUGGESTIONS: usize = 8;

/// Welcome panel displayed when creating a new task.
/// Shows a centered input form with title, instructions, and send button.
pub struct WelcomePanel {
    focus_handle: FocusHandle,
    input_state: Entity<InputState>,
    context_list: Entity<ListState<FilePickerDelegate>>,
    mode_select: Entity<SelectState<Vec<ModeSelectItem>>>,
    model_select: Entity<SelectState<Vec<ModelSelectItem>>>,
    agent_select: Entity<SelectState<Vec<AgentItem>>>,
    session_select: Entity<SelectState<Vec<String>>>,
    current_agent_name: Option<String>,
    current_session_id: Option<String>,
    has_agents: bool,
    has_modes: bool,
    has_models: bool,
    is_session_loading: bool,
    has_workspace: bool,
    active_workspace_name: Option<String>,
    /// Specific workspace ID to display (if provided via action)
    workspace_id: Option<String>,
    /// Working directory for file operations
    working_directory: std::path::PathBuf,
    pasted_images: Vec<(ImageContent, String)>,
    code_selections: Vec<AddCodeSelection>,
    selected_files: Vec<String>,
    file_suggestions: Vec<FileItem>,
    /// Command suggestions based on input
    command_suggestions: Vec<AvailableCommand>,
    /// Whether to show command suggestions (input starts with /)
    show_command_suggestions: bool,
    /// Selected command index for keyboard navigation
    _subscriptions: Vec<Subscription>,
    /// Available MCP servers (name, config)
    available_mcps: Vec<(String, McpServerConfig)>,
    /// Selected MCP server names
    selected_mcps: Vec<String>,
    /// Whether MCP selection has been initialized from config
    mcp_selection_initialized: bool,
    /// Whether MCP selection has been overridden by user
    mcp_selection_overridden: bool,
    /// Whether we should recreate the session after MCP config changes
    pending_mcp_session_recreate: bool,
}

impl crate::panels::dock_panel::DockPanel for WelcomePanel {
    fn title() -> &'static str {
        "Welcome"
    }

    fn description() -> &'static str {
        "Welcome panel for creating new tasks"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> gpui::Pixels {
        px(0.)
    }
}

impl WelcomePanel {
    fn loading_agents_label() -> String {
        t!("welcome.agent.loading").to_string()
    }

    fn no_agents_label() -> String {
        t!("welcome.agent.none").to_string()
    }

    fn no_sessions_label() -> String {
        t!("welcome.session.none").to_string()
    }

    fn creating_session_label() -> String {
        t!("welcome.session.creating").to_string()
    }

    fn session_label(short_id: &str) -> String {
        t!("welcome.session.item", id = short_id).to_string()
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        Self::view_internal(None, window, cx)
    }

    /// Create a WelcomePanel for a specific workspace
    pub fn view_for_workspace(
        workspace_id: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::view_internal(Some(workspace_id), window, cx)
    }

    /// Get the workspace_id (if this panel is associated with a workspace)
    pub fn workspace_id(&self) -> Option<String> {
        self.workspace_id.clone()
    }

    /// Get the workspace_name (if available)
    pub fn workspace_name(&self) -> Option<String> {
        self.active_workspace_name.clone()
    }

    /// Get the working_directory
    pub fn working_directory(&self) -> std::path::PathBuf {
        self.working_directory.clone()
    }

    /// Create a WelcomePanel with specific workspace and working directory (for restoration from persistence)
    pub fn view_with_workspace_and_dir(
        workspace_id: Option<String>,
        working_directory: std::path::PathBuf,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::view_internal_with_dir(workspace_id, Some(working_directory), window, cx)
    }

    fn view_internal(
        workspace_id: Option<String>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::view_internal_with_dir(workspace_id, None, window, cx)
    }

    fn view_internal_with_dir(
        workspace_id: Option<String>,
        working_directory: Option<std::path::PathBuf>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(workspace_id.clone(), working_directory, window, cx));

        // Subscribe to CodeSelectionBus using the shared helper function
        crate::core::event_bus::subscribe_entity_to_code_selections(
            &entity,
            AppState::global(cx).code_selection_bus.clone(),
            "WelcomePanel",
            |panel, selection, cx| {
                panel.code_selections.push(selection);
                cx.notify();
            },
            cx,
        );

        // Subscribe to AgentConfigBus for dynamic agent list updates
        {
            let agent_config_bus = AppState::global(cx).agent_config_bus.clone();
            let weak_entity = entity.downgrade();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            // Subscribe to bus
            log::info!("[WelcomePanel] Subscribing to AgentConfigBus");
            agent_config_bus.subscribe(move |event| {
                log::debug!("[WelcomePanel] Received agent config event");
                let _ = tx.send(event.clone());
            });

            // Spawn background task to process events
            cx.spawn(async move |cx| {
                while let Some(event) = rx.recv().await {
                    if let Some(entity) = weak_entity.upgrade() {
                        _ = cx.update(|cx| {
                            entity.update(cx, |this, cx| {
                                this.on_agent_config_event(&event, cx);
                            });
                        });
                    } else {
                        break;
                    }
                }
            })
            .detach();
        }

        // Subscribe to agent_select focus to refresh agents list when no agents available
        entity.update(cx, |this, cx| {
            // Subscribe to input changes to detect @ symbol
            let input_subscription = cx.subscribe_in(
                &this.input_state,
                window,
                |this, _input, event: &gpui_component::input::InputEvent, _window, cx| match event {
                    gpui_component::input::InputEvent::Change => {
                        this.on_input_change(cx);
                    }
                    _ => {}
                },
            );
            this._subscriptions.push(input_subscription);

            let agent_select_focus = this.agent_select.focus_handle(cx);
            let subscription = cx.on_focus(
                &agent_select_focus,
                window,
                |this: &mut Self, window, cx| {
                    this.try_refresh_agents(window, cx);
                },
            );
            this._subscriptions.push(subscription);

            // Refresh sessions when agent selection changes
            let agent_select_sub = cx.subscribe_in(
                &this.agent_select,
                window,
                |this, _, _: &SelectEvent<Vec<AgentItem>>, window, cx| {
                    this.on_agent_changed(window, cx);
                },
            );
            this._subscriptions.push(agent_select_sub);

            // Subscribe to session_select changes to update welcome_session
            let session_select_sub = cx.subscribe_in(
                &this.session_select,
                window,
                |this, _, _: &SelectEvent<Vec<String>>, window, cx| {
                    this.on_session_changed(window, cx);
                },
            );
            this._subscriptions.push(session_select_sub);

            // Subscribe to mode_select changes to send SetSessionMode command to agent
            let mode_select_sub = cx.subscribe_in(
                &this.mode_select,
                window,
                |this, _, _: &SelectEvent<Vec<ModeSelectItem>>, _window, cx| {
                    this.on_mode_changed(cx);
                },
            );
            this._subscriptions.push(mode_select_sub);

            let model_select_sub = cx.subscribe_in(
                &this.model_select,
                window,
                |this, _, _: &SelectEvent<Vec<ModelSelectItem>>, _window, cx| {
                    this.on_model_changed(cx);
                },
            );
            this._subscriptions.push(model_select_sub);
        });

        // Load workspace info immediately and refresh on each panel creation
        Self::load_workspace_info(&entity, workspace_id.as_deref(), cx);

        entity
    }

    /// Load workspace info from WorkspaceService
    /// If workspace_id is provided, load that specific workspace
    /// Otherwise, load the active workspace
    fn load_workspace_info(entity: &Entity<Self>, workspace_id: Option<&str>, cx: &mut App) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("[WelcomePanel] WorkspaceService not available");
                return;
            }
        };

        log::info!(
            "[WelcomePanel] Loading workspace info (workspace_id: {:?})...",
            workspace_id
        );
        let workspace_id = workspace_id.map(|s| s.to_string());
        let weak_entity = entity.downgrade();
        cx.spawn(async move |cx| {
            // Get workspace - either specific or active
            let workspace = if let Some(ws_id) = workspace_id {
                workspace_service.get_workspace(&ws_id).await
            } else {
                workspace_service.get_active_workspace().await
            };

            log::info!(
                "[WelcomePanel] Loaded workspace: {:?}",
                workspace.as_ref().map(|ws| &ws.name)
            );

            // Update UI
            _ = cx.update(|cx| {
                if let Some(entity) = weak_entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.has_workspace = workspace.is_some();
                        this.active_workspace_name = workspace.map(|ws| ws.name);
                        log::info!(
                            "[WelcomePanel] Updated workspace name: {:?}",
                            this.active_workspace_name
                        );
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn new(workspace_id: Option<String>, working_directory: Option<std::path::PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("markdown")
                .multi_line(true)
                .auto_grow(2, 8) // Auto-grow from 2 to 8 rows
                .soft_wrap(true) // Enable word wrapping
                .placeholder(t!("welcome.input.placeholder").to_string())
        });

        // Get the working directory - use provided or get from AppState
        let working_dir = working_directory.unwrap_or_else(|| AppState::global(cx).current_working_dir().clone());

        let context_list = cx.new(|cx| {
            let delegate = FilePickerDelegate::new(&working_dir);
            ListState::new(delegate, window, cx).searchable(true)
        });

        let mode_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));

        let model_select =
            cx.new(|cx| SelectState::new(Vec::<ModelSelectItem>::new(), None, window, cx));

        // Get available agents from AppState - we'll load them asynchronously
        // For now, start with placeholder
        let agent_list = vec![AgentItem::new(Self::loading_agents_label())];
        let agent_select = cx.new(|cx| SelectState::new(agent_list, None, window, cx));

        let has_agents = false; // Will be updated after async load
        let first_agent: Option<String> = None;

        // Initialize session selector (initially empty)
        let session_select =
            cx.new(|cx| SelectState::new(vec![Self::no_sessions_label()], None, window, cx));

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            input_state,
            context_list,
            mode_select,
            model_select,
            agent_select,
            session_select,
            current_agent_name: None,
            current_session_id: None,
            has_agents,
            has_modes: false,
            has_models: false,
            is_session_loading: false,
            has_workspace: false,
            active_workspace_name: None,
            workspace_id,
            working_directory: working_dir,
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            selected_files: Vec::new(),
            file_suggestions: Vec::new(),
            command_suggestions: Vec::new(),
            show_command_suggestions: false,
            _subscriptions: Vec::new(),
            available_mcps: Vec::new(),
            selected_mcps: Vec::new(),
            mcp_selection_initialized: false,
            mcp_selection_overridden: false,
            pending_mcp_session_recreate: false,
        };

        // Load sessions for the initially selected agent if any
        if has_agents {
            if let Some(initial_agent) = first_agent {
                panel.refresh_sessions_for_agent(&initial_agent, None, window, cx);
            }
        }

        // Load MCP servers asynchronously
        panel.load_mcp_servers(cx);

        panel
    }

    /// Load MCP servers from AgentConfigService
    fn load_mcp_servers(&mut self, cx: &mut Context<Self>) {
        let agent_config_service = match AppState::global(cx).agent_config_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let weak_self = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let mcp_servers = agent_config_service.list_mcp_servers().await;

            _ = cx.update(|cx| {
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, |this, cx| {
                        // Directly use the HashMap as Vec of tuples
                        this.available_mcps = mcp_servers.into_iter().collect();
                        this.sync_mcp_selection_with_available();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn sync_mcp_selection_with_available(&mut self) {
        let enabled_mcps = self
            .available_mcps
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone())
            .collect::<HashSet<_>>();

        if self.mcp_selection_overridden {
            self.selected_mcps
                .retain(|name| enabled_mcps.contains(name));
        } else {
            self.selected_mcps = enabled_mcps.into_iter().collect();
        }

        self.mcp_selection_initialized = true;
        self.selected_mcps.sort();
    }

    fn on_mcp_servers_changed(&mut self) {
        self.available_mcps.sort_by(|a, b| a.0.cmp(&b.0));
        self.sync_mcp_selection_with_available();
        self.pending_mcp_session_recreate = self.current_agent_name.is_some();
    }

    fn upsert_mcp_server(&mut self, name: &str, config: &McpServerConfig) {
        if let Some(entry) = self
            .available_mcps
            .iter_mut()
            .find(|(server_name, _)| server_name == name)
        {
            *entry = (name.to_string(), config.clone());
        } else {
            self.available_mcps.push((name.to_string(), config.clone()));
        }

        self.on_mcp_servers_changed();
    }

    fn remove_mcp_server(&mut self, name: &str) {
        self.available_mcps
            .retain(|(server_name, _)| server_name != name);
        self.on_mcp_servers_changed();
    }

    fn collect_mcp_servers_from_selection(
        available_mcps: &[(String, McpServerConfig)],
        selected_mcps: &[String],
    ) -> Vec<acp::McpServer> {
        let selected_set: HashSet<&String> = selected_mcps.iter().collect();
        available_mcps
            .iter()
            .filter(|(name, config)| config.enabled && selected_set.contains(name))
            .map(|(_, config)| config.config.clone())
            .collect()
    }

    /// Try to refresh agents list from AppState if we don't have agents yet
    fn try_refresh_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_agents {
            return;
        }

        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let agent_select = self.agent_select.clone();
        let weak_self = cx.entity().downgrade();
        cx.spawn_in(window, async move |_this, window| {
            let agents = agent_service.list_agents().await;

            if agents.is_empty() {
                return;
            }

            _ = window.update(|window, cx| {
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, |this, cx| {
                        // We now have agents, update the select
                        this.has_agents = true;
                        let agent_items: Vec<AgentItem> = agents
                            .clone()
                            .into_iter()
                            .map(|name| AgentItem::new(name))
                            .collect();
                        agent_select.update(cx, |state, cx| {
                            state.set_items(agent_items, window, cx);
                            state.set_selected_index(Some(IndexPath::default()), window, cx);
                        });
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Handle agent configuration events (add/remove/reload)
    fn on_agent_config_event(
        &mut self,
        event: &crate::core::event_bus::agent_config_bus::AgentConfigEvent,
        cx: &mut Context<Self>,
    ) {
        use crate::core::event_bus::agent_config_bus::AgentConfigEvent;

        log::info!("[WelcomePanel] Received agent config event: {:?}", event);

        match event {
            AgentConfigEvent::AgentAdded { name, .. } => {
                log::info!("[WelcomePanel] Agent added: {}", name);
                // Force refresh to include new agent
                self.has_agents = false;
            }
            AgentConfigEvent::AgentRemoved { name } => {
                log::info!("[WelcomePanel] Agent removed: {}", name);
                // Force refresh to remove deleted agent
                self.has_agents = false;
            }
            AgentConfigEvent::AgentUpdated { name, .. } => {
                log::info!("[WelcomePanel] Agent updated: {}", name);
                // No action needed - agent name hasn't changed
            }
            AgentConfigEvent::ConfigReloaded { config } => {
                log::info!("[WelcomePanel] Agent config reloaded");
                // Force full refresh
                self.has_agents = false;
                self.available_mcps = config.mcp_servers.clone().into_iter().collect();
                self.on_mcp_servers_changed();
            }
            AgentConfigEvent::McpServerAdded { name, config } => {
                log::info!("[WelcomePanel] MCP server added: {}", name);
                self.upsert_mcp_server(name, config);
            }
            AgentConfigEvent::McpServerUpdated { name, config } => {
                log::info!("[WelcomePanel] MCP server updated: {}", name);
                self.upsert_mcp_server(name, config);
            }
            AgentConfigEvent::McpServerRemoved { name } => {
                log::info!("[WelcomePanel] MCP server removed: {}", name);
                self.remove_mcp_server(name);
            }
            // Model and Command events don't affect WelcomePanel
            AgentConfigEvent::ModelAdded { .. }
            | AgentConfigEvent::ModelUpdated { .. }
            | AgentConfigEvent::ModelRemoved { .. }
            | AgentConfigEvent::CommandAdded { .. }
            | AgentConfigEvent::CommandUpdated { .. }
            | AgentConfigEvent::CommandRemoved { .. } => {
                // No action needed for non-agent config changes
            }
        }

        cx.notify();
    }

    /// Handle agent selection change - refresh sessions for the newly selected agent
    fn on_agent_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let no_agents_label = Self::no_agents_label();
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != no_agents_label => name,
            _ => {
                // No valid agent selected, clear sessions
                self.session_select.update(cx, |state, cx| {
                    state.set_items(vec![Self::no_sessions_label()], window, cx);
                    state.set_selected_index(None, window, cx);
                });
                self.current_agent_name = None;
                self.current_session_id = None;
                AppState::global_mut(cx).clear_welcome_session();
                self.is_session_loading = false;
                self.sync_session_capabilities(None, window, cx);
                cx.notify();
                return;
            }
        };

        if self.current_agent_name.as_ref() == Some(&agent_name) {
            return;
        }

        self.current_agent_name = Some(agent_name.clone());
        self.begin_session_recreate(agent_name, window, cx);
    }

    fn on_mcp_selection_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let no_agents_label = Self::no_agents_label();
        let agent_name = self
            .current_agent_name
            .clone()
            .or_else(|| self.agent_select.read(cx).selected_value().cloned())
            .filter(|name| name != &no_agents_label);

        if let Some(agent_name) = agent_name {
            self.current_agent_name = Some(agent_name.clone());
            self.begin_session_recreate(agent_name, window, cx);
        }
    }

    /// Handle session selection change - update welcome_session
    fn on_session_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let no_agents_label = Self::no_agents_label();
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != no_agents_label => name,
            _ => return,
        };

        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        // Get the selected session index
        let selected_index = match self.session_select.read(cx).selected_index(cx) {
            Some(idx) => idx.row,
            None => return,
        };

        // Get all sessions for this agent
        let mut sessions = agent_service.list_sessions_for_agent(&agent_name);
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Get the selected session
        if let Some(selected_session) = sessions.get(selected_index) {
            self.current_session_id = Some(selected_session.session_id.clone());

            // Update welcome session
            AppState::global_mut(cx).set_welcome_session(WelcomeSession {
                session_id: selected_session.session_id.clone(),
                agent_name: agent_name.clone(),
            });

            log::info!(
                "[WelcomePanel] Session changed to: {} for agent: {}",
                selected_session.session_id,
                agent_name
            );

            self.sync_session_capabilities(Some(selected_session), window, cx);
            cx.notify();
        }
    }

    /// Handle mode selection change - send SetSessionMode command to agent
    fn on_mode_changed(&mut self, cx: &mut Context<Self>) {
        let no_agents_label = Self::no_agents_label();
        // Get the selected mode
        let mode = match self.mode_select.read(cx).selected_value() {
            Some(m) => m.clone(),
            None => return,
        };

        // Get the current session ID
        let session_id = match &self.current_session_id {
            Some(id) => id.clone(),
            None => {
                log::debug!("[WelcomePanel] Cannot change mode: no session selected");
                return;
            }
        };

        // Get the agent name
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != no_agents_label => name,
            _ => {
                log::debug!("[WelcomePanel] Cannot change mode: no agent selected");
                return;
            }
        };

        // Get the agent service to access agent manager
        let app_state = AppState::global(cx);
        let agent_manager = match app_state.agent_manager() {
            Some(manager) => manager.clone(),
            None => {
                log::error!("[WelcomePanel] Cannot change mode: agent manager not available");
                return;
            }
        };

        log::info!(
            "[WelcomePanel] Mode changed to: {} for session: {}",
            mode,
            session_id
        );

        // Send SetSessionMode command to agent asynchronously
        cx.spawn(async move |_entity, _cx| {
            // Get the agent handle
            let agent_handle = match agent_manager.get(&agent_name).await {
                Some(handle) => handle,
                None => {
                    log::error!("[WelcomePanel] Cannot change mode: agent '{}' not found", agent_name);
                    return;
                }
            };

            // Create the SetSessionModeRequest
            use agent_client_protocol as acp;
            let mut request = acp::SetSessionModeRequest::new(
                acp::SessionId::from(session_id.clone()),
                mode.clone(),
            );
            request.meta = None;

            // Send the request to the agent
            match agent_handle.set_session_mode(request).await {
                Ok(response) => {
                    log::info!(
                        "[WelcomePanel] Successfully set session mode to '{}' for session '{}': {:?}",
                        mode,
                        session_id,
                        response
                    );
                }
                Err(e) => {
                    log::error!(
                        "[WelcomePanel] Failed to set session mode to '{}' for session '{}': {}",
                        mode,
                        session_id,
                        e
                    );
                }
            }
        })
        .detach();
    }

    /// Handle model selection change - send SetSessionModel command to agent
    fn on_model_changed(&mut self, cx: &mut Context<Self>) {
        let no_agents_label = Self::no_agents_label();
        // Get the selected model ID
        let model_id = match self.model_select.read(cx).selected_value() {
            Some(model) => model.clone(),
            None => return,
        };

        // Get the current session ID
        let session_id = match &self.current_session_id {
            Some(id) => id.clone(),
            None => {
                log::debug!("[WelcomePanel] Cannot change model: no session selected");
                return;
            }
        };

        // Get the agent name
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != no_agents_label => name,
            _ => {
                log::debug!("[WelcomePanel] Cannot change model: no agent selected");
                return;
            }
        };

        // Get the agent service to access agent manager
        let app_state = AppState::global(cx);
        let agent_manager = match app_state.agent_manager() {
            Some(manager) => manager.clone(),
            None => {
                log::error!("[WelcomePanel] Cannot change model: agent manager not available");
                return;
            }
        };

        log::info!(
            "[WelcomePanel] Model changed to: {} for session: {}",
            model_id,
            session_id
        );

        // Send SetSessionModel command to agent asynchronously
        cx.spawn(async move |_entity, _cx| {
            let agent_handle = match agent_manager.get(&agent_name).await {
                Some(handle) => handle,
                None => {
                    log::error!(
                        "[WelcomePanel] Cannot change model: agent '{}' not found",
                        agent_name
                    );
                    return;
                }
            };

            use agent_client_protocol as acp;
            let mut request = acp::SetSessionModelRequest::new(
                acp::SessionId::from(session_id.clone()),
                model_id.clone(),
            );
            request.meta = None;

            match agent_handle.set_session_model(request).await {
                Ok(response) => {
                    log::info!(
                        "[WelcomePanel] Successfully set session model to '{}' for session '{}': {:?}",
                        model_id,
                        session_id,
                        response
                    );
                }
                Err(e) => {
                    log::error!(
                        "[WelcomePanel] Failed to set session model to '{}' for session '{}': {}",
                        model_id,
                        session_id,
                        e
                    );
                }
            }
        })
        .detach();
    }

    /// Handle input change - detect @ symbol to open file picker and / for commands
    fn on_input_change(&mut self, cx: &mut Context<Self>) {
        let value = self.input_state.read(cx).value();

        let mention_query = value.rfind('@').and_then(|at_index| {
            let query = &value[at_index + 1..];
            if query.chars().any(char::is_whitespace) {
                None
            } else {
                Some(query)
            }
        });

        if let Some(query) = mention_query {
            if self.show_command_suggestions {
                self.show_command_suggestions = false;
                self.command_suggestions.clear();
            }
            self.update_file_suggestions(query, cx);
            return;
        }

        self.clear_file_suggestions(cx);

        // Check if input starts with / for command suggestions
        if value.trim_start().starts_with('/') {
            // Get the command prefix (everything after the /)
            let trimmed = value.trim_start();
            let command_text = trimmed.trim_start_matches('/');
            if command_text.chars().any(char::is_whitespace) {
                if self.show_command_suggestions {
                    self.show_command_suggestions = false;
                    self.command_suggestions.clear();
                    cx.notify();
                }
                return;
            }
            let command_prefix = command_text;

            // Get available commands for the current session
            let all_commands = self.get_available_commands(cx);

            // Filter commands by prefix
            if command_prefix.is_empty() {
                // Show all commands when just "/" is entered
                self.command_suggestions = all_commands;
                self.show_command_suggestions = !self.command_suggestions.is_empty();
            } else {
                // Filter commands that start with the prefix
                self.command_suggestions = all_commands
                    .into_iter()
                    .filter(|cmd| cmd.name.starts_with(command_prefix))
                    .collect();
                self.show_command_suggestions = !self.command_suggestions.is_empty();
            }

            log::debug!(
                "[WelcomePanel] Command suggestions: {} matches for prefix '{}'",
                self.command_suggestions.len(),
                command_prefix
            );
            cx.notify();
        } else {
            // Not a command input, hide suggestions
            if self.show_command_suggestions {
                self.show_command_suggestions = false;
                self.command_suggestions.clear();
                cx.notify();
            }
        }
    }

    fn clear_file_suggestions(&mut self, cx: &mut Context<Self>) {
        if !self.file_suggestions.is_empty() {
            self.file_suggestions.clear();
            cx.notify();
        }
    }

    fn update_file_suggestions(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim();
        self.context_list.update(cx, |state, cx| {
            state.delegate_mut().set_search_query(query.to_string());
            cx.notify();
        });

        let items = self
            .context_list
            .read(cx)
            .delegate()
            .filtered_items()
            .iter()
            .take(MAX_FILE_SUGGESTIONS)
            .cloned()
            .collect::<Vec<_>>();

        self.file_suggestions = items;
        cx.notify();
    }

    fn apply_command_selection(
        &mut self,
        command: &AvailableCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = format!("/{} ", command.name);
        self.input_state.update(cx, |state, cx| {
            state.set_value(SharedString::from(value), window, cx);
        });
        self.show_command_suggestions = false;
        self.command_suggestions.clear();
        cx.notify();
    }

    /// Get available commands for the current session
    fn get_available_commands(&self, cx: &Context<Self>) -> Vec<AvailableCommand> {
        // Get the current session ID
        let session_id = match &self.current_session_id {
            Some(id) => id,
            None => {
                log::debug!("[WelcomePanel] No current session, cannot get commands");
                return Vec::new();
            }
        };

        // Get MessageService
        let message_service = match AppState::global(cx).message_service() {
            Some(service) => service,
            None => {
                log::warn!("[WelcomePanel] MessageService not available");
                return Vec::new();
            }
        };

        // Get commands for the session
        message_service
            .get_commands_by_session_id(session_id)
            .unwrap_or_default()
    }

    fn sync_session_capabilities(
        &mut self,
        session: Option<&AgentSessionInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.update_mode_select(session, window, cx);
        self.update_model_select(session, window, cx);
    }

    fn update_mode_select(
        &mut self,
        session: Option<&AgentSessionInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (mode_items, selected_mode_id) = session
            .and_then(|info| info.new_session_response.as_ref())
            .and_then(|response| response.modes.as_ref())
            .map(|modes| {
                let items = modes
                    .available_modes
                    .iter()
                    .map(|mode| ModeSelectItem::new(mode.id.to_string(), mode.name.clone()))
                    .collect::<Vec<_>>();
                (items, Some(modes.current_mode_id.to_string()))
            })
            .unwrap_or_else(|| (Vec::new(), None));

        let has_items = !mode_items.is_empty();
        self.has_modes = has_items;
        self.mode_select.update(cx, |state, cx| {
            state.set_items(mode_items, window, cx);
            if let Some(mode_id) = selected_mode_id {
                state.set_selected_value(&mode_id, window, cx);
            } else if has_items {
                state.set_selected_index(Some(IndexPath::default()), window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
    }

    fn update_model_select(
        &mut self,
        session: Option<&AgentSessionInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (model_items, selected_model_id) = session
            .and_then(|info| info.new_session_response.as_ref())
            .and_then(|response| response.models.as_ref())
            .map(|models| {
                let items = models
                    .available_models
                    .iter()
                    .map(|model| {
                        let label = if model.name.is_empty() {
                            model.model_id.to_string()
                        } else {
                            model.name.clone()
                        };
                        ModelSelectItem::new(model.model_id.to_string(), label)
                    })
                    .collect::<Vec<_>>();
                (items, Some(models.current_model_id.to_string()))
            })
            .unwrap_or_else(|| (Vec::new(), None));

        let has_models = !model_items.is_empty();
        self.has_models = has_models;
        self.model_select.update(cx, |state, cx| {
            state.set_items(model_items, window, cx);
            if let Some(model_id) = selected_model_id {
                state.set_selected_value(&model_id, window, cx);
            } else if has_models {
                state.set_selected_index(Some(IndexPath::default()), window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
    }

    /// Refresh sessions for the currently selected agent
    fn refresh_sessions_for_agent(
        &mut self,
        agent_name: &str,
        preferred_session_id: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let mut sessions = agent_service.list_sessions_for_agent(agent_name);
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        if sessions.is_empty() {
            // No sessions for this agent
            self.session_select.update(cx, |state, cx| {
                state.set_items(vec![Self::no_sessions_label()], window, cx);
                state.set_selected_index(None, window, cx);
            });
            self.current_session_id = None;
            self.is_session_loading = false;

            // Clear welcome session when no sessions available
            AppState::global_mut(cx).clear_welcome_session();
            self.sync_session_capabilities(None, window, cx);
        } else {
            // Display sessions (show first 8 chars of session ID)
            let session_display: Vec<String> = sessions
                .iter()
                .map(|s| {
                    let short_id = if s.session_id.len() > 8 {
                        &s.session_id[..8]
                    } else {
                        &s.session_id
                    };
                    Self::session_label(short_id)
                })
                .collect();

            let selected_index = preferred_session_id
                .and_then(|session_id| {
                    sessions
                        .iter()
                        .position(|session| session.session_id == session_id)
                })
                .unwrap_or(0);

            self.session_select.update(cx, |state, cx| {
                state.set_items(session_display, window, cx);
                state.set_selected_index(Some(IndexPath::new(selected_index)), window, cx);
            });

            // Set current session to the selected one
            if let Some(selected_session) = sessions.get(selected_index) {
                self.current_session_id = Some(selected_session.session_id.clone());
                self.is_session_loading = false;

                // Store as welcome session for CreateTaskFromWelcome action
                AppState::global_mut(cx).set_welcome_session(WelcomeSession {
                    session_id: selected_session.session_id.clone(),
                    agent_name: agent_name.to_string(),
                });

                self.sync_session_capabilities(Some(selected_session), window, cx);
            }
        }

        cx.notify();
    }

    fn begin_session_recreate(
        &mut self,
        agent_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_mcp_session_recreate = false;
        self.is_session_loading = true;
        self.current_session_id = None;
        AppState::global_mut(cx).clear_welcome_session();
        self.session_select.update(cx, |state, cx| {
            state.set_items(vec![Self::creating_session_label()], window, cx);
            state.set_selected_index(None, window, cx);
        });
        self.sync_session_capabilities(None, window, cx);
        cx.notify();

        self.create_session_for_agent(agent_name, window, cx);
    }

    fn create_session_for_agent(
        &mut self,
        agent_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let agent_config_service = AppState::global(cx).agent_config_service().cloned();
        let available_mcps = self.available_mcps.clone();
        let selected_mcps = self.selected_mcps.clone();
        let mcp_selection_initialized = self.mcp_selection_initialized;

        let weak_self = cx.entity().downgrade();
        let agent_name_for_session = agent_name.clone();
        cx.spawn_in(window, async move |_this, window| {
            let mut mcp_servers =
                Self::collect_mcp_servers_from_selection(&available_mcps, &selected_mcps);

            if !mcp_selection_initialized {
                if let Some(service) = agent_config_service {
                    let defaults = service.list_mcp_servers().await;
                    mcp_servers = defaults
                        .into_iter()
                        .filter(|(_, config)| config.enabled)
                        .map(|(_, config)| config.config)
                        .collect();
                }
            }

            match agent_service
                .create_session_with_mcp(&agent_name_for_session, mcp_servers)
                .await
            {
                Ok(session_id) => {
                    log::info!("[WelcomePanel] Created new session: {}", session_id);
                    _ = window.update(|window, cx| {
                        if let Some(this) = weak_self.upgrade() {
                            this.update(cx, |this, cx| {
                                this.current_session_id = Some(session_id.clone());
                                this.refresh_sessions_for_agent(
                                    &agent_name_for_session,
                                    Some(&session_id),
                                    window,
                                    cx,
                                );
                                this.apply_selected_mode_to_session(cx);
                                this.apply_selected_model_to_session(cx);
                            });
                        }
                    });
                }
                Err(e) => {
                    log::error!("[WelcomePanel] Failed to create session: {}", e);
                    _ = window.update(|_window, cx| {
                        if let Some(this) = weak_self.upgrade() {
                            this.update(cx, |this, cx| {
                                this.is_session_loading = false;
                                cx.notify();
                            });
                        }
                    });
                }
            }
        })
        .detach();
    }

    fn apply_selected_mode_to_session(&mut self, cx: &mut Context<Self>) {
        self.on_mode_changed(cx);
    }

    fn apply_selected_model_to_session(&mut self, cx: &mut Context<Self>) {
        self.on_model_changed(cx);
    }

    /// Create a new session for the currently selected agent
    fn create_new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let no_agents_label = Self::no_agents_label();
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != no_agents_label => name,
            _ => return,
        };

        self.current_agent_name = Some(agent_name.clone());
        self.begin_session_recreate(agent_name, window, cx);
    }

    /// Handles sending the task based on the current input, mode, and agent selections.
    fn handle_send_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Check if workspace exists
        if !self.has_workspace {
            log::warn!("[WelcomePanel] Cannot create task: No workspace available");
            // TODO: Show user-facing notification/toast
            return;
        }

        let task_name = self.input_state.read(cx).text().to_string();

        if !task_name.is_empty() {
            let mode = self
                .mode_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_else(|| "default".to_string());

            let agent_name = self
                .agent_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_else(|| "test-agent".to_string());

            let agent_name = if agent_name == Self::no_agents_label() {
                "test-agent".to_string()
            } else {
                agent_name
            };

            // Clear the input immediately
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });

            // Dispatch CreateTaskFromWelcome action with images
            let images = std::mem::take(&mut self.pasted_images);
            let action = CreateTaskFromWelcome {
                task_input: task_name,
                agent_name,
                mode,
                images,
            };

            window.dispatch_action(Box::new(action), cx);

            // Clear pasted images and code selections after dispatching action
            self.code_selections.clear();
        }
    }
}

impl Focusable for WelcomePanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl WelcomePanel {
    /// Handle paste event and add images to pasted_images list
    /// Returns true if we handled the paste (had images), false otherwise
    fn handle_paste(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        log::info!("Handling paste in WelcomePanel");

        let mut handled = false;
        if let Some(clipboard_item) = cx.read_from_clipboard() {
            for entry in clipboard_item.entries().iter() {
                if let ClipboardEntry::Image(image) = entry {
                    log::info!("Processing pasted image: {:?}", image.format);
                    let image = image.clone();
                    handled = true;

                    cx.spawn_in(window, async move |this, cx| {
                        match crate::utils::clipboard::image_to_content(image).await {
                            Ok((image_content, filename)) => {
                                _ = cx.update(move |_window, cx| {
                                    let _ = this.update(cx, |this, cx| {
                                        this.pasted_images.push((image_content, filename));
                                        cx.notify();
                                    });
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to process pasted image: {}", e);
                            }
                        }
                    })
                    .detach();
                }
            }
        }
        handled
    }
}

impl Render for WelcomePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // log::debug!(
        //     "[WelcomePanel::render] Rendering with {} code_selections and {} pasted_images",
        //     self.code_selections.len(),
        //     self.pasted_images.len()
        // );
        if self.pending_mcp_session_recreate {
            self.pending_mcp_session_recreate = false;
            if let Some(agent_name) = self.current_agent_name.clone() {
                self.begin_session_recreate(agent_name, window, cx);
            }
        }

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .track_focus(&self.focus_handle)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(800.)) // Maximum width for better readability
                    .gap_4()
                    .child(
                        // Welcome title and subtitle
                        v_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .px(px(32.))
                            .child(
                                gpui::div()
                                    .text_2xl()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(t!("welcome.title").to_string()),
                            )
                            .child(
                                gpui::div()
                                    .text_base()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if self.has_workspace {
                                        if let Some(workspace_name) = &self.active_workspace_name {
                                            t!(
                                                "welcome.subtitle.current_workspace",
                                                workspace = workspace_name
                                            )
                                            .to_string()
                                        } else {
                                            t!("welcome.subtitle.start").to_string()
                                        }
                                    } else {
                                        t!("welcome.subtitle.no_workspace").to_string()
                                    }),
                            ),
                    )
                    .child(
                        // Chat input with title and send handler
                        {
                            let entity = cx.entity().clone();
                            let mut chat =
                                ChatInputBox::new("welcome-chat-input", self.input_state.clone());
                            if self.current_session_id.is_some() {
                                if self.has_modes {
                                    chat = chat.mode_select(self.mode_select.clone());
                                }
                                if self.has_models {
                                    chat = chat.model_select(self.model_select.clone());
                                }
                            }
                            if self.is_session_loading {
                                chat = chat.agent_status_text(t!("welcome.loading").to_string());
                            }

                            // log::debug!(
                            //     "[WelcomePanel::render] Creating ChatInputBox with {} code_selections",
                            //     self.code_selections.len()
                            // );
                            chat
                                // .title("New Task")
                                .agent_select(self.agent_select.clone())
                                .pasted_images(self.pasted_images.clone())
                                .code_selections(self.code_selections.clone())
                                .file_suggestions(self.file_suggestions.clone())
                                .on_file_select(cx.listener(|this, file: &FileItem, window, cx| {
                                    let file_path = file.path.to_string_lossy().to_string();
                                    let mut filename = file.relative_path.clone();
                                    if file.is_folder && !filename.ends_with('/') {
                                        filename.push('/');
                                    }

                                    let input_state = this.input_state.clone();
                                    let current_value = input_state.read(cx).value();
                                    let mut applied_to_input = false;
                                    if let Some(at_index) = current_value.rfind('@') {
                                        let query = &current_value[at_index + 1..];
                                        if !query.chars().any(char::is_whitespace) {
                                            let prefix = &current_value[..at_index];
                                            let new_value =
                                                SharedString::from(format!("{prefix}@{filename} "));
                                            window.defer(cx, move |window, cx| {
                                                input_state.update(cx, |state, cx| {
                                                    state.set_value(new_value, window, cx);
                                                });
                                            });
                                            applied_to_input = true;
                                        }
                                    }

                                    if !applied_to_input
                                        && !file.is_folder
                                        && !this.selected_files.contains(&file_path)
                                    {
                                        this.selected_files.push(file_path);
                                    }

                                    this.file_suggestions.clear();
                                    cx.notify();
                                }))
                                // Pass command suggestions to ChatInputBox
                                .command_suggestions(self.command_suggestions.clone())
                                .show_command_suggestions(self.show_command_suggestions)
                                .on_command_select(cx.listener(|this, command, window, cx| {
                                    this.apply_command_selection(command, window, cx);
                                }))
                                // Pass MCP servers and selection to ChatInputBox
                                .available_mcps(self.available_mcps.clone())
                                .selected_mcps(self.selected_mcps.clone())
                                .on_mcp_toggle(cx.listener(
                                    |this, (name, checked): &(String, bool), window, cx| {
                                        // Simple toggle logic
                                        if *checked {
                                            if !this.selected_mcps.contains(name) {
                                                this.selected_mcps.push(name.clone());
                                            }
                                        } else {
                                            this.selected_mcps.retain(|s| s != name);
                                        }
                                        this.mcp_selection_overridden = true;
                                        log::info!(
                                            "[WelcomePanel] MCP '{}' {}",
                                            name,
                                            if *checked { "selected" } else { "deselected" }
                                        );
                                        this.on_mcp_selection_changed(window, cx);
                                        cx.notify();
                                    },
                                ))
                                .on_paste(move |window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.handle_paste(window, cx);
                                    });
                                })
                                .on_remove_image(cx.listener(|this, idx, _, cx| {
                                    // Remove the image at the given index
                                    if *idx < this.pasted_images.len() {
                                        this.pasted_images.remove(*idx);
                                        cx.notify();
                                    }
                                }))
                                .on_remove_code_selection(cx.listener(|this, idx, _, cx| {
                                    // Remove the code selection at the given index
                                    if *idx < this.code_selections.len() {
                                        this.code_selections.remove(*idx);
                                        cx.notify();
                                    }
                                }))
                                .selected_files(self.selected_files.clone())
                                .on_remove_file(cx.listener(|this, idx, _, cx| {
                                    // Remove the file at the given index
                                    if *idx < this.selected_files.len() {
                                        this.selected_files.remove(*idx);
                                        cx.notify();
                                    }
                                }))
                                .on_send(cx.listener(|this, _, window, cx| {
                                    this.handle_send_task(window, cx);
                                }))
                        },
                    ),
            )
    }
}
