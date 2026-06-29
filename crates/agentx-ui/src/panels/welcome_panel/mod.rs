//! Dock-hosted welcome panel.
//!
//! This is the migrated "new task" surface from the legacy workspace. It owns
//! the task composer, starts an agent session for the selected agent/MCP set,
//! creates the workspace task, then hands the live session to the dock workspace.

mod view;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use gpui::*;
use gpui_component::{
    IndexPath, WindowExt as _,
    input::{InputEvent, InputState},
    notification::Notification,
    select::{SelectEvent, SelectState},
};

use agentx_app::{ConfigService, FileService, SessionService, WorkspaceService};
use agentx_bus::{EventBus, Receiver};
use agentx_domain::{
    AgentId, AgentRegistry, AgentStatus, Config, ContentBlock, DomainEvent, McpServerConfig,
    SessionConfigOption, SessionId, SessionInit, SessionMode, SlashCommand, Task, TaskId,
    Workspace, WorkspaceId,
};

use crate::chat::{active_mention, format_code_selection_as_context, image_attachment};
use crate::components::{
    AgentItem, CodeSelection, FileItem, ImageAttachment, ModeSelectItem, ModelSelectItem,
};

const NO_AGENTS_LABEL: &str = "No agents configured";
const MAX_FILE_SUGGESTIONS: usize = 8;

pub(crate) struct WelcomeLaunch {
    pub agent: AgentId,
    pub cwd: PathBuf,
    pub init: SessionInit,
    pub events: Receiver<DomainEvent>,
    pub initial_content: Option<Vec<ContentBlock>>,
}

type LaunchHandler = Arc<dyn Fn(WelcomeLaunch, &mut Window, &mut App) + Send + Sync + 'static>;

