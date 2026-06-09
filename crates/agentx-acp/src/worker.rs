//! One supervised agent subprocess, driven as a command-in / event-out actor.
//!
//! ACP connection futures are `!Send` and the prompt path uses
//! [`tokio::task::spawn_local`], so each agent gets its **own OS thread** running
//! a current-thread runtime and a [`LocalSet`]. The rest of the app talks to the
//! actor only through [`AgentWorker`]: domain-typed commands go in over an mpsc
//! channel, and results come back over per-command oneshots. Streamed output
//! never returns through those channels — it is published on the [`EventBus`] as
//! [`DomainEvent::SessionAppended`], so persistence and the UI observe a turn as
//! it happens.
//!
//! The ACP↔domain translation lives in [`crate::mapping`]; this module only
//! moves bytes and events. Two pieces of shared state make that work:
//! - a per-session [`StreamAccumulator`] (behind a `Mutex`, because the ACP
//!   callbacks must be `Send`) folds the chunk stream into whole events;
//! - a [`PermissionStore`] parks the agent's permission request so the dispatch
//!   loop is never blocked waiting on a human — the supervisor answers it later.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use agent_client_protocol::{
    self as acp_runtime, Agent, ByteStreams, ConnectionTo, Responder, schema as acp,
};
use anyhow::{Context as _, anyhow};
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::sync::{mpsc, oneshot};
use tokio::task::LocalSet;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use agentx_bus::EventBus;
use agentx_domain::{
    AgentConfig, AgentError, AgentId, AgentStatus, ContentBlock, DomainEvent, PermissionId,
    PermissionOutcome, ProxyConfig, SessionEvent, SessionId, SessionInit, StopReason,
};

use crate::accumulator::StreamAccumulator;
use crate::mapping::{content_blocks_to_acp, permission_outcome_to_acp, stop_reason_to_domain};

/// Permission requests the agent has raised and is blocked on, keyed by a fresh
/// id and shared across all worker threads via the supervisor.
///
/// The agent's request reaches us inside the connection's dispatch loop, which
/// must not block. So the callback *parks* the [`Responder`] here and returns
/// immediately; [`answer`](Self::answer) hands the user's decision back later,
/// from the application's runtime.
#[derive(Default)]
pub(crate) struct PermissionStore {
    pending: Mutex<HashMap<String, Responder<acp::RequestPermissionResponse>>>,
    next_id: AtomicU64,
}

impl PermissionStore {
    /// Park a responder and return the id the UI will resolve it by.
    fn park(&self, responder: Responder<acp::RequestPermissionResponse>) -> PermissionId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        self.pending
            .lock()
            .expect("permission store poisoned")
            .insert(id.clone(), responder);
        PermissionId::from(id)
    }

    /// Answer a parked request, forwarding the decision to the waiting agent.
    pub(crate) fn answer(&self, id: &str, outcome: PermissionOutcome) -> Result<(), AgentError> {
        let responder = self
            .pending
            .lock()
            .expect("permission store poisoned")
            .remove(id)
            .ok_or_else(|| AgentError::Protocol(format!("unknown permission `{id}`")))?;
        responder
            .respond(permission_outcome_to_acp(outcome))
            .map_err(|error| AgentError::Transport(error.to_string()))
    }
}

/// A handle to one running agent actor. Cheap to clone; cloning shares the same
/// command channel and status cell.
#[derive(Clone)]
pub(crate) struct AgentWorker {
    commands: mpsc::Sender<Command>,
    status: Arc<Mutex<AgentStatus>>,
}

