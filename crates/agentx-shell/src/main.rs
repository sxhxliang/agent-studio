//! The rewrite's entry point — the composition root.
//!
//! Wires the driven adapters into the application's [`SessionService`] and opens
//! the dock workspace ([`agentx_ui::open_workspace_window`]), where the welcome
//! panel starts a task and hands it to the conversation panel:
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
use agentx_app::{
    ConfigService, FileService, PersistenceProjector, SessionService, WorkspaceService,
};
use agentx_bus::EventBus;
use agentx_domain::{
    AgentGateway, AgentRegistry, Config, ConfigStore, DomainEvent, SessionRepository,
    WorkspaceFiles, WorkspaceRepository,
};
use agentx_store::{
    FsConfigStore, FsSessionRepository, FsWorkspaceFiles, FsWorkspaceRepository, paths,
};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_dir = paths::data_dir();
    let config_path = paths::config_path(&data_dir);
    let sessions_dir = paths::sessions_dir(&data_dir);
    let workspaces_path = paths::workspaces_path(&data_dir);
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
                repository.clone(),
            ));
            let registry = supervisor as Arc<dyn AgentRegistry>;

            // Workspaces + tasks: persisted to their own JSON file, with task
            // last-message previews derived from the session timelines.
            let workspace_repo: Arc<dyn WorkspaceRepository> =
                Arc::new(FsWorkspaceRepository::new(workspaces_path));
            let workspace_service = Arc::new(WorkspaceService::new(
                workspace_repo,
                repository,
                bus.clone(),
            ));

            // Configuration read/save use-case, shared with the settings panel.
            let config_service = Arc::new(ConfigService::new(
                Arc::new(FsConfigStore::new(config_path)) as Arc<dyn ConfigStore>,
                bus.clone(),
            ));

            // File listing for the composer's `@`-mention picker.
            let file_service = Arc::new(FileService::new(
                Arc::new(FsWorkspaceFiles::new()) as Arc<dyn WorkspaceFiles>
            ));

            cx.spawn(async move |cx| {
                // Blocking `std::fs` is runtime-agnostic, so loading config on
                // GPUI's executor is fine (it's a one-off, KB-sized read). A
                // missing or malformed file is not fatal — the launcher just
                // shows no agents until the user fixes `config.json`.
                let config = config_service.load().await.unwrap_or_else(|error| {
                    log::error!("load config.json: {error}");
                    Config::default()
                });

                let _ = cx.update(|cx| {
                    agentx_ui::open_workspace_window(
                        registry,
                        service,
                        workspace_service,
                        config_service,
                        file_service,
                        bus,
                        config,
                        cwd,
                        cx,
                    );
                });
            })
            .detach();
        });

    Ok(())
}
