use agentx::Assets;
use agentx::{AgentManager, Config, PermissionStore, workspace::open_new};
use anyhow::Context as _;
use gpui::Application;
use std::sync::Arc;

fn main() {
    // Parse config path from command line arguments
    let config_path = parse_config_path();

    let app = Application::new().with_assets(Assets);
    app.run(move |cx| {
        agentx::init(cx);

        // Get session_bus and permission_bus from global AppState
        let session_bus = agentx::AppState::global(cx).session_bus.clone();
        let permission_bus = agentx::AppState::global(cx).permission_bus.clone();

        cx.spawn(async move |cx| {
            let config: Config = match std::fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))
            {
                Ok(raw) => match serde_json::from_str(&raw)
                    .with_context(|| format!("invalid config at {}", config_path.display()))
                {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("Failed to parse config: {}", e);
                        return;
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read config file: {}", e);
                    return;
                }
            };

            println!("Config loaded from {}", config_path.display());

            // Initialize agent manager
            let permission_store = Arc::new(PermissionStore::default());

            match AgentManager::initialize(
                config.agent_servers.clone(),
                permission_store.clone(),
                session_bus.clone(),
                permission_bus.clone(),
            )
            .await
            {
                Ok(manager) => {
                    let agent_count = manager.list_agents().await.len();
                    println!("Loaded {} agents.", agent_count);

                    // Set the first agent as active by default
                    let active_agent: Option<String> = manager.list_agents().await.first().cloned();

                    if let Some(ref agent) = active_agent {
                        println!("Active agent set to: {}", agent);
                    }

                    // Store in global AppState
                    let init_result = cx.update(|cx| {
                        // Set config path first
                        agentx::AppState::global_mut(cx).set_config_path(config_path.clone());
                        // Then set agent manager with config
                        agentx::AppState::global_mut(cx).set_agent_manager(manager, config);
                        agentx::AppState::global_mut(cx).set_permission_store(permission_store);

                        // Get message service for persistence initialization
                        agentx::AppState::global(cx).message_service().cloned()
                    });

                    // Initialize persistence subscription in async context
                    if let Ok(Some(message_service)) = init_result {
                        message_service.init_persistence();
                        let _ = cx.update(|cx| {
                            open_new(cx, |_, _, _| {
                                // Load settings and config
                            })
                            .detach();
                        });
                    } else {
                        eprintln!("MessageService not initialized, window will not open");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to initialize agent manager: {}", e);
                }
            }
        })
        .detach();
    });
}

/// Parse config path from command line arguments
fn parse_config_path() -> std::path::PathBuf {
    let mut config_path = std::path::PathBuf::from("config.json");
    let mut args = std::env::args().skip(1);

    while let Some(flag) = args.next() {
        if flag == "--config" {
            if let Some(value) = args.next() {
                config_path = std::path::PathBuf::from(value);
            }
        }
    }

    config_path
}
