use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, Size as UiSize, StyledExt as _, ThemeMode,
    ThemeRegistry,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement as _,
    stepper::{Stepper, StepperItem},
    switch::Switch,
    v_flex,
};
use rust_i18n::t;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{
    AppSettings, AppState,
    app::actions::{SelectLocale, SwitchTheme, SwitchThemeMode},
    assets::get_agent_icon,
    core::{
        config::{AgentProcessConfig, Config},
        nodejs::{NodeJsChecker, NodeJsDetectionMode},
    },
    utils,
};

use super::DockWorkspace;

#[derive(Clone, Debug)]
struct AgentChoice {
    name: String,
    enabled: bool,
}

#[derive(Clone, Debug)]
enum NodeJsStatus {
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
pub(super) struct StartupState {
    initialized: bool,
    step: usize,
    intro_completed: bool,
    nodejs_status: NodeJsStatus,
    nodejs_skipped: bool,
    nodejs_custom_path_input: Option<Entity<InputState>>,
    nodejs_custom_path_validating: bool,
    nodejs_custom_path_error: Option<String>,
    nodejs_show_custom_input: bool,
    agent_choices: Vec<AgentChoice>,
    default_agent_configs: HashMap<String, AgentProcessConfig>,
    agent_list_scroll_handle: ScrollHandle,
    agent_apply_in_progress: bool,
    agent_apply_error: Option<String>,
    agent_load_error: Option<String>,
    agent_applied: bool,
    agent_synced: bool,
    agent_sync_in_progress: bool,
    proxy_enabled: bool,
    proxy_http_input: Option<Entity<InputState>>,
    proxy_https_input: Option<Entity<InputState>>,
    proxy_all_input: Option<Entity<InputState>>,
    proxy_apply_in_progress: bool,
    proxy_apply_error: Option<String>,
    proxy_applied: bool,
    proxy_inputs_initialized: bool,
    workspace_selected: bool,
    workspace_path: Option<PathBuf>,
    workspace_loading: bool,
    workspace_error: Option<String>,
    workspace_checked: bool,
    workspace_check_in_progress: bool,
}

impl StartupState {
    pub(super) fn new() -> Self {
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

    fn nodejs_ready(&self) -> bool {
        self.nodejs_skipped || matches!(self.nodejs_status, NodeJsStatus::Available { .. })
    }

    fn agents_ready(&self) -> bool {
        self.agent_applied || self.agent_choices.is_empty()
    }

    fn workspace_ready(&self) -> bool {
        self.workspace_selected
    }

    fn proxy_ready(&self) -> bool {
        self.proxy_applied
    }

    pub(super) fn is_complete(&self) -> bool {
        self.intro_completed
            && self.nodejs_ready()
            && self.agents_ready()
            && self.proxy_ready()
            && self.workspace_ready()
    }

    fn advance_step_if_needed(&mut self) {
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

impl DockWorkspace {
    pub(super) fn ensure_startup_initialized(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.startup_state.initialized {
            self.startup_state.initialized = true;
        }

        if self.startup_state.intro_completed {
            self.ensure_proxy_inputs_initialized(window, cx);
            self.ensure_nodejs_input_initialized(window, cx);
            if matches!(self.startup_state.nodejs_status, NodeJsStatus::Idle) {
                self.start_nodejs_check(window, cx, NodeJsDetectionMode::Fast);
            }

            self.maybe_sync_agents(window, cx);
            self.maybe_check_workspace(window, cx);
        }
    }

    fn ensure_nodejs_input_initialized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.nodejs_custom_path_input.is_some() {
            return;
        }

        // Pre-fill with saved nodejs_path from settings
        let saved_path = AppSettings::global(cx).nodejs_path.clone();
        let placeholder = if cfg!(target_os = "windows") {
            t!("startup.nodejs.placeholder.windows").to_string()
        } else {
            t!("startup.nodejs.placeholder.unix").to_string()
        };

        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(placeholder);
            if !saved_path.is_empty() {
                state.set_value(saved_path.to_string(), window, cx);
            }
            state
        });

        self.startup_state.nodejs_custom_path_input = Some(input);
    }

    fn ensure_proxy_inputs_initialized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.proxy_inputs_initialized {
            return;
        }

