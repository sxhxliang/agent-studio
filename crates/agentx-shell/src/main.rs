//! The rewrite's entry point — the composition root.
//!
//! Wires the driven adapters into the application's [`SessionService`] and opens
//! the launcher ([`agentx_ui::open_welcome_window`]), where the user picks an
//! agent from their `config.json` to start chatting:
//!
//! ```text
//! cargo run -p agentx-shell
//! ```
//!
//! No Tokio runtime is created here. The store adapters use blocking `std::fs`,
//! so config loads and timeline reads happen on GPUI's own executor; the
//! [`PersistenceProjector`] drains the bus on GPUI's background executor so the
//! per-event file appends never block the UI thread. Each agent runs its own
//! current-thread Tokio runtime inside `agentx-acp` for its subprocess I/O.
//! The projector subscribes before any agent starts; each chat launch subscribes
//! its own view stream before booting its agent, so no session-setup events are
//! missed.

use std::sync::Arc;

use anyhow::Result;

use agentx_acp::AcpSupervisor;
use agentx_app::{PersistenceProjector, SessionService};
use agentx_bus::EventBus;
use agentx_domain::{AgentGateway, AgentRegistry, Config, ConfigStore, DomainEvent, SessionRepository};
use agentx_store::{FsConfigStore, FsSessionRepository, paths};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_dir = paths::data_dir();
    let config_path = paths::config_path(&data_dir);
    let sessions_dir = paths::sessions_dir(&data_dir);
    let cwd = std::env::current_dir()?;

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);

            let bus = EventBus::new();
            let repository: Arc<dyn SessionRepository> =
                Arc::new(FsSessionRepository::new(sessions_dir));

            // Persist the timeline. Subscribed now (before any agent starts) and
            // drained on the background executor so the per-event `std::fs`
            // appends never block the UI thread.
            {
                let projector = PersistenceProjector::new(repository.clone());
                let mut events = bus.subscribe::<DomainEvent>();
                cx.background_executor()
                    .spawn(async move {
                        while let Ok(event) = events.recv().await {
                            if let Err(error) = projector.project(&event).await {
                                log::warn!("persist failed: {error}");
                            }
                        }
                    })
                    .detach();
            }

            let supervisor = Arc::new(AcpSupervisor::new(bus.clone()));
            let service = Arc::new(SessionService::new(
                supervisor.clone() as Arc<dyn AgentGateway>,
                bus.clone(),
                repository,
            ));
            let registry = supervisor as Arc<dyn AgentRegistry>;

            cx.spawn(async move |cx| {
                // Blocking `std::fs` is runtime-agnostic, so loading config on
                // GPUI's executor is fine (it's a one-off, KB-sized read). A
                // missing or malformed file is not fatal — the launcher just
                // shows no agents until the user fixes `config.json`.
                let config = FsConfigStore::new(config_path)
                    .load()
                    .await
                    .unwrap_or_else(|error| {
                        log::error!("load config.json: {error}");
                        Config::default()
                    });

                let _ = cx.update(|cx| {
                    agentx_ui::open_welcome_window(registry, service, bus, config, cwd, cx);
                });
            })
            .detach();
        });

    Ok(())
}
