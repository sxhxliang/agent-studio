use gpui::*;

use crate::{
    AppState,
    app::actions::{
        AddAgent, ChangeConfigPath, ReloadAgentConfig, RemoveAgent, RestartAgent, SetUploadDir,
        UpdateAgent,
    },
};

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
        nodejs_path: None,
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
        nodejs_path: None,
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
            if agent_config_service.has_active_sessions(&name).await {
                log::warn!(
                    "Agent '{}' has active sessions. User should confirm removal.",
                    name
                );
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

    if !new_path.exists() {
        log::error!("Config file does not exist: {:?}", new_path);
        return;
    }

    let config_result = std::fs::read_to_string(&new_path);
    match config_result {
        Ok(json) => match serde_json::from_str::<crate::core::config::Config>(&json) {
            Ok(_config) => {
                log::info!("Config file validated successfully: {:?}", new_path);

                AppState::global_mut(cx).set_config_path(new_path.clone());

                log::warn!(
                    "Config path changed to: {:?}. Please restart the application to apply changes.",
                    new_path
                );

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
        },
        Err(e) => {
            log::error!("Failed to read config file: {}", e);
        }
    }
}