impl AgentWorker {
    /// Spawn the agent's thread and wait until its ACP handshake completes.
    ///
    /// Resolves once the agent has initialized (status [`AgentStatus::Ready`]) or
    /// returns the failure that stopped it from starting.
    pub(crate) async fn spawn(
        agent: AgentId,
        config: AgentConfig,
        proxy: ProxyConfig,
        bus: EventBus,
        permissions: Arc<PermissionStore>,
    ) -> Result<Self, AgentError> {
        let (commands_tx, commands_rx) = mpsc::channel(32);
        let (ready_tx, ready_rx) = oneshot::channel();
        let status = Arc::new(Mutex::new(AgentStatus::Connecting));
        let thread_status = status.clone();
        let thread_agent = agent.clone();

        thread::Builder::new()
            .name(format!("acp-agent-{agent}"))
            .spawn(move || {
                let result = run_worker(
                    thread_agent.clone(),
                    config,
                    proxy,
                    bus,
                    permissions,
                    commands_rx,
                    ready_tx,
                );
                if let Err(error) = result {
                    log::error!("agent `{thread_agent}` worker exited: {error:?}");
                    *thread_status.lock().expect("status poisoned") =
                        AgentStatus::Unavailable {
                            reason: error.to_string(),
                        };
                }
            })
            .map_err(|error| AgentError::Transport(error.to_string()))?;

        match ready_rx.await {
            Ok(Ok(())) => {
                *status.lock().expect("status poisoned") = AgentStatus::Ready;
                Ok(Self {
                    commands: commands_tx,
                    status,
                })
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err(AgentError::Transport(
                "agent worker stopped before it was ready".into(),
            )),
        }
    }

    pub(crate) fn status(&self) -> AgentStatus {
        self.status.lock().expect("status poisoned").clone()
    }

    pub(crate) async fn create_session(&self, cwd: PathBuf) -> Result<SessionInit, AgentError> {
        let (respond, result) = oneshot::channel();
        self.dispatch(Command::CreateSession { cwd, respond }).await?;
        Self::recv(result).await?
    }

    pub(crate) async fn prompt(
        &self,
        session: SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<StopReason, AgentError> {
        let (respond, result) = oneshot::channel();
        self.dispatch(Command::Prompt {
            session,
            content,
            respond,
        })
        .await?;
        Self::recv(result).await?
    }

    pub(crate) async fn cancel(&self, session: SessionId) -> Result<(), AgentError> {
        let (respond, result) = oneshot::channel();
        self.dispatch(Command::Cancel { session, respond }).await?;
        Self::recv(result).await?
    }

    pub(crate) async fn shutdown(&self) {
        let _ = self.commands.send(Command::Shutdown).await;
    }

    async fn dispatch(&self, command: Command) -> Result<(), AgentError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| AgentError::Transport("agent worker is not running".into()))
    }

    async fn recv<T>(result: oneshot::Receiver<T>) -> Result<T, AgentError> {
        result
            .await
            .map_err(|_| AgentError::Transport("agent worker dropped the request".into()))
    }
}

/// A domain-typed instruction for the actor. Streamed output is *not* a reply
/// here — it flows out on the bus — so each command's oneshot carries only the
/// turn's final result.
enum Command {
    CreateSession {
        cwd: PathBuf,
        respond: oneshot::Sender<Result<SessionInit, AgentError>>,
    },
    Prompt {
        session: SessionId,
        content: Vec<ContentBlock>,
        respond: oneshot::Sender<Result<StopReason, AgentError>>,
    },
    Cancel {
        session: SessionId,
        respond: oneshot::Sender<Result<(), AgentError>>,
    },
    Shutdown,
}

/// Per-session fold state, shared between the (`Send`) notification callback and
/// the prompt task that flushes it when a turn ends.
type Accumulators = Arc<Mutex<HashMap<SessionId, StreamAccumulator>>>;

fn run_worker(
    agent: AgentId,
    config: AgentConfig,
    proxy: ProxyConfig,
    bus: EventBus,
    permissions: Arc<PermissionStore>,
    commands_rx: mpsc::Receiver<Command>,
    ready_tx: oneshot::Sender<Result<(), AgentError>>,
) -> anyhow::Result<()> {
    let runtime = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .context("build agent runtime")?;
    let local = LocalSet::new();
    runtime.block_on(local.run_until(event_loop(
        agent,
        config,
        proxy,
        bus,
        permissions,
        commands_rx,
        ready_tx,
    )))
}

