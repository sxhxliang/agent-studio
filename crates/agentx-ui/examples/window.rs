//! Windowed composition root for the rewrite.
//!
//! Wires the real adapters into the application's [`SessionService`] and opens a
//! [`agentx_ui::ChatView`] against a live agent from your `config.json`:
//!
//! ```text
//! cargo run -p agentx-ui --example window -- <agent>
//! ```
//!
//! No Tokio runtime is created here. The store adapters use blocking `std::fs`,
//! so config loads and timeline reads can happen on GPUI's own executor; the
//! [`PersistenceProjector`] drains the bus on GPUI's background executor so the
//! per-event file appends never block the UI thread. Each agent runs its own
//! current-thread Tokio runtime inside `agentx-acp` for its subprocess I/O.
//! Every bus consumer (projector + view) subscribes before the agent starts, so
//! no session-setup events are missed.
//!
//! Not yet here: a session list / resume UI (each run creates a fresh session,
//! which is persisted but not reloaded).

use std::sync::Arc;

use anyhow::{Context as _, Result};

use agentx_acp::AcpSupervisor;
use agentx_app::{PersistenceProjector, SessionService};
use agentx_bus::EventBus;
use agentx_domain::{
    AgentGateway, AgentId, AgentRegistry, ConfigStore, DomainEvent, SessionRepository,
};
use agentx_store::{FsConfigStore, FsSessionRepository, paths};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let agent = AgentId::from(std::env::args().nth(1).context("usage: window <agent>")?);
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

            // Persist the timeline. Subscribed now (before the agent starts) and
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

            // The view's event stream — also subscribed before the agent starts.
            let view_events = bus.subscribe::<DomainEvent>();

            let supervisor = Arc::new(AcpSupervisor::new(bus.clone()));
            let service = Arc::new(SessionService::new(
                supervisor.clone() as Arc<dyn AgentGateway>,
                bus.clone(),
                repository,
            ));

            cx.spawn(async move |cx| {
                // Blocking `std::fs` is runtime-agnostic, so loading config on
                // GPUI's executor is fine (it's a one-off, KB-sized read).
                let config = match FsConfigStore::new(config_path).load().await {
                    Ok(config) => config,
                    Err(error) => {
                        log::error!("load config.json: {error}");
                        let _ = cx.update(|cx| cx.quit());
                        return;
                    }
                };
                let Some(agent_config) = config.agents.get(agent.as_str()).cloned() else {
                    log::error!(
                        "agent `{agent}` not in config; available: {:?}",
                        config.agents.keys().collect::<Vec<_>>()
                    );
                    let _ = cx.update(|cx| cx.quit());
                    return;
                };

                supervisor.set_proxy(config.proxy.clone()).await.ok();
                if let Err(error) = supervisor.add_agent(agent.clone(), agent_config).await {
                    log::error!("failed to start agent `{agent}`: {error}");
                    let _ = cx.update(|cx| cx.quit());
                    return;
                }
                let session = match service.get_or_create_session(&agent, &cwd, &[]).await {
                    Ok(session) => session,
                    Err(error) => {
                        log::error!("failed to create session: {error}");
                        let _ = cx.update(|cx| cx.quit());
                        return;
                    }
                };
                let Some(init) = service.session_init(&session).await else {
                    log::error!("session capabilities unavailable");
                    let _ = cx.update(|cx| cx.quit());
                    return;
                };
                let _ = cx.update(|cx| {
                    agentx_ui::open_chat_window(service.clone(), view_events, agent.clone(), init, cx);
                });
            })
            .detach();
        });

    Ok(())
}
