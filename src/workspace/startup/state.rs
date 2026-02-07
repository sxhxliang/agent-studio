use std::collections::HashMap;
use std::path::PathBuf;

use gpui::*;

use crate::core::config::{AgentProcessConfig, Config};

#[derive(Clone, Debug)]
pub(in crate::workspace) struct AgentChoice {
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub(in crate::workspace) enum NodeJsStatus {
    Idle,
    Checking,
    Available {
        version: Option<String>,
        path: Option<PathBuf>,
    },
    Unavailable {
        message: String,
        hint: Option<String>,
    },
}

#[derive(Debug)]
pub struct StartupState {
    pub(in crate::workspace) initialized: bool,
    pub(in crate::workspace) step: usize,
    pub(in crate::workspace) intro_completed: bool,
    pub(in crate::workspace) nodejs_status: NodeJsStatus,
    pub(in crate::workspace) nodejs_skipped: bool,
    pub(in crate::workspace) nodejs_custom_path_input:
        Option<Entity<gpui_component::input::InputState>>,
    pub(in crate::workspace) nodejs_custom_path_validating: bool,
    pub(in crate::workspace) nodejs_custom_path_error: Option<String>,
    pub(in crate::workspace) nodejs_show_custom_input: bool,
    pub(in crate::workspace) agent_choices: Vec<AgentChoice>,
    pub(in crate::workspace) default_agent_configs: HashMap<String, AgentProcessConfig>,
    pub(in crate::workspace) agent_list_scroll_handle: ScrollHandle,
    pub(in crate::workspace) agent_apply_in_progress: bool,
    pub(in crate::workspace) agent_apply_error: Option<String>,
    pub(in crate::workspace) agent_load_error: Option<String>,
    pub(in crate::workspace) agent_applied: bool,
    pub(in crate::workspace) agent_synced: bool,
    pub(in crate::workspace) agent_sync_in_progress: bool,
    pub(in crate::workspace) proxy_enabled: bool,
    pub(in crate::workspace) proxy_http_input: Option<Entity<gpui_component::input::InputState>>,
    pub(in crate::workspace) proxy_https_input: Option<Entity<gpui_component::input::InputState>>,
    pub(in crate::workspace) proxy_all_input: Option<Entity<gpui_component::input::InputState>>,
    pub(in crate::workspace) proxy_apply_in_progress: bool,
    pub(in crate::workspace) proxy_apply_error: Option<String>,
    pub(in crate::workspace) proxy_applied: bool,
    pub(in crate::workspace) proxy_inputs_initialized: bool,
    pub(in crate::workspace) workspace_selected: bool,
    pub(in crate::workspace) workspace_path: Option<PathBuf>,
    pub(in crate::workspace) workspace_loading: bool,
    pub(in crate::workspace) workspace_error: Option<String>,
    pub(in crate::workspace) workspace_checked: bool,
    pub(in crate::workspace) workspace_check_in_progress: bool,
}

impl StartupState {
    pub(in crate::workspace) fn new() -> Self {
        let (agent_choices, default_agent_configs, agent_load_error) =
            Self::load_default_agent_configs();
        let agent_applied = agent_choices.is_empty();

        Self {
            initialized: false,
            step: 0,
            intro_completed: false,
            nodejs_status: NodeJsStatus::Idle,
            nodejs_skipped: false,
            nodejs_custom_path_input: None,
            nodejs_custom_path_validating: false,
            nodejs_custom_path_error: None,
            nodejs_show_custom_input: false,
            agent_choices,
            default_agent_configs,
            agent_list_scroll_handle: ScrollHandle::new(),
            agent_apply_in_progress: false,
            agent_apply_error: None,
            agent_load_error,
            agent_applied,
            agent_synced: false,
            agent_sync_in_progress: false,
            proxy_enabled: false,
            proxy_http_input: None,
            proxy_https_input: None,
            proxy_all_input: None,
            proxy_apply_in_progress: false,
            proxy_apply_error: None,
            proxy_applied: false,
            proxy_inputs_initialized: false,
            workspace_selected: false,
            workspace_path: None,
            workspace_loading: false,
            workspace_error: None,
            workspace_checked: false,
            workspace_check_in_progress: false,
        }
    }

    pub(in crate::workspace) fn nodejs_ready(&self) -> bool {
        self.nodejs_skipped || matches!(self.nodejs_status, NodeJsStatus::Available { .. })
    }

    pub(in crate::workspace) fn agents_ready(&self) -> bool {
        self.agent_applied || self.agent_choices.is_empty()
    }

    pub(in crate::workspace) fn workspace_ready(&self) -> bool {
        self.workspace_selected
    }

    pub(in crate::workspace) fn proxy_ready(&self) -> bool {
        self.proxy_applied
    }

    pub(in crate::workspace) fn is_complete(&self) -> bool {
        self.intro_completed
            && self.nodejs_ready()
            && self.agents_ready()
            && self.proxy_ready()
            && self.workspace_ready()
    }

    pub(in crate::workspace) fn advance_step_if_needed(&mut self) {
        if self.step == 0 && self.intro_completed {
            self.step = 1;
        }
        if self.step == 1 && self.nodejs_ready() {
            self.step = 2;
        }
        if self.step == 2 && self.agents_ready() {
            self.step = 3;
        }
        if self.step == 3 && self.proxy_ready() {
            self.step = 4;
        }
        if self.step > 4 {
            self.step = 4;
        }
    }

    fn load_default_agent_configs() -> (
        Vec<AgentChoice>,
        HashMap<String, AgentProcessConfig>,
        Option<String>,
    ) {
        let raw = match crate::assets::get_default_config() {
            Some(raw) => raw,
            None => {
                return (
                    Vec::new(),
                    HashMap::new(),
                    Some("Embedded config.json not found.".to_string()),
                );
            }
        };

        let config: Config = match serde_json::from_str(&raw) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to parse embedded config.json: {}", err);
                return (
                    Vec::new(),
                    HashMap::new(),
                    Some(format!("Failed to parse embedded config.json: {}", err)),
                );
            }
        };

        let mut agent_entries: Vec<_> = config.agent_servers.into_iter().collect();
        agent_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut agent_choices = Vec::new();
        let mut default_agent_configs = HashMap::new();

        for (name, config) in agent_entries {
            default_agent_configs.insert(name.clone(), config.clone());
            agent_choices.push(AgentChoice {
                name,
                enabled: true,
            });
        }

        (agent_choices, default_agent_configs, None)
    }
}
