use agentx::acp_client::{AgentManager, PermissionStore};
use agentx::workspace::open_new;
use agentx::{Config, Settings};
use anyhow::Context as _;
use gpui::Application;
use gpui_component_assets::Assets;
use std::sync::Arc;

fn main() {
    let app = Application::new().with_assets(Assets);
    let settings = Settings::parse().expect("Failed to parse settings");
    app.run(move |cx| {
        agentx::init(cx);

        // Get session_bus from global AppState
        let session_bus = agentx::AppState::global(cx).session_bus.clone();

        open_new(cx, |_, _, _| {
            // Load settings and config
        })
        .detach();

        cx.spawn(async move |cx| {
            let config: Config = match std::fs::read_to_string(&settings.config_path)
                .with_context(|| format!("failed to read {}", settings.config_path.display()))
            {
                Ok(raw) => match serde_json::from_str(&raw).with_context(|| {
                    format!("invalid config at {}", settings.config_path.display())
                }) {
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

            println!("Config loaded from {}", settings.config_path.display());

            // Initialize agent manager
            let permission_store = Arc::new(PermissionStore::default());

            match AgentManager::initialize(
                config.agent_servers.clone(),
                permission_store.clone(),
                session_bus.clone(),
            )
            .await
            {
                Ok(manager) => {
                    println!("Loaded {} agents.", manager.list_agents().len());

                    // Set the first agent as active by default
                    let active_agent: Option<String> = manager.list_agents().first().cloned();

                    if let Some(ref agent) = active_agent {
                        println!("Active agent set to: {}", agent);
                    }

                    // Store in global AppState
                    cx.update(|cx| {
                        agentx::AppState::global_mut(cx).set_agent_manager(manager);
                        agentx::AppState::global_mut(cx).set_permission_store(permission_store);
                    })
                    .ok();
                }
                Err(e) => {
                    eprintln!("Failed to initialize agent manager: {}", e);
                }
            }
        })
        .detach();
    });
}
