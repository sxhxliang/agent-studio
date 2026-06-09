//! Windowed smoke test — the M4b composition root.
//!
//! Wires the real adapters into the application's [`SessionService`] and opens a
//! [`agentx_ui::ChatView`] against a live agent from your `config.json`:
//!
//! ```text
//! cargo run -p agentx-ui --example window -- <agent>
//! ```
//!
//! Notes for this milestone:
//! - No persistence. The on-disk adapters use `tokio::fs`, which needs a Tokio
//!   runtime; GPUI's executor is not one. Config is loaded on a short-lived
//!   runtime here, and the session repository is present but never exercised. A
//!   real runtime strategy + persistence lands in the M6 composition root.
//! - No permission UI yet — requests are shown as a line; pick prompts that
//!   don't trigger tool permissions.

use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow};

use agentx_acp::AcpSupervisor;
use agentx_app::SessionService;
use agentx_bus::EventBus;
use agentx_domain::{AgentGateway, AgentId, AgentRegistry, ConfigStore, SessionRepository};
use agentx_store::{FsConfigStore, FsSessionRepository, paths};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let agent = AgentId::from(
        std::env::args()
            .nth(1)
            .context("usage: window <agent>")?,
    );

    // FsConfigStore uses tokio::fs, so load the config on a short-lived runtime
    // before GPUI takes over the main thread.
    let data_dir = paths::data_dir();
    let config = tokio::runtime::Builder::new_multi_thread()
        .build()?
        .block_on(FsConfigStore::new(paths::config_path(&data_dir)).load())
        .context("load config.json")?;
    let agent_config = config.agents.get(agent.as_str()).cloned().ok_or_else(|| {
        anyhow!(
            "agent `{agent}` not in config; available: {:?}",
            config.agents.keys().collect::<Vec<_>>()
        )
    })?;
    let proxy = config.proxy.clone();
    let cwd = std::env::current_dir()?;

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);

            let bus = EventBus::new();
            let supervisor = Arc::new(AcpSupervisor::new(bus.clone()));
            let repository: Arc<dyn SessionRepository> =
                Arc::new(FsSessionRepository::new(paths::sessions_dir(&data_dir)));
            let service = Arc::new(SessionService::new(
                supervisor.clone() as Arc<dyn AgentGateway>,
                bus.clone(),
                repository,
            ));

            // Channel ops and worker startup are runtime-agnostic, so the agent
            // can be started and the session created on GPUI's own executor.
            cx.spawn(async move |cx| {
                supervisor.set_proxy(proxy).await.ok();
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
                    agentx_ui::open_chat_window(service.clone(), bus.clone(), agent.clone(), init, cx);
                });
            })
            .detach();
        });

    Ok(())
}