pub struct WelcomePanel {
    registry: Arc<dyn AgentRegistry>,
    service: Arc<SessionService>,
    workspace_service: Arc<WorkspaceService>,
    config_service: Arc<ConfigService>,
    file_service: Arc<FileService>,
    bus: EventBus,
    config: Config,
    cwd: PathBuf,
    workspace: Option<Workspace>,
    on_launch: LaunchHandler,
    agents: Vec<AgentId>,
    selected_agent: Option<AgentId>,
    current_session_id: Option<SessionId>,
    current_init: Option<SessionInit>,
    config_options: Vec<SessionConfigOption>,
    modes: Vec<SessionMode>,
    current_mode: Option<String>,
    commands: Vec<SlashCommand>,
    input: Entity<InputState>,
    agent_select: Entity<SelectState<Vec<AgentItem>>>,
    mode_select: Entity<SelectState<Vec<ModeSelectItem>>>,
    model_select: Entity<SelectState<Vec<ModelSelectItem>>>,
    file_suggestions: Vec<FileItem>,
    selected_files: Vec<String>,
    pasted_images: Vec<ImageAttachment>,
    code_selections: Vec<CodeSelection>,
    command_suggestions: Vec<SlashCommand>,
    show_command_suggestions: bool,
    available_mcps: Vec<(String, McpServerConfig)>,
    selected_mcps: Vec<String>,
    mcp_selection_overridden: bool,
    session_loading: bool,
    status: Option<SharedString>,
    config_reload_pending: bool,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl WelcomePanel {
    #[allow(clippy::too_many_arguments)] // Composition root supplies all app ports.
    pub(crate) fn new(
        registry: Arc<dyn AgentRegistry>,
        service: Arc<SessionService>,
        workspace_service: Arc<WorkspaceService>,
        config_service: Arc<ConfigService>,
        file_service: Arc<FileService>,
        bus: EventBus,
        config: Config,
        cwd: PathBuf,
        on_launch: LaunchHandler,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut agents: Vec<AgentId> = config
            .agents
            .keys()
            .map(|name| AgentId::from(name.clone()))
            .collect();
        agents.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let selected_agent = agents.first().cloned();
        let agent_items = if agents.is_empty() {
            vec![AgentItem::new(NO_AGENTS_LABEL)]
        } else {
            agents
                .iter()
                .map(|agent| AgentItem::new(agent.to_string()))
                .collect()
        };

        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("markdown")
                .multi_line(true)
                .auto_grow(2, 8)
                .soft_wrap(true)
                .placeholder("Ask AgentX to work on a task...")
        });
        let agent_select =
            cx.new(|cx| SelectState::new(agent_items, Some(IndexPath::new(0)), window, cx));
        let mode_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));
        let model_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));

        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &input,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => this.handle_send(window, cx),
                InputEvent::Change => this.on_input_change(cx),
                _ => {}
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &agent_select,
            window,
            |this, _, _: &SelectEvent<Vec<AgentItem>>, window, cx| {
                this.on_agent_changed(window, cx);
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &mode_select,
            window,
            |this, _, _: &SelectEvent<Vec<ModeSelectItem>>, window, cx| {
                this.on_mode_changed(window, cx);
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &model_select,
            window,
            |this, _, _: &SelectEvent<Vec<ModelSelectItem>>, window, cx| {
                this.on_model_changed(window, cx);
            },
        ));

        let available_mcps = Self::sorted_mcp_servers(&config);
        let mut panel = Self {
            registry,
            service,
            workspace_service,
            config_service,
            file_service,
            bus,
            config,
            cwd,
            workspace: None,
            on_launch,
            agents,
            selected_agent,
            current_session_id: None,
            current_init: None,
            config_options: Vec::new(),
            modes: Vec::new(),
            current_mode: None,
            commands: Vec::new(),
            input,
            agent_select,
            mode_select,
            model_select,
            file_suggestions: Vec::new(),
            selected_files: Vec::new(),
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            command_suggestions: Vec::new(),
            show_command_suggestions: false,
            available_mcps,
            selected_mcps: Vec::new(),
            mcp_selection_overridden: false,
            session_loading: false,
            status: None,
            config_reload_pending: false,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        panel.sync_mcp_selection_with_available();
        panel.subscribe_domain_events(cx);
        panel.ensure_workspace(cx);
        if let Some(agent) = panel.selected_agent.clone() {
            panel.begin_session_recreate(agent, window, cx);
        }
        panel
    }

    fn subscribe_domain_events(&self, cx: &mut Context<Self>) {
        let mut events = self.bus.subscribe::<DomainEvent>();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                let _ = cx.update(|cx| {
                    if let Some(panel) = this.upgrade() {
                        panel.update(cx, |this, cx| {
                            this.record(event, cx);
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn sorted_mcp_servers(config: &Config) -> Vec<(String, McpServerConfig)> {
        let mut mcps: Vec<_> = config.mcp_servers.clone().into_iter().collect();
        mcps.sort_by(|a, b| a.0.cmp(&b.0));
        mcps
    }

    fn sync_mcp_selection_with_available(&mut self) {
        let enabled = self
            .available_mcps
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone())
            .collect::<HashSet<_>>();

        if self.mcp_selection_overridden {
            self.selected_mcps.retain(|name| enabled.contains(name));
        } else {
            self.selected_mcps = enabled.into_iter().collect();
        }
        self.selected_mcps.sort();
    }

    fn record(&mut self, event: DomainEvent, cx: &mut Context<Self>) {
        match event {
            DomainEvent::SessionCommandsChanged { session, commands }
                if self.current_session_id.as_ref() == Some(&session) =>
            {
                self.commands = commands;
                self.on_input_change(cx);
            }
            DomainEvent::SessionConfigChanged { session, options }
                if self.current_session_id.as_ref() == Some(&session) =>
            {
                self.config_options = options;
                cx.notify();
            }
            DomainEvent::ConfigChanged => {
                self.config_reload_pending = true;
                cx.notify();
            }
            DomainEvent::WorkspaceAdded { workspace } => {
                if same_path(&workspace.path, &self.cwd) {
                    self.workspace = Some(workspace);
                    cx.notify();
                }
            }
            DomainEvent::WorkspaceRemoved { workspace } => {
                if self.workspace.as_ref().is_some_and(|ws| ws.id == workspace) {
                    self.workspace = None;
                    self.ensure_workspace(cx);
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    fn ensure_workspace(&self, cx: &mut Context<Self>) {
        let service = self.workspace_service.clone();
        let cwd = self.cwd.clone();
        cx.spawn(async move |this, cx| {
            let groups = service.workspaces_with_tasks().await.unwrap_or_default();
            let existing = groups
                .into_iter()
                .map(|(workspace, _)| workspace)
                .find(|workspace| same_path(&workspace.path, &cwd));
            let workspace = match existing {
                Some(workspace) => Some(workspace),
                None => service.add_workspace(cwd).await.ok(),
            };
            let _ = cx.update(|cx| {
                if let Some(panel) = this.upgrade() {
                    panel.update(cx, |this, cx| {
                        this.workspace = workspace;
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn reload_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config_reload_pending = false;
        let service = self.config_service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let config = service.load().await.unwrap_or_default();
            let _ = this.update_in(cx, |this, window, cx| {
                this.apply_config(config, window, cx);
            });
        })
        .detach();
    }

    fn apply_config(&mut self, config: Config, window: &mut Window, cx: &mut Context<Self>) {
        self.config = config;
        self.agents = self
            .config
            .agents
            .keys()
            .map(|name| AgentId::from(name.clone()))
            .collect();
        self.agents.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        self.available_mcps = Self::sorted_mcp_servers(&self.config);
        self.sync_mcp_selection_with_available();
        self.update_agent_select(window, cx);

        if let Some(agent) = self.selected_agent.clone()
            && !self.config.agents.contains_key(agent.as_str())
        {
            self.selected_agent = self.agents.first().cloned();
            self.current_session_id = None;
            self.current_init = None;
        }
        if self.current_session_id.is_none()
            && let Some(agent) = self.selected_agent.clone()
        {
            self.begin_session_recreate(agent, window, cx);
        }
        cx.notify();
    }

    fn update_agent_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = if self.agents.is_empty() {
            vec![AgentItem::new(NO_AGENTS_LABEL)]
        } else {
            self.agents
                .iter()
                .map(|agent| AgentItem::new(agent.to_string()))
                .collect()
        };
        let selected = self
            .selected_agent
            .as_ref()
            .and_then(|agent| self.agents.iter().position(|candidate| candidate == agent))
            .or_else(|| (!self.agents.is_empty()).then_some(0))
            .unwrap_or(0);
        self.agent_select.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_index(Some(IndexPath::new(selected)), window, cx);
        });
    }

    fn on_agent_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.agent_select.read(cx).selected_value().cloned() else {
            return;
        };
        if name == NO_AGENTS_LABEL {
            self.selected_agent = None;
            self.current_session_id = None;
            self.current_init = None;
            self.status = Some("Configure an agent in settings first.".into());
            cx.notify();
            return;
        }

        let agent = AgentId::from(name);
        if self.selected_agent.as_ref() == Some(&agent) && self.current_session_id.is_some() {
            return;
        }
        self.selected_agent = Some(agent.clone());
        self.begin_session_recreate(agent, window, cx);
    }

    fn on_mcp_selection_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(agent) = self.selected_agent.clone() {
            self.begin_session_recreate(agent, window, cx);
        }
    }

    fn begin_session_recreate(
        &mut self,
        agent: AgentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(agent_config) = self.config.agents.get(agent.as_str()).cloned() else {
            self.status = Some(format!("Agent `{agent}` is not configured.").into());
            cx.notify();
            return;
        };

        self.session_loading = true;
        self.status = None;
        self.current_session_id = None;
        self.current_init = None;
        self.config_options.clear();
        self.modes.clear();
        self.current_mode = None;
        self.commands.clear();
        self.clear_selectors(window, cx);
        cx.notify();

        let registry = self.registry.clone();
        let service = self.service.clone();
        let proxy = self.config.proxy.clone();
        let cwd = self.cwd.clone();
        let selected_mcps = self.collect_selected_mcps();

        cx.spawn_in(window, async move |this, cx| {
            let running = registry.agents().into_iter().any(|descriptor| {
                descriptor.id == agent
                    && !matches!(descriptor.status, AgentStatus::Unavailable { .. })
            });
            let started = if running {
                Ok(())
            } else {
                let _ = registry.set_proxy(proxy).await;
                registry.add_agent(agent.clone(), agent_config).await
            };
            let result = match started {
                Ok(()) => service.new_session(&agent, &cwd, &selected_mcps).await,
                Err(error) => Err(error),
            };

            let _ = this.update_in(cx, |this, window, cx| {
                if this.selected_agent.as_ref() != Some(&agent) {
                    return;
                }
                this.session_loading = false;
                match result {
                    Ok(init) => {
                        this.current_session_id = Some(init.session_id.clone());
                        this.current_init = Some(init.clone());
                        this.apply_session_init(init, window, cx);
                    }
                    Err(error) => {
                        let text: SharedString =
                            format!("Failed to create session: {error}").into();
                        this.status = Some(text.clone());
                        window.push_notification(Notification::error(text), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn collect_selected_mcps(&self) -> Vec<McpServerConfig> {
        let selected = self.selected_mcps.iter().collect::<HashSet<_>>();
        self.available_mcps
            .iter()
            .filter(|(name, config)| config.enabled && selected.contains(name))
            .map(|(name, config)| {
                let mut config = config.clone();
                config.name = name.clone();
                config
            })
            .collect()
    }

    fn clear_selectors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mode_select.update(cx, |state, cx| {
            state.set_items(Vec::new(), window, cx);
            state.set_selected_index(None, window, cx);
        });
        self.model_select.update(cx, |state, cx| {
            state.set_items(Vec::new(), window, cx);
            state.set_selected_index(None, window, cx);
        });
    }

    fn apply_session_init(
        &mut self,
        init: SessionInit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.config_options = init.config_options;
        self.modes = init.modes;
        self.current_mode = init.current_mode;
        self.commands = init.commands;
        self.update_mode_select(window, cx);
        self.update_model_select(window, cx);
    }

    fn update_mode_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode_config = self.mode_config_option().cloned();
        let (items, selected_value) = if let Some(option) = mode_config {
            (
                option
                    .values
                    .iter()
                    .map(|value| ModeSelectItem::new(value.value.clone(), value.name.clone()))
                    .collect::<Vec<_>>(),
                Some(option.current_value),
            )
        } else {
            (
                self.modes
                    .iter()
                    .map(|mode| ModeSelectItem::new(mode.id.clone(), mode.name.clone()))
                    .collect::<Vec<_>>(),
                self.current_mode.clone(),
            )
        };
        let selected = selected_value
            .as_ref()
            .and_then(|value| items.iter().position(|item| item.id == *value))
            .or_else(|| (!items.is_empty()).then_some(0));
        self.mode_select.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_index(selected.map(IndexPath::new), window, cx);
        });
    }

    fn update_model_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(option) = self.model_config_option().cloned() else {
            self.model_select.update(cx, |state, cx| {
                state.set_items(Vec::new(), window, cx);
                state.set_selected_index(None, window, cx);
            });
            return;
        };
        let items = option
            .values
            .iter()
            .map(|value| ModelSelectItem::new(value.value.clone(), value.name.clone()))
            .collect::<Vec<_>>();
        let selected = items
            .iter()
            .position(|item| item.id == option.current_value)
            .or_else(|| (!items.is_empty()).then_some(0));
        self.model_select.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_index(selected.map(IndexPath::new), window, cx);
        });
    }

    fn mode_config_option(&self) -> Option<&SessionConfigOption> {
        self.config_options
            .iter()
            .find(|option| option.category.as_deref() == Some("mode") || option.id == "mode")
    }

    fn model_config_option(&self) -> Option<&SessionConfigOption> {
        self.config_options
            .iter()
            .find(|option| option.category.as_deref() == Some("model") || option.id == "model")
    }

    fn on_mode_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(value) = self.mode_select.read(cx).selected_value().cloned() else {
            return;
        };
        let Some(session) = self.current_session_id.clone() else {
            return;
        };
        if let Some(config_id) = self.mode_config_option().map(|option| option.id.clone()) {
            self.set_config_option(session, config_id, value, window, cx);
        } else {
            self.current_mode = Some(value.clone());
            let service = self.service.clone();
            cx.spawn_in(window, async move |this, cx| {
                let result = service.set_mode(&session, &value).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if let Err(error) = result {
                        this.report_error(format!("Failed to set mode: {error}"), window, cx);
                    }
                });
            })
            .detach();
        }
    }

    fn on_model_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(value) = self.model_select.read(cx).selected_value().cloned() else {
            return;
        };
        let Some(session) = self.current_session_id.clone() else {
            return;
        };
        let Some(config_id) = self.model_config_option().map(|option| option.id.clone()) else {
            return;
        };
        self.set_config_option(session, config_id, value, window, cx);
    }

    fn set_config_option(
        &mut self,
        session: SessionId,
        config_id: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(option) = self
            .config_options
            .iter_mut()
            .find(|option| option.id == config_id)
        {
            option.current_value = value.clone();
        }
        let service = self.service.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = service
                .set_config_option(&session, &config_id, &value)
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Err(error) = result {
                    this.report_error(format!("Failed to set option: {error}"), window, cx);
                }
            });
        })
        .detach();
    }

    fn on_input_change(&mut self, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        if let Some((_, query)) = active_mention(&value) {
            self.show_command_suggestions = false;
            self.command_suggestions.clear();
            self.update_file_suggestions(query, cx);
            return;
        }
        if !self.file_suggestions.is_empty() {
            self.file_suggestions.clear();
        }

        let trimmed = value.trim_start();
        if let Some(command_text) = trimmed.strip_prefix('/') {
            if command_text.chars().any(char::is_whitespace) {
                self.show_command_suggestions = false;
                self.command_suggestions.clear();
                cx.notify();
                return;
            }
            self.command_suggestions = self
                .commands
                .iter()
                .filter(|command| command.name.starts_with(command_text))
                .cloned()
                .collect();
            self.show_command_suggestions = !self.command_suggestions.is_empty();
            cx.notify();
        } else if self.show_command_suggestions || !self.command_suggestions.is_empty() {
            self.show_command_suggestions = false;
            self.command_suggestions.clear();
            cx.notify();
        }
    }

    fn update_file_suggestions(&mut self, query: String, cx: &mut Context<Self>) {
        let service = self.file_service.clone();
        let cwd = self.cwd.clone();
        cx.spawn(async move |this, cx| {
            let entries = service
                .list_files(&cwd, query.trim())
                .await
                .unwrap_or_default();
            let _ = cx.update(|cx| {
                if let Some(panel) = this.upgrade() {
                    panel.update(cx, |this, cx| {
                        let current = this.input.read(cx).value().to_string();
                        if active_mention(&current).is_none() {
                            this.file_suggestions.clear();
                            cx.notify();
                            return;
                        }
                        this.file_suggestions = entries
                            .into_iter()
                            .take(MAX_FILE_SUGGESTIONS)
                            .map(|entry| {
                                FileItem::new(entry.name, entry.relative_path, entry.is_dir)
                            })
                            .collect();
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn apply_file_mention(&mut self, file: FileItem, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        if let Some((at, _)) = active_mention(&value) {
            let mut path = file.relative_path.clone();
            if file.is_folder && !path.ends_with('/') {
                path.push('/');
            }
            let new_value = format!("{}@{} ", &value[..at], path);
            self.input
                .update(cx, |state, cx| state.set_value(new_value, window, cx));
        }
        if !file.is_folder && !self.selected_files.contains(&file.relative_path) {
            self.selected_files.push(file.relative_path);
        }
        self.file_suggestions.clear();
        cx.notify();
    }

    fn apply_command_selection(
        &mut self,
        command: SlashCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input.update(cx, |state, cx| {
            state.set_value(format!("/{} ", command.name), window, cx);
        });
        self.show_command_suggestions = false;
        self.command_suggestions.clear();
        cx.notify();
    }

    fn remove_image(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.pasted_images.len() {
            self.pasted_images.remove(index);
            cx.notify();
        }
    }

    fn remove_code_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.code_selections.len() {
            self.code_selections.remove(index);
            cx.notify();
        }
    }

    fn remove_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.selected_files.len() {
            self.selected_files.remove(index);
            cx.notify();
        }
    }

    fn handle_paste(&mut self, cx: &mut Context<Self>) {
        let Some(clipboard_item) = cx.read_from_clipboard() else {
            return;
        };
        let mut changed = false;
        for entry in clipboard_item.entries() {
            if let ClipboardEntry::Image(image) = entry {
                self.pasted_images.push(image_attachment(image.clone()));
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn handle_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session_loading {
            return;
        }
        let Some(agent) = self.selected_agent.clone() else {
            self.status = Some("Select an agent first.".into());
            cx.notify();
            return;
        };
        let Some(session) = self.current_session_id.clone() else {
            self.begin_session_recreate(agent, window, cx);
            return;
        };
        let Some(init) = self.current_init.clone() else {
            return;
        };
        let Some(workspace) = self.workspace.clone() else {
            self.ensure_workspace(cx);
            self.status = Some("Preparing workspace...".into());
            cx.notify();
            return;
        };

        let text = self.input.read(cx).value().to_string();
        let has_content = !text.trim().is_empty()
            || !self.pasted_images.is_empty()
            || !self.code_selections.is_empty()
            || !self.selected_files.is_empty();
        if !has_content {
            return;
        }

        self.input
            .update(cx, |state, cx| state.set_value("", window, cx));
        let content = self.take_prompt_content(text);
        self.file_suggestions.clear();
        self.command_suggestions.clear();
        self.show_command_suggestions = false;

        let task_name = task_name_from_content(&content).unwrap_or_else(|| "New task".to_string());
        let mode = self.current_mode_name(cx);
        let mut task = Task::new(
            TaskId::generate(),
            workspace.id.clone(),
            task_name,
            agent.clone(),
            mode,
            Utc::now(),
        );
        task.bind_session(session);

        let events = self.bus.subscribe::<DomainEvent>();
        let launch = WelcomeLaunch {
            agent,
            cwd: workspace.path,
            init,
            events,
            initial_content: Some(content),
        };
        let workspace_service = self.workspace_service.clone();
        let on_launch = self.on_launch.clone();
        self.session_loading = true;
        self.status = Some("Starting task...".into());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = workspace_service.add_task(task).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.session_loading = false;
                match result {
                    Ok(()) => {
                        this.status = None;
                        on_launch(launch, window, cx);
                    }
                    Err(error) => {
                        this.report_error(format!("Failed to create task: {error}"), window, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn take_prompt_content(&mut self, text: String) -> Vec<ContentBlock> {
        let mut content = Vec::new();
        for selection in self.code_selections.drain(..) {
            content.push(ContentBlock::text(format_code_selection_as_context(
                &selection,
            )));
        }
        if !text.trim().is_empty() {
            content.push(ContentBlock::text(text));
        }
        for image in self.pasted_images.drain(..) {
            content.push(ContentBlock::Image {
                mime_type: image.mime_type,
                data: image.data,
            });
        }
        for path in self.selected_files.drain(..) {
            let name = std::path::Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
                .unwrap_or_else(|| path.clone());
            content.push(ContentBlock::ResourceLink {
                name,
                uri: path,
                mime_type: None,
            });
        }
        content
    }

    fn current_mode_name(&self, cx: &Context<Self>) -> String {
        self.mode_select
            .read(cx)
            .selected_value()
            .cloned()
            .or_else(|| self.current_mode.clone())
            .unwrap_or_else(|| "default".to_string())
    }

    fn report_error(
        &mut self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = text.into();
        self.status = Some(text.clone());
        window.push_notification(Notification::error(text), cx);
    }

    fn has_modes(&self, cx: &Context<Self>) -> bool {
        self.mode_select.read(cx).selected_value().is_some()
    }

    fn has_models(&self, cx: &Context<Self>) -> bool {
        self.model_select.read(cx).selected_value().is_some()
    }

    fn session_status_text(&self) -> String {
        if self.session_loading {
            "Creating session...".to_string()
        } else if let Some(session) = &self.current_session_id {
            format!(
                "Session {}",
                session.as_str().chars().take(8).collect::<String>()
            )
        } else {
            "Select an agent".to_string()
        }
    }

    fn workspace_name(&self) -> Option<&str> {
        self.workspace
            .as_ref()
            .map(|workspace| workspace.name.as_str())
    }

    fn workspace_id(&self) -> Option<WorkspaceId> {
        self.workspace
            .as_ref()
            .map(|workspace| workspace.id.clone())
    }
}

fn same_path(left: &std::path::Path, right: &std::path::Path) -> bool {
    left == right || left.canonicalize().ok().as_deref() == right.canonicalize().ok().as_deref()
}

fn task_name_from_content(content: &[ContentBlock]) -> Option<String> {
    content.iter().find_map(|block| {
        let text = block.as_plain_text()?.trim();
        if text.is_empty() {
            None
        } else {
            let first_line = text.lines().next().unwrap_or(text).trim();
            Some(truncate(first_line, 80))
        }
    })
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut truncated = text.chars().take(max).collect::<String>();
    truncated.push_str("...");
    truncated
}
