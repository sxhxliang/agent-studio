use gpui::*;
use gpui_component::input::InputState;
use rust_i18n::t;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::{
    AppSettings, AppState,
    core::nodejs::{NodeJsChecker, NodeJsDetectionMode},
    utils,
};

use super::state::NodeJsStatus;
use crate::workspace::DockWorkspace;

impl DockWorkspace {
    pub(in crate::workspace) fn ensure_startup_initialized(
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

    pub(in crate::workspace) fn start_nodejs_check(
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
            let result = smol::unblock(move || {
                let checker = NodeJsChecker::new(custom_path).with_detection_mode(mode);
                checker.check_nodejs_available_blocking()
            })
            .await;

            _ = this.update_in(window, |this, _, cx| {
                match result {
                    Ok(result) => {
                        if result.available {
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

    pub(in crate::workspace) fn validate_custom_nodejs_path(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.startup_state.nodejs_custom_path_validating {
            return;
        }

        let input_value = self
            .startup_state
            .nodejs_custom_path_input
            .as_ref()
            .map(|input: &Entity<gpui_component::input::InputState>| {
                input.read(cx).value().to_string()
            })
            .unwrap_or_default();

        let input_value = input_value.trim().to_string();
        if input_value.is_empty() {
            self.startup_state.nodejs_custom_path_error =
                Some(t!("startup.nodejs.error.empty_path").to_string());
            cx.notify();
            return;
        }

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

    pub(in crate::workspace) fn apply_agent_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(in crate::workspace) fn open_workspace_folder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(in crate::workspace) fn apply_proxy_config(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            .map(|input: &Entity<gpui_component::input::InputState>| input.read(cx).value())
            .unwrap_or_default();
        let https_proxy_url = https_input
            .as_ref()
            .map(|input: &Entity<gpui_component::input::InputState>| input.read(cx).value())
            .unwrap_or_default();
        let all_proxy_url = all_input
            .as_ref()
            .map(|input: &Entity<gpui_component::input::InputState>| input.read(cx).value())
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
}