async fn event_loop(
    agent: AgentId,
    config: AgentConfig,
    proxy: ProxyConfig,
    bus: EventBus,
    permissions: Arc<PermissionStore>,
    mut commands_rx: mpsc::Receiver<Command>,
    ready_tx: oneshot::Sender<Result<(), AgentError>>,
) -> anyhow::Result<()> {
    let mut child = match spawn_child(&agent, &config, &proxy) {
        Ok(child) => child,
        Err(error) => {
            let _ = ready_tx.send(Err(AgentError::Transport(error.to_string())));
            return Err(error);
        }
    };
    let outgoing = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("agent `{agent}` is missing stdin"))?
        .compat_write();
    let incoming = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("agent `{agent}` is missing stdout"))?
        .compat();
    let transport = ByteStreams::new(outgoing, incoming);

    let accumulators: Accumulators = Arc::new(Mutex::new(HashMap::new()));

    let connection = acp_runtime::Client
        .builder()
        .on_receive_request(
            {
                let permissions = permissions.clone();
                let bus = bus.clone();
                async move |request: acp::RequestPermissionRequest,
                            responder: Responder<acp::RequestPermissionResponse>,
                            _conn|
                            -> acp_runtime::Result<()> {
                    let session = SessionId::from(request.session_id.to_string());
                    let permission = permissions.park(responder);
                    bus.publish(DomainEvent::PermissionRequested {
                        permission,
                        session,
                    });
                    Ok(())
                }
            },
            acp_runtime::on_receive_request!(),
        )
        .on_receive_notification(
            {
                let accumulators = accumulators.clone();
                let bus = bus.clone();
                async move |notification: acp::SessionNotification, _conn| -> acp_runtime::Result<()> {
                    let session = SessionId::from(notification.session_id.to_string());
                    let events = accumulators
                        .lock()
                        .expect("accumulators poisoned")
                        .entry(session.clone())
                        .or_default()
                        .push(notification.update);
                    publish_events(&bus, &session, events);
                    Ok(())
                }
            },
            acp_runtime::on_receive_notification!(),
        )
        .connect_with(
            transport,
            async move |conn: ConnectionTo<Agent>| -> acp_runtime::Result<()> {
                let mut init = acp::InitializeRequest::new(acp::ProtocolVersion::V1);
                init.client_info =
                    Some(acp::Implementation::new("agentx", env!("CARGO_PKG_VERSION")));
                match conn.send_request(init).block_task().await {
                    Ok(_response) => {
                        let _ = ready_tx.send(Ok(()));
                    }
                    Err(error) => {
                        let _ = ready_tx
                            .send(Err(AgentError::Protocol(format!("initialize: {error}"))));
                        return Err(error);
                    }
                }

                while let Some(command) = commands_rx.recv().await {
                    match command {
                        Command::CreateSession { cwd, respond } => {
                            let result = conn
                                .send_request(acp::NewSessionRequest::new(cwd))
                                .block_task()
                                .await
                                .map(|response| session_init(response.session_id))
                                .map_err(|error| AgentError::Protocol(error.to_string()));
                            let _ = respond.send(result);
                        }
                        Command::Prompt {
                            session,
                            content,
                            respond,
                        } => {
                            // Run the turn off the command loop so concurrent
                            // commands (notably cancel) are still serviced.
                            let conn = conn.clone();
                            let accumulators = accumulators.clone();
                            let bus = bus.clone();
                            tokio::task::spawn_local(async move {
                                let request = acp::PromptRequest::new(
                                    session.to_string(),
                                    content_blocks_to_acp(content),
                                );
                                let outcome = conn.send_request(request).block_task().await;
                                // Flush the turn's trailing buffered events
                                // *before* reporting the stop reason, so the
                                // final message lands ahead of the Stopped event
                                // the application appends next.
                                let trailing = accumulators
                                    .lock()
                                    .expect("accumulators poisoned")
                                    .entry(session.clone())
                                    .or_default()
                                    .finish();
                                publish_events(&bus, &session, trailing);
                                let result = outcome
                                    .map(|response| stop_reason_to_domain(response.stop_reason))
                                    .map_err(|error| AgentError::Protocol(error.to_string()));
                                let _ = respond.send(result);
                            });
                        }
                        Command::Cancel { session, respond } => {
                            let result = conn
                                .send_notification(acp::CancelNotification::new(session.to_string()))
                                .map_err(|error| AgentError::Protocol(error.to_string()));
                            let _ = respond.send(result);
                        }
                        Command::Shutdown => break,
                    }
                }

                if let Ok(None) = child.try_wait() {
                    let _ = child.kill().await;
                }
                Ok(())
            },
        )
        .await;

    connection.map_err(|error| anyhow!("agent `{agent}` connection ended: {error}"))
}

/// Build the agent subprocess command for the current platform.
fn spawn_child(
    agent: &AgentId,
    config: &AgentConfig,
    proxy: &ProxyConfig,
) -> anyhow::Result<tokio::process::Child> {
    let mut command = if cfg!(target_os = "windows") {
        // On Windows the configured command is usually a `.cmd`/`.bat` shim, so
        // it must be launched through the shell rather than executed directly.
        let mut shell = tokio::process::Command::new("cmd");
        let mut args = vec!["/C".to_string(), config.command.clone()];
        args.extend(config.args.iter().cloned());
        shell.args(&args);
        shell
    } else {
        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(&config.args);
        cmd
    };

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.envs(&config.env);
    for (key, value) in proxy.env_vars() {
        command.env(key, value);
    }
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::inherit());

    command
        .spawn()
        .with_context(|| format!("spawn agent `{agent}`"))
}

/// M4a builds a session with only its id; modes/models/commands are wired when
/// the UI that surfaces them lands.
fn session_init(session_id: acp::SessionId) -> SessionInit {
    SessionInit {
        session_id: SessionId::from(session_id.to_string()),
        modes: Vec::new(),
        current_mode: None,
        models: Vec::new(),
        current_model: None,
        commands: Vec::new(),
    }
}

fn publish_events(bus: &EventBus, session: &SessionId, events: Vec<SessionEvent>) {
    for event in events {
        bus.publish(DomainEvent::SessionAppended {
            session: session.clone(),
            event,
        });
    }
}
