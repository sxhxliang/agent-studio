use agentx::Assets;
use agentx::core::config_manager;
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

        // Initialize platform-specific requirements for system tray (GTK on Linux)
        if let Err(e) = agentx::system_tray::init_platform() {
            log::error!("Failed to initialize platform for system tray: {}", e);
        }

        // Initialize system tray
        match agentx::system_tray::SystemTray::new() {
            Ok(tray) => {
                agentx::system_tray::setup_tray_event_handler(tray, cx);
                log::info!("System tray initialized successfully");
            }
            Err(e) => {
                log::error!("Failed to initialize system tray: {}", e);
            }
        }

        // Get session_bus and permission_bus from global AppState
        let session_bus = agentx::AppState::global(cx).session_bus.clone();
        let permission_bus = agentx::AppState::global(cx).permission_bus.clone();

        // Open GUI window immediately (non-blocking)
        open_new(cx, |_, _, _| {
            // GUI window is now open
        })
        .detach();

        // Initialize agents in the background (async, non-blocking)
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
                        match load_default_config() {
                            Ok(config) => config,
                            Err(e) => {
                                eprintln!("Failed to load default config: {}", e);
                                return;
                            }
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read config file: {}", e);
                    match load_default_config() {
                        Ok(config) => config,
                        Err(e) => {
                            eprintln!("Failed to load default config: {}", e);
                            return;
                        }
                    }
                }
            };

            println!("Config loaded from {}", config_path.display());

            // Inject nodejs_path from AppSettings into agent configs
            let nodejs_path = cx.update(|cx| {
                agentx::AppSettings::global(cx).nodejs_path.clone()
            });

            let mut agent_servers = config.agent_servers.clone();
            if !nodejs_path.is_empty() {
                log::info!("Using custom Node.js path from settings: {}", nodejs_path);
                // Inject nodejs_path into all agent configs
                for (_name, agent_config) in agent_servers.iter_mut() {
                    agent_config.nodejs_path = Some(nodejs_path.to_string());
                }
            }
            let agent_server_count = agent_servers.len();

            // Initialize agent manager (this happens in background after GUI is shown)
            let permission_store = Arc::new(PermissionStore::default());

            match AgentManager::initialize(
                agent_servers,
                permission_store.clone(),
                session_bus.clone(),
                permission_bus.clone(),
                config.proxy.clone(),
            )
            .await
            {
                Ok(manager) => {
                    println!(
                        "Initializing {} agents in background...",
                        agent_server_count
                    );

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
                    if let Some(message_service) = init_result {
                        message_service.init_persistence();
                        println!("Agent initialization started - agents will appear as they are ready");
                    } else {
                        eprintln!("MessageService not initialized");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to initialize agent manager: {}", e);
                    eprintln!("Please check if Node.js is installed or configure the Node.js path in Settings.");
                }
            }
        })
        .detach();
    });
}

/// Parse config path from command line arguments or use user data directory
fn parse_config_path() -> std::path::PathBuf {
    let mut args = std::env::args().skip(1);

    // Check if user specified a custom config path via --config flag
    while let Some(flag) = args.next() {
        if flag == "--config" {
            if let Some(value) = args.next() {
                return std::path::PathBuf::from(value);
            }
        }
    }

    // No custom config specified, use user data directory
    match config_manager::initialize_user_config() {
        Ok(path) => {
            println!("Using config from user data directory: {}", path.display());
            path
        }
        Err(e) => {
            eprintln!("Failed to initialize user config: {}", e);
            let fallback = config_manager::get_user_config_path_or_temp();
            if let Err(err) = config_manager::ensure_default_config_at(&fallback) {
                eprintln!(
                    "Failed to create fallback config at {}: {}",
                    fallback.display(),
                    err
                );
            }
            eprintln!("Falling back to {}", fallback.display());
            fallback
        }
    }
}

fn load_default_config() -> anyhow::Result<Config> {
    let raw = agentx::get_default_config()
        .ok_or_else(|| anyhow::anyhow!("embedded default config missing"))?;
    let config = serde_json::from_str(&raw).context("invalid embedded default config")?;
    Ok(config)
}
