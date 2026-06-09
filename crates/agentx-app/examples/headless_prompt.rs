//! Headless smoke test for the rewritten ACP stack.
//!
//! This is a miniature composition root: it wires the real adapters
//! ([`AcpSupervisor`], [`FsConfigStore`], [`FsSessionRepository`]) into the
//! application's [`SessionService`] and [`PersistenceProjector`], starts one
//! agent from your `config.json`, sends a single prompt, and prints the
//! [`DomainEvent`]s that stream back.
//!
//! ```text
//! cargo run -p agentx-app --example headless_prompt -- <agent> [prompt words...]
//! ```
//!
//! With no prompt words a default greeting is sent. Permission requests are
//! auto-cancelled so the run never blocks waiting for a UI that does not exist
//! yet.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};

use agentx_acp::AcpSupervisor;
use agentx_app::{PersistenceProjector, SessionService};
use agentx_bus::EventBus;
use agentx_domain::{
    AgentGateway, AgentId, AgentRegistry, ConfigStore, ContentBlock, DomainEvent, PermissionOutcome,
    SessionRepository,
};
use agentx_store::{FsConfigStore, FsSessionRepository, paths};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (agent, prompt) = parse_args()?;

    // ---- load the real on-disk configuration ----
    let data_dir = paths::data_dir();
    let config = FsConfigStore::new(paths::config_path(&data_dir))
        .load()
        .await
        .context("load config.json")?;
    let agent_config = config
        .agents
        .get(agent.as_str())
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "agent `{agent}` not in config; available: {:?}",
                config.agents.keys().collect::<Vec<_>>()
            )
        })?;

    // ---- compose the runtime ----
    let bus = EventBus::new();
    let repository: Arc<dyn SessionRepository> =
        Arc::new(FsSessionRepository::new(paths::sessions_dir(&data_dir)));
    let supervisor = Arc::new(AcpSupervisor::new(bus.clone()));

    spawn_projector(bus.clone(), repository.clone());
    spawn_event_printer(bus.clone(), supervisor.clone());

    supervisor.set_proxy(config.proxy.clone()).await.ok();
    println!("starting agent `{agent}` ...");
    supervisor
        .add_agent(agent.clone(), agent_config)
        .await
        .with_context(|| format!("start agent `{agent}`"))?;
    println!("agent `{agent}` is {:?}\n", supervisor.status(&agent));

    let service = SessionService::new(
        supervisor.clone() as Arc<dyn AgentGateway>,
        bus.clone(),
        repository,
    );

    let cwd = std::env::current_dir()?;
    let session = service
        .get_or_create_session(&agent, &cwd, &[])
        .await
        .context("create session")?;
    println!("session: {session}");
    println!("prompt:  {prompt}\n--- streaming domain events ---");

    let reason = service
        .send_message(&session, vec![ContentBlock::text(prompt)])
        .await
        .context("send prompt")?;
    println!("--- turn finished: {reason:?} ---");

    // Give the async subscribers a moment to drain the final events.
    tokio::time::sleep(Duration::from_millis(250)).await;
    Ok(())
}

fn parse_args() -> Result<(AgentId, String)> {
    let mut args = std::env::args().skip(1);
    let agent = args
        .next()
        .context("usage: headless_prompt <agent> [prompt words...]")?;
    let rest: Vec<String> = args.collect();
    let prompt = if rest.is_empty() {
        "Say hello in one short sentence.".to_string()
    } else {
        rest.join(" ")
    };
    Ok((AgentId::from(agent), prompt))
}

/// Persist every appended timeline event, exactly as the real app will.
fn spawn_projector(bus: EventBus, repository: Arc<dyn SessionRepository>) {
    let projector = PersistenceProjector::new(repository);
    let mut rx = bus.subscribe::<DomainEvent>();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Err(error) = projector.project(&event).await {
                log::warn!("persist failed: {error}");
            }
        }
    });
}

/// Print each domain event and auto-cancel permission requests so the headless
/// run terminates instead of waiting on a human.
fn spawn_event_printer(bus: EventBus, supervisor: Arc<AcpSupervisor>) {
    let mut rx = bus.subscribe::<DomainEvent>();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                DomainEvent::SessionAppended { event, .. } => println!("  event:  {event:?}"),
                DomainEvent::SessionStatusChanged { status, .. } => {
                    println!("  status: {status:?}")
                }
                DomainEvent::AgentStatusChanged { agent, status } => {
                    println!("  agent {agent}: {status:?}")
                }
                DomainEvent::PermissionRequested { request } => {
                    println!(
                        "  permission {} requested ({}) — auto-cancelling",
                        request.id, request.tool_call.title
                    );
                    let _ = supervisor
                        .resolve_permission(
                            &request.session,
                            request.id.as_str(),
                            PermissionOutcome::Cancelled,
                        )
                        .await;
                }
                DomainEvent::SessionCommandsChanged { commands, .. } => {
                    let names = commands
                        .iter()
                        .map(|command| format!("/{}", command.name))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("  commands: {names}");
                }
                DomainEvent::ConfigChanged => {}
            }
        }
    });
}
