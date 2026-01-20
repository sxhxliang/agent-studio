use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Window, px,
};
use gpui_component::{
    input::InputState,
    setting::{SettingPage, Settings},
};
use rust_i18n::t;
use std::{collections::HashMap, path::PathBuf};

use crate::{
    AppState,
    core::{
        config::{AgentProcessConfig, CommandConfig, McpServerConfig, ModelConfig},
        updater::UpdateManager,
    },
};

use super::types::{AppSettings, UpdateStatus};

pub struct SettingsPanel {
    pub(super) focus_handle: FocusHandle,
    pub(super) update_status: UpdateStatus,
    pub(super) update_manager: UpdateManager,
    // Cached configuration state (synchronized by events)
    pub(super) cached_agents: HashMap<String, AgentProcessConfig>,
    pub(super) cached_models: HashMap<String, ModelConfig>,
    pub(super) cached_mcp_servers: HashMap<String, McpServerConfig>,
    pub(super) cached_commands: HashMap<String, CommandConfig>,
    pub(super) cached_upload_dir: PathBuf,
    // JSON editor state for MCP servers
    pub(super) mcp_json_editor: Entity<InputState>,
    pub(super) mcp_json_error: Option<String>,
    pub(super) mcp_active_tab: usize,
    // System prompts input states
    pub(super) doc_comment_input: Entity<InputState>,
    pub(super) inline_comment_input: Entity<InputState>,
    pub(super) explain_input: Entity<InputState>,
    pub(super) improve_input: Entity<InputState>,
}

impl crate::panels::dock_panel::DockPanel for SettingsPanel {
    fn title() -> &'static str {
        "Settings"
    }

    fn title_key() -> Option<&'static str> {
        Some("settings.title")
    }

    fn description() -> &'static str {
        "A collection of settings groups and items for the application."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> gpui::Pixels {
        px(0.)
    }
}

impl SettingsPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // AppSettings is now initialized globally in themes::init(), so we don't need to set it here

        let mcp_json_editor = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .line_number(true)
                .indent_guides(true)
                .tab_size(gpui_component::input::TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .soft_wrap(false)
                .placeholder(t!("settings.mcp.json.placeholder").to_string())
        });

        // System prompts input states
        let doc_comment_input = cx.new(|cx| InputState::new(window, cx));
        let inline_comment_input = cx.new(|cx| InputState::new(window, cx));
        let explain_input = cx.new(|cx| InputState::new(window, cx));
        let improve_input = cx.new(|cx| InputState::new(window, cx));

        let panel = Self {
            focus_handle: cx.focus_handle(),
            update_status: UpdateStatus::Idle,
            update_manager: UpdateManager::default(),
            cached_agents: HashMap::new(),
            cached_models: HashMap::new(),
            cached_mcp_servers: HashMap::new(),
            cached_commands: HashMap::new(),
            cached_upload_dir: PathBuf::from("."),
            mcp_json_editor,
            mcp_json_error: None,
            mcp_active_tab: 0,
            doc_comment_input,
            inline_comment_input,
            explain_input,
            improve_input,
        };

        // Load all configuration from service asynchronously
        let weak_entity = cx.entity().downgrade();
        if let Some(service) = AppState::global(cx).agent_config_service() {
            let service = service.clone();
            cx.spawn_in(window, async move |_this, window| {
                let agents = service.list_agents().await;
                let models = service.list_models().await;
                let mcp_servers = service.list_mcp_servers().await;
                let commands = service.list_commands().await;
                let upload_dir = service.get_upload_dir().await;

                _ = window.update(|window, cx| {
                    if let Some(entity) = weak_entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.cached_agents = agents.into_iter().collect();
                            this.cached_models = models.into_iter().collect();
                            this.cached_mcp_servers = mcp_servers.into_iter().collect();
                            this.cached_commands = commands.into_iter().collect();
                            this.cached_upload_dir = upload_dir;
                            // Load system prompts into input fields
                            this.load_system_prompts(window, cx);
                            cx.notify();
                        });
                    }
                });
            })
            .detach();
        }

        // Subscribe to AgentConfigBus for dynamic updates
        let agent_config_bus = AppState::global(cx).agent_config_bus.clone();
        let weak_entity = cx.entity().downgrade();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        agent_config_bus.subscribe(move |event| {
            let _ = tx.send(event.clone());
        });

        cx.spawn_in(window, async move |_this, window| {
            while let Some(event) = rx.recv().await {
                if let Some(entity) = weak_entity.upgrade() {
                    _ = window.update(|_window, cx| {
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

        panel
    }

    /// Handle agent configuration events
    fn on_agent_config_event(
        &mut self,
        event: &crate::core::event_bus::agent_config_bus::AgentConfigEvent,
        cx: &mut Context<Self>,
    ) {
        use crate::core::event_bus::agent_config_bus::AgentConfigEvent;

        log::info!("[SettingsPanel] Processing config event: {:?}", event);

        // Update cache based on event type
        match event {
            // Agent events
            AgentConfigEvent::AgentAdded { name, config } => {
                self.cached_agents.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::AgentUpdated { name, config } => {
                self.cached_agents.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::AgentRemoved { name } => {
                self.cached_agents.remove(name);
            }

            // Model events
            AgentConfigEvent::ModelAdded { name, config } => {
                self.cached_models.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::ModelUpdated { name, config } => {
                self.cached_models.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::ModelRemoved { name } => {
                self.cached_models.remove(name);
            }

            // MCP Server events
            AgentConfigEvent::McpServerAdded { name, config } => {
                self.cached_mcp_servers.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::McpServerUpdated { name, config } => {
                self.cached_mcp_servers.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::McpServerRemoved { name } => {
                self.cached_mcp_servers.remove(name);
            }

            // Command events
            AgentConfigEvent::CommandAdded { name, config } => {
                self.cached_commands.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::CommandUpdated { name, config } => {
                self.cached_commands.insert(name.clone(), config.clone());
            }
            AgentConfigEvent::CommandRemoved { name } => {
                self.cached_commands.remove(name);
            }

            // Full reload
            AgentConfigEvent::ConfigReloaded { config } => {
                self.cached_agents = config.agent_servers.clone();
                self.cached_models = config.models.clone();
                self.cached_mcp_servers = config.mcp_servers.clone();
                self.cached_commands = config.commands.clone();
                self.cached_upload_dir = config.upload_dir.clone();
            }
        }

        // Trigger re-render
        cx.notify();
    }

    fn setting_pages(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Vec<SettingPage> {
        let view = cx.entity();
        let resettable = AppSettings::global(cx).resettable;

        vec![
            self.general_page(&view, resettable),
            self.network_page(&view),
            self.update_page(&view, resettable),
            self.agent_page(&view),
            self.model_page(&view),
            self.prompt_page(&view),
            self.mcp_page(&view),
            self.command_page(&view),
            super::about_page::about_page(resettable),
        ]
    }
}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_component::{Sizable, Size, group_box::GroupBoxVariant};

        let app_settings = AppSettings::global(cx);
        let size = Size::from_str(app_settings.size.as_str());
        let group_variant = GroupBoxVariant::from_str(app_settings.group_variant.as_str());

        Settings::new("app-settings")
            .with_size(size)
            .with_group_variant(group_variant)
            .pages(self.setting_pages(window, cx))
    }
}