        let http_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("http://127.0.0.1:1087".to_string()));
        let https_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("http://127.0.0.1:1087".to_string()));
        let all_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("socks5://127.0.0.1:1080".to_string())
        });

        self.startup_state.proxy_http_input = Some(http_input);
        self.startup_state.proxy_https_input = Some(https_input);
        self.startup_state.proxy_all_input = Some(all_input);
        self.startup_state.proxy_inputs_initialized = true;
    }

    fn start_nodejs_check(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        mode: NodeJsDetectionMode,
    ) {
        if matches!(self.startup_state.nodejs_status, NodeJsStatus::Checking) {
            return;
        }

        let custom_path = AppSettings::global(cx).nodejs_path.clone();
        let custom_path = if custom_path.is_empty() {
            None
        } else {
            Some(PathBuf::from(custom_path.to_string()))
        };

        self.startup_state.nodejs_status = NodeJsStatus::Checking;
        self.startup_state.nodejs_skipped = false;
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            // Run nodejs check on a background thread to avoid blocking the UI thread.
            // NodeJsChecker uses tokio commands internally, and blocking here would freeze the UI.
            let result = smol::unblock(move || {
                let checker = NodeJsChecker::new(custom_path).with_detection_mode(mode);
                checker.check_nodejs_available_blocking()
            })
            .await;

            _ = this.update_in(window, |this, _, cx| {
                match result {
                    Ok(result) => {
                        if result.available {
                            // Save the detected path to settings so it's used next time
                            if let Some(ref path) = result.path {
                                let path_str = path.display().to_string();
                                AppSettings::global_mut(cx).nodejs_path = path_str.into();
                                crate::themes::save_state(cx);
                                log::info!(
                                    "Saved detected Node.js path to settings: {}",
                                    path.display()
                                );
                            }

                            this.startup_state.nodejs_status = NodeJsStatus::Available {
                                version: result.version,
                                path: result.path,
                            };
                        } else {
                            this.startup_state.nodejs_status = NodeJsStatus::Unavailable {
                                message: result.error_message.unwrap_or_else(|| {
                                    t!("startup.nodejs.error.not_found").to_string()
                                }),
                                hint: result.install_hint,
                            };
                        }
                    }
                    Err(err) => {
                        this.startup_state.nodejs_status = NodeJsStatus::Unavailable {
                            message: err.to_string(),
                            hint: None,
                        };
                    }
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn validate_custom_nodejs_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.nodejs_custom_path_validating {
            return;
        }

        let input_value = self
            .startup_state
            .nodejs_custom_path_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();

        let input_value = input_value.trim().to_string();
        if input_value.is_empty() {
            self.startup_state.nodejs_custom_path_error =
                Some(t!("startup.nodejs.error.empty_path").to_string());
            cx.notify();
            return;
        }

        // Expand ~ to home directory
        let expanded = if input_value.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME") {
                input_value.replacen('~', &home, 1)
            } else {
                input_value.clone()
            }
        } else {
            input_value.clone()
        };

        let custom_path = PathBuf::from(&expanded);

        self.startup_state.nodejs_custom_path_validating = true;
        self.startup_state.nodejs_custom_path_error = None;
        self.startup_state.nodejs_status = NodeJsStatus::Checking;
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            let result = smol::unblock(move || {
                let checker = NodeJsChecker::new(Some(custom_path));
                checker.check_nodejs_available_blocking()
            })
            .await;

            _ = this.update_in(window, |this, _, cx| {
                this.startup_state.nodejs_custom_path_validating = false;

                match result {
                    Ok(result) if result.available => {
                        // Save the validated custom path to settings
                        if let Some(ref path) = result.path {
                            let path_str = path.display().to_string();
                            AppSettings::global_mut(cx).nodejs_path = path_str.into();
                            crate::themes::save_state(cx);
                            log::info!("Saved custom Node.js path to settings: {}", path.display());
                        }

                        this.startup_state.nodejs_status = NodeJsStatus::Available {
                            version: result.version,
                            path: result.path,
                        };
                        this.startup_state.nodejs_custom_path_error = None;
                    }
                    Ok(result) => {
                        this.startup_state.nodejs_status = NodeJsStatus::Unavailable {
                            message: result.error_message.clone().unwrap_or_else(|| {
                                t!("startup.nodejs.error.not_found").to_string()
                            }),
                            hint: result.install_hint.clone(),
                        };
                        this.startup_state.nodejs_custom_path_error =
                            Some(result.error_message.unwrap_or_else(|| {
                                t!("startup.nodejs.error.invalid_path").to_string()
                            }));
                    }
                    Err(err) => {
                        this.startup_state.nodejs_status = NodeJsStatus::Unavailable {
                            message: err.to_string(),
                            hint: None,
                        };
                        this.startup_state.nodejs_custom_path_error = Some(
                            t!(
                                "startup.nodejs.error.validate_failed",
                                error = err.to_string()
                            )
                            .to_string(),
                        );
                    }
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn maybe_sync_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.agent_synced || self.startup_state.agent_sync_in_progress {
            return;
        }

        let agent_config_service = match AppState::global(cx).agent_config_service() {
            Some(service) => service.clone(),
            None => return,
        };

        self.startup_state.agent_sync_in_progress = true;

        cx.spawn_in(window, async move |this, window| {
            let current_agents = agent_config_service.list_agents().await;
            let current_names: HashSet<String> =
                current_agents.into_iter().map(|(name, _)| name).collect();

            _ = this.update_in(window, |this, _, cx| {
                for choice in &mut this.startup_state.agent_choices {
                    choice.enabled = current_names.contains(&choice.name);
                }

                this.startup_state.agent_synced = true;
                this.startup_state.agent_sync_in_progress = false;

                if this.startup_state.agent_choices.is_empty() {
                    this.startup_state.agent_applied = true;
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn maybe_check_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.workspace_checked || self.startup_state.workspace_check_in_progress {
            return;
        }

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.workspace_checked = true;
                return;
            }
        };

        self.startup_state.workspace_check_in_progress = true;

        cx.spawn_in(window, async move |this, window| {
            let active_workspace = workspace_service.get_active_workspace().await;
            let fallback_workspace = if active_workspace.is_none() {
                workspace_service.list_workspaces().await.into_iter().next()
            } else {
                None
            };

            let selected_path = active_workspace
                .map(|ws| ws.path)
                .or_else(|| fallback_workspace.map(|ws| ws.path));

            _ = this.update_in(window, |this, _, cx| {
                if let Some(path) = selected_path {
                    this.startup_state.workspace_selected = true;
                    this.startup_state.workspace_path = Some(path.clone());
                    AppState::global_mut(cx).set_current_working_dir(path);
                }

                this.startup_state.workspace_checked = true;
                this.startup_state.workspace_check_in_progress = false;
                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_agent_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.agent_apply_in_progress {
            return;
        }

        let agent_config_service = match AppState::global(cx).agent_config_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.agent_apply_error =
                    Some(t!("startup.agents.error.service_unavailable").to_string());
                cx.notify();
                return;
            }
        };

        let selections = self.startup_state.agent_choices.clone();
        let default_configs = self.startup_state.default_agent_configs.clone();

        self.startup_state.agent_apply_in_progress = true;
        self.startup_state.agent_apply_error = None;
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            let current_agents = agent_config_service.list_agents().await;
            let current_names: HashSet<String> =
                current_agents.into_iter().map(|(name, _)| name).collect();
            let mut errors = Vec::new();

            for choice in selections {
                if choice.enabled && !current_names.contains(&choice.name) {
                    match default_configs.get(&choice.name) {
                        Some(config) => {
                            if let Err(err) = agent_config_service
                                .add_agent(choice.name.clone(), config.clone())
                                .await
                            {
                                errors.push(format!("Failed to enable {}: {}", choice.name, err));
                            }
                        }
                        None => {
                            errors.push(format!(
                                "Missing config for selected agent: {}",
                                choice.name
                            ));
                        }
                    }
                } else if !choice.enabled && current_names.contains(&choice.name) {
                    if let Err(err) = agent_config_service.remove_agent(&choice.name).await {
                        errors.push(format!("Failed to disable {}: {}", choice.name, err));
                    }
                }
            }

            _ = this.update_in(window, |this, _, cx| {
                this.startup_state.agent_apply_in_progress = false;

                if errors.is_empty() {
                    this.startup_state.agent_applied = true;
                    this.startup_state.agent_apply_error = None;
                } else {
                    this.startup_state.agent_apply_error = Some(errors.join("\n"));
                }

                this.startup_state.advance_step_if_needed();
                cx.notify();
            });
        })
        .detach();
    }

    fn open_workspace_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.workspace_loading {
            return;
        }

        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.workspace_error =
                    Some(t!("startup.workspace.error.service_unavailable").to_string());
                cx.notify();
                return;
            }
        };

        self.startup_state.workspace_loading = true;
        self.startup_state.workspace_error = None;
        cx.notify();

        let dialog_title = t!("startup.workspace.dialog.title").to_string();

        cx.spawn_in(window, async move |this, window| {
            let selection = utils::pick_folder(&dialog_title).await;
            let cancelled = selection.is_none();

            if cancelled {
                _ = this.update_in(window, |this, _, cx| {
                    this.startup_state.workspace_loading = false;
                    cx.notify();
                });
                return;
            }

            let Some(folder_path) = selection else {
                return;
            };

            let add_result = workspace_service.add_workspace(folder_path.clone()).await;
            let mut selected_path = None;
            let mut error_message = None;

            match add_result {
                Ok(workspace) => {
                    selected_path = Some(workspace.path);
                }
                Err(err) => {
                    let message = err.to_string();
                    if message.contains("Workspace already exists") {
                        selected_path = Some(folder_path.clone());
                    } else {
                        error_message = Some(message);
                    }
                }
            }

            _ = this.update_in(window, |this, _, cx| {
                this.startup_state.workspace_loading = false;

                if let Some(path) = selected_path {
                    this.startup_state.workspace_selected = true;
                    this.startup_state.workspace_path = Some(path.clone());
                    this.startup_state.workspace_error = None;
                    this.startup_state.workspace_checked = true;
                    AppState::global_mut(cx).set_current_working_dir(path);
                    this.startup_state.advance_step_if_needed();
                } else {
                    this.startup_state.workspace_error = error_message;
                }

                cx.notify();
            });
        })
        .detach();
    }

    fn apply_proxy_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_state.proxy_apply_in_progress {
            return;
        }

        let agent_config_service = match AppState::global(cx).agent_config_service() {
            Some(service) => service.clone(),
            None => {
                self.startup_state.proxy_apply_error =
                    Some(t!("startup.proxy.error.service_unavailable").to_string());
                cx.notify();
                return;
            }
        };

        let http_input = self.startup_state.proxy_http_input.clone();
        let https_input = self.startup_state.proxy_https_input.clone();
        let all_input = self.startup_state.proxy_all_input.clone();
        let enabled = self.startup_state.proxy_enabled;

        let http_proxy_url = http_input
            .as_ref()
            .map(|input| input.read(cx).value())
            .unwrap_or_default();
        let https_proxy_url = https_input
            .as_ref()
            .map(|input| input.read(cx).value())
            .unwrap_or_default();
        let all_proxy_url = all_input
            .as_ref()
            .map(|input| input.read(cx).value())
            .unwrap_or_default();

        self.startup_state.proxy_apply_in_progress = true;
        self.startup_state.proxy_apply_error = None;
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            let proxy_config = crate::core::config::ProxyConfig {
                enabled,
                http_proxy_url: http_proxy_url.to_string(),
                https_proxy_url: https_proxy_url.to_string(),
                all_proxy_url: all_proxy_url.to_string(),
                proxy_type: String::new(),
                host: String::new(),
                port: 0,
                username: String::new(),
                password: String::new(),
            };

            let result = agent_config_service.update_proxy_config(proxy_config).await;

            _ = this.update_in(window, |this, _, cx| {
                this.startup_state.proxy_apply_in_progress = false;
                if let Err(err) = result {
                    this.startup_state.proxy_apply_error = Some(err.to_string());
                } else {
                    this.startup_state.proxy_applied = true;
                    this.startup_state.proxy_apply_error = None;
                    this.startup_state.advance_step_if_needed();
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_startup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // 获取步骤图标
        let intro_icon = if self.startup_state.intro_completed {
            IconName::CircleCheck
        } else {
            IconName::Settings
        };

        let node_icon = match self.startup_state.nodejs_status {
            NodeJsStatus::Available { .. } => IconName::CircleCheck,
            NodeJsStatus::Unavailable { .. } => IconName::TriangleAlert,
            NodeJsStatus::Checking => IconName::LoaderCircle,
            NodeJsStatus::Idle => IconName::SquareTerminal,
        };

        let agent_icon = if self.startup_state.agents_ready() {
            IconName::CircleCheck
        } else if self.startup_state.agent_apply_error.is_some() {
            IconName::TriangleAlert
        } else {
            IconName::Bot
        };

        let workspace_icon = if self.startup_state.workspace_ready() {
            IconName::CircleCheck
        } else {
            IconName::Folder
        };

        let proxy_icon = if self.startup_state.proxy_ready() {
            IconName::CircleCheck
        } else if self.startup_state.proxy_apply_error.is_some() {
            IconName::TriangleAlert
        } else {
            IconName::Globe
        };

        // 渲染步骤条
        let stepper = Stepper::new("startup-stepper")
            .w_full()
            .bg(cx.theme().background)
            .with_size(UiSize::Large)
            .selected_index(self.startup_state.step)
            .text_center(true)
            .items([
                StepperItem::new().icon(intro_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.preferences.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.preferences.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(node_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.nodejs.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.nodejs.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(agent_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.agents.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.agents.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(proxy_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.proxy.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.proxy.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(workspace_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.workspace.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.workspace.subtitle").to_string()),
                        ),
                ),
            ])
            .on_click(cx.listener(|this, step, _, cx| {
                if !this.startup_state.intro_completed && *step > 0 {
                    return;
                }
                this.startup_state.step = *step;
                cx.notify();
            }));

        // 渲染当前步骤内容
        let content = match self.startup_state.step {
            0 => self.render_preferences_step(cx),
            1 => self.render_nodejs_step(cx),
            2 => self.render_agents_step(cx),
            3 => self.render_proxy_step(cx),
            _ => self.render_workspace_step(cx),
        };

        let theme = cx.theme();
        let bg_color = theme.background;

        div()
            .flex_1()
            .size_full()
            .bg(theme.background)
            .flex()
            .items_center()
            .justify_center()
            .p_8() // 添加外边距，防止内容贴边
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(960.)) // 最大宽度 960px
                    .gap_8()
                    .child(
                        // 标题
                        div()
                            .text_size(px(36.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .text_center()
                            .child(t!("startup.title").to_string()),
                    )
                    .child(stepper)
                    .child(
                        // 内容卡片
                        div()
                            .w_full()
                            .min_h(px(400.))
                            .rounded(px(16.))
                            .bg(bg_color)
                            .shadow_lg()
                            .border_1()
                            .border_color(theme.border)
                            .p_8()
                            .child(content),
                    ),
            )
            .into_any_element()
    }

    fn render_preferences_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let current_locale = AppSettings::global(cx).locale.clone();
        let current_theme = cx.theme().theme_name().clone();
        let is_dark = cx.theme().mode.is_dark();
        let themes = ThemeRegistry::global(cx).sorted_themes();

        let locale_buttons = h_flex()
            .gap_2()
            .child(
                Button::new("startup-locale-en")
                    .label(t!("startup.preferences.locale.en").to_string())
                    .when(current_locale.as_ref() == "en", |btn| btn.primary())
                    .when(current_locale.as_ref() != "en", |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SelectLocale("en".into())), cx);
                    })),
            )
            .child(
                Button::new("startup-locale-zh")
                    .label(t!("startup.preferences.locale.zh_cn").to_string())
                    .when(current_locale.as_ref() == "zh-CN", |btn| btn.primary())
                    .when(current_locale.as_ref() != "zh-CN", |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SelectLocale("zh-CN".into())), cx);
                    })),
            );

        let theme_mode_buttons = h_flex()
            .gap_2()
            .child(
                Button::new("startup-theme-light")
                    .label(t!("startup.preferences.mode.light").to_string())
                    .when(!is_dark, |btn| btn.primary())
                    .when(is_dark, |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SwitchThemeMode(ThemeMode::Light)), cx);
                    })),
            )
            .child(
                Button::new("startup-theme-dark")
                    .label(t!("startup.preferences.mode.dark").to_string())
                    .when(is_dark, |btn| btn.primary())
                    .when(!is_dark, |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SwitchThemeMode(ThemeMode::Dark)), cx);
                    })),
            );

        let mut theme_buttons = h_flex().w_full().gap_2().flex_wrap();
        for (idx, theme_config) in themes.iter().enumerate() {
            let name = theme_config.name.clone();
            let is_active = name == current_theme;
            theme_buttons = theme_buttons.child(
                Button::new(("startup-theme-btn", idx))
                    .label(name.clone())
                    .when(is_active, |btn| btn.icon(IconName::Check))
                    .when(!is_active, |btn| btn.outline())
                    .small()
                    .on_click(cx.listener(move |_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SwitchTheme(name.clone())), cx);
                    })),
            );
        }

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.preferences.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.preferences.description").to_string()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.preferences.language_label").to_string()),
                    )
                    .child(locale_buttons),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.preferences.theme_mode_label").to_string()),
                    )
                    .child(theme_mode_buttons),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.preferences.theme_label").to_string()),
                    )
                    .child(theme_buttons),
            );

        let actions = h_flex().gap_3().mt_6().justify_end().child(
            Button::new("startup-preferences-next")
                .label(t!("startup.preferences.continue").to_string())
                .primary()
                .on_click(cx.listener(|this, _ev, _window, cx| {
                    this.startup_state.intro_completed = true;
                    if this.startup_state.step == 0 {
                        this.startup_state.step = 1;
                    }
                    this.startup_state.advance_step_if_needed();
                    cx.notify();
                })),
        );

        content = content.child(actions);
        content.into_any_element()
    }

    fn render_nodejs_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.nodejs.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.nodejs.description").to_string()),
            );

        match &self.startup_state.nodejs_status {
            NodeJsStatus::Idle => {
                content = content.child(
                    div()
                        .mt_4()
                        .p_4()
                        .rounded(theme.radius)
                        .bg(theme.muted)
                        .text_color(theme.muted_foreground)
                        .child(t!("startup.nodejs.status.idle").to_string()),
                );
            }
            NodeJsStatus::Checking => {
                content = content.child(
                    h_flex()
                        .mt_4()
                        .p_4()
                        .gap_3()
                        .items_center()
                        .rounded(theme.radius)
                        .bg(theme.muted)
                        .child(
                            div()
                                .text_color(theme.accent_foreground)
                                .child(t!("startup.nodejs.status.checking").to_string()),
                        ),
                );
            }
            NodeJsStatus::Available { version, path } => {
                let detail = match (version, path) {
                    (Some(version), Some(path)) => t!(
                        "startup.nodejs.detail.version_path",
                        version = version,
                        path = path.display().to_string()
                    )
                    .to_string(),
                    (Some(version), None) => {
                        t!("startup.nodejs.detail.version", version = version).to_string()
                    }
                    (None, Some(path)) => t!(
                        "startup.nodejs.detail.path",
                        path = path.display().to_string()
                    )
                    .to_string(),
                    (None, None) => t!("startup.nodejs.detail.available").to_string(),
                };

                content = content.child(
                    v_flex()
                        .mt_4()
                        .p_4()
                        .gap_2()
                        .rounded(theme.radius)
                        .bg(theme.background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_color(theme.success_active)
                                .font_weight(FontWeight::MEDIUM)
                                .child(t!("startup.nodejs.success").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme.muted_foreground)
                                .child(detail),
                        ),
                );
            }
            NodeJsStatus::Unavailable { message, hint } => {
                // Auto-show custom input when detection fails
                self.startup_state.nodejs_show_custom_input = true;

                content = content.child(
                    v_flex()
                        .mt_4()
                        .p_4()
                        .gap_2()
                        .rounded(theme.radius)
                        .bg(theme.background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_color(theme.colors.danger_active)
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("⚠ {}", message)),
                        )
                        .when_some(hint.as_ref(), |this, hint| {
                            this.child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.muted_foreground)
                                    .child(hint.clone()),
                            )
                        }),
                );
            }
        }

        // Show custom path input section when toggled or detection failed
        if self.startup_state.nodejs_show_custom_input {
            let custom_path_input = self.startup_state.nodejs_custom_path_input.clone();
            let is_validating = self.startup_state.nodejs_custom_path_validating;

            content = content.child(
                v_flex()
                    .mt_4()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(t!("startup.nodejs.custom.title").to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(t!("startup.nodejs.custom.hint").to_string()),
                    )
                    .when_some(custom_path_input, |this, input| {
                        this.child(
                            h_flex().gap_2().child(Input::new(&input).w_full()).child(
                                Button::new("startup-nodejs-validate")
                                    .label(if is_validating {
                                        t!("startup.nodejs.custom.validating").to_string()
                                    } else {
                                        t!("startup.nodejs.custom.validate").to_string()
                                    })
                                    .outline()
                                    .disabled(is_validating)
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.validate_custom_nodejs_path(window, cx);
                                    })),
                            ),
                        )
                    })
                    .when_some(
                        self.startup_state.nodejs_custom_path_error.clone(),
                        |this, error| {
                            this.child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.colors.danger_active)
                                    .child(error),
                            )
                        },
                    ),
            );
        }

        let mut actions = h_flex().gap_3().mt_6().justify_between();

        let show_custom = self.startup_state.nodejs_show_custom_input;
        let left_actions = h_flex()
            .gap_2()
            .child(
                Button::new("startup-nodejs-recheck")
                    .label(t!("startup.nodejs.action.recheck").to_string())
                    .outline()
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.start_nodejs_check(window, cx, NodeJsDetectionMode::Full);
                    })),
            )
            .child(
                Button::new("startup-nodejs-manual")
                    .label(if show_custom {
                        t!("startup.nodejs.action.collapse").to_string()
                    } else {
                        t!("startup.nodejs.action.manual").to_string()
                    })
                    .ghost()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.nodejs_show_custom_input =
                            !this.startup_state.nodejs_show_custom_input;
                        cx.notify();
                    })),
            );

        let right_actions = if self.startup_state.nodejs_ready() {
            h_flex().child(
                Button::new("startup-nodejs-next")
                    .label(t!("startup.nodejs.action.next").to_string())
                    .primary()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.step = 2;
                        cx.notify();
                    })),
            )
        } else {
            h_flex().child(
                Button::new("startup-nodejs-skip")
                    .label(t!("startup.nodejs.action.skip").to_string())
                    .ghost()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.nodejs_skipped = true;
                        this.startup_state.advance_step_if_needed();
                        cx.notify();
                    })),
            )
        };

        actions = actions.child(left_actions).child(right_actions);

        content.child(actions).into_any_element()
    }

    fn render_agents_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_6()
            .child(
                // 关闭按钮
                div().absolute().top_0().right_0().child(
                    Button::new("startup-close")
                        .ghost()
                        .icon(IconName::Close)
                        .on_click(cx.listener(|this, _ev, _, cx| {
                            this.startup_state.agent_applied = true;
                            this.startup_state.advance_step_if_needed();
                            cx.notify();
                        })),
                ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(t!("startup.agents.title").to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(theme.muted_foreground)
                            .line_height(rems(1.5))
                            .child(t!("startup.agents.description").to_string()),
                    ),
            );

        if let Some(error) = &self.startup_state.agent_load_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(theme.colors.danger_foreground)
                    .child(format!("⚠ {}", error)),
            );
        }

        if self.startup_state.agent_choices.is_empty() {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(t!("startup.agents.empty").to_string()),
            );
        } else {
            let disabled = self.startup_state.agent_apply_in_progress;

            // Agent 列表
            let mut list = v_flex().w_full().gap_0();

            for (idx, choice) in self.startup_state.agent_choices.iter().enumerate() {
                let name = choice.name.clone();
                let checked = choice.enabled;

                // Agent 图标映射
                let icon = get_agent_icon(&name);

                list = list.child(
                    h_flex()
                        .w_full()
                        .py_2()
                        .px_3()
                        .gap_2()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(theme.border)
                        .when(idx == 0, |this| this.border_t_1())
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Checkbox::new(("startup-agent-check", idx))
                                        .checked(checked)
                                        .disabled(disabled)
                                        .with_size(UiSize::Small)
                                        .on_click(cx.listener(move |this, checked, _, cx| {
                                            if let Some(choice) =
                                                this.startup_state.agent_choices.get_mut(idx)
                                            {
                                                choice.enabled = *checked;
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .w(px(24.))
                                        .h(px(24.))
                                        .rounded(px(6.))
                                        .bg(theme.background)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::new(icon)
                                                .size(px(14.))
                                                .text_color(theme.foreground),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child(name.clone()),
                                ),
                        )
                        .child(
                            Switch::new(("startup-agent-switch", idx))
                                .checked(checked)
                                .disabled(disabled)
                                .with_size(UiSize::Small)
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    if let Some(choice) =
                                        this.startup_state.agent_choices.get_mut(idx)
                                    {
                                        choice.enabled = *checked;
                                        cx.notify();
                                    }
                                })),
                        ),
                );
            }

            // Scrollable container with track_scroll for stable scrolling
            let scroll_handle = &self.startup_state.agent_list_scroll_handle;
            let scrollable_list = div()
                .id("agent-list-scroll-container")
                .w_full()
                .max_h(px(280.))
                .min_h_0()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.border)
                .overflow_y_scroll()
                .track_scroll(scroll_handle)
                .child(list);

            content = content.child(scrollable_list);
        }

        if AppState::global(cx).agent_config_service().is_none() {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(t!("startup.agents.service_loading").to_string()),
            );
        }

        if let Some(error) = &self.startup_state.agent_apply_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.colors.danger_foreground)
                    .child(format!("⚠ {}", error)),
            );
        }

        let service_ready = AppState::global(cx).agent_config_service().is_some();
        let apply_label = if self.startup_state.agent_apply_in_progress {
            t!("startup.agents.apply.in_progress")
        } else {
            t!("startup.agents.apply.ready")
        };

        let enabled_count = self
            .startup_state
            .agent_choices
            .iter()
            .filter(|c| c.enabled)
            .count();

        // 底部操作栏
        let actions = h_flex()
            .mt_6()
            .pt_6()
            .border_t_1()
            .border_color(theme.border)
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(theme.colors.muted_foreground)
                    .child(
                        t!(
                            "startup.agents.footer.selected",
                            selected = enabled_count,
                            total = self.startup_state.agent_choices.len()
                        )
                        .to_string(),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("startup-agent-skip")
                            .label(t!("startup.agents.action.skip").to_string())
                            .outline()
                            .on_click(cx.listener(|this, _ev, _, cx| {
                                this.startup_state.agent_applied = true;
                                this.startup_state.advance_step_if_needed();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("startup-agent-apply")
                            .label(apply_label.to_string())
                            .primary()
                            .disabled(!service_ready || self.startup_state.agent_apply_in_progress)
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.apply_agent_selection(window, cx);
                            })),
                    ),
            );

        content.child(actions).into_any_element()
    }

    fn render_proxy_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let http_input = self.startup_state.proxy_http_input.clone();
        let https_input = self.startup_state.proxy_https_input.clone();
        let all_input = self.startup_state.proxy_all_input.clone();

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.proxy.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.proxy.description").to_string()),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Switch::new("startup-proxy-enabled")
                            .checked(self.startup_state.proxy_enabled)
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.startup_state.proxy_enabled = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(theme.foreground)
                            .child(t!("startup.proxy.enable").to_string()),
                    ),
            );

        if let Some(http_input) = http_input {
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child("HTTP_PROXY"),
                    )
                    .child(
                        Input::new(&http_input)
                            .disabled(!self.startup_state.proxy_enabled)
                            .w_full(),
                    ),
            );
        }

        if let Some(https_input) = https_input {
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child("HTTPS_PROXY"),
                    )
                    .child(
                        Input::new(&https_input)
                            .disabled(!self.startup_state.proxy_enabled)
                            .w_full(),
                    ),
            );
        }

        if let Some(all_input) = all_input {
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child("ALL_PROXY"),
                    )
                    .child(
                        Input::new(&all_input)
                            .disabled(!self.startup_state.proxy_enabled)
                            .w_full(),
                    ),
            );
        }

        if let Some(error) = &self.startup_state.proxy_apply_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.colors.danger_foreground)
                    .child(format!("⚠ {}", error)),
            );
        }

        let apply_label = if self.startup_state.proxy_apply_in_progress {
            t!("startup.proxy.apply.in_progress")
        } else {
            t!("startup.proxy.apply.ready")
        };

        let actions = h_flex()
            .mt_6()
            .pt_6()
            .border_t_1()
            .border_color(theme.border)
            .justify_between()
            .items_center()
            .child(
                Button::new("startup-proxy-skip")
                    .label(t!("startup.proxy.action.skip").to_string())
                    .outline()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.proxy_applied = true;
                        this.startup_state.advance_step_if_needed();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("startup-proxy-apply")
                    .label(apply_label.to_string())
                    .primary()
                    .disabled(self.startup_state.proxy_apply_in_progress)
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.apply_proxy_config(window, cx);
                    })),
            );

        content.child(actions).into_any_element()
    }

    fn render_workspace_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.workspace.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.workspace.description").to_string()),
            );

        if let Some(path) = &self.startup_state.workspace_path {
            content = content.child(
                v_flex()
                    .mt_4()
                    .p_4()
                    .gap_2()
                    .rounded(theme.radius)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_color(theme.success_active)
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.workspace.status.selected").to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(format!("{}", path.display())),
                    ),
            );
        } else {
            content = content.child(
                div()
                    .mt_4()
                    .p_4()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(t!("startup.workspace.status.not_selected").to_string()),
            );
        }

        if let Some(error) = &self.startup_state.workspace_error {
            content = content.child(
                div()
                    .mt_4()
                    .p_4()
                    .rounded(theme.radius)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(theme.colors.danger_active)
                    .child(format!("⚠ {}", error)),
            );
        }

        if self.startup_state.workspace_loading {
            content = content.child(
                div()
                    .mt_4()
                    .p_4()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .text_color(theme.accent_foreground)
                    .child(t!("startup.workspace.status.loading").to_string()),
            );
        }

        let pick_label = if self.startup_state.workspace_selected {
            t!("startup.workspace.action.repick").to_string()
        } else {
            t!("startup.workspace.action.pick").to_string()
        };

        // 底部操作栏
        let actions = h_flex()
            .mt_6()
            .pt_6()
            .border_t_1()
            .border_color(theme.border)
            .justify_between()
            .items_center()
            .child(
                // 左侧按钮
                Button::new("startup-workspace-pick")
                    .label(pick_label)
                    .outline()
                    .disabled(self.startup_state.workspace_loading)
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.open_workspace_folder(window, cx);
                    })),
            )
            .child(
                // 右侧按钮组
                h_flex()
                    .gap_3()
                    .when(self.startup_state.workspace_selected, |this| {
                        this.child(
                            Button::new("startup-workspace-finish")
                                .label(t!("startup.workspace.action.finish").to_string())
                                .primary()
                                .disabled(self.startup_state.workspace_loading)
                                .on_click(cx.listener(|this, _ev, window, cx| {
                                    // 确保所有状态都已完成
                                    this.startup_state.workspace_selected = true;
                                    this.startup_state.workspace_checked = true;

                                    // 强制刷新整个窗口以触发主工作区显示
                                    window.refresh();
                                    cx.notify();
                                })),
                        )
                    }),
            );

        content.child(actions).into_any_element()
    }
}
