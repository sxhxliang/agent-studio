//! A CLI interactive execution program that can talk to multiple ACP agents concurrently.
//!
//! The service reads `config.json` (configurable via `--config`) to determine
//! which agent binaries to spawn, and provides a REPL to interact with them.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
};

use agent_client_protocol::{self as acp, Agent as _};
use anyhow::{anyhow, Context, Result};
use log::{error, warn};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{mpsc, oneshot, RwLock},
    task::LocalSet,
};

use crate::config::AgentProcessConfig;
use crate::permission_bus::{PermissionBusContainer, PermissionRequestEvent};
use crate::session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
use agent_client_protocol_schema as schema;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Clone)]
pub struct AgentManager {
    agents: HashMap<String, Arc<AgentHandle>>,
}

impl AgentManager {
    pub async fn initialize(
        configs: HashMap<String, AgentProcessConfig>,
        permission_store: Arc<PermissionStore>,
        session_bus: SessionUpdateBusContainer,
        permission_bus: PermissionBusContainer,
    ) -> Result<Arc<Self>> {
        if configs.is_empty() {
            return Err(anyhow!("no agents defined in config"));
        }
        let mut agents = HashMap::new();
        for (name, cfg) in configs {
            match AgentHandle::spawn(
                name.clone(),
                cfg,
                permission_store.clone(),
                session_bus.clone(),
                permission_bus.clone(),
            )
            .await
            {
                Ok(handle) => {
                    agents.insert(name, Arc::new(handle));
                }
                Err(e) => {
                    warn!("Failed to initialize agent '{}': {}", name, e);
                }
            }
        }
        if agents.is_empty() {
            warn!("No agents could be initialized, continuing without agents");
        }
        Ok(Arc::new(Self { agents }))
    }

    pub fn list_agents(&self) -> Vec<String> {
        let mut list = self.agents.keys().cloned().collect::<Vec<_>>();
        list.sort();
        list
    }

    pub fn get(&self, name: &str) -> Option<Arc<AgentHandle>> {
        self.agents.get(name).cloned()
    }
}

pub struct AgentHandle {
    name: String,
    sender: mpsc::Sender<AgentCommand>,
}

impl AgentHandle {
    async fn spawn(
        name: String,
        config: AgentProcessConfig,
        permission_store: Arc<PermissionStore>,
        session_bus: SessionUpdateBusContainer,
        permission_bus: PermissionBusContainer,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(32);
        let (ready_tx, ready_rx) = oneshot::channel();
        let thread_name = format!("agent-worker-{name}");
        let worker_name = name.clone();
        thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let log_name = worker_name.clone();
                if let Err(err) = run_agent_worker(
                    worker_name,
                    config,
                    permission_store,
                    session_bus,
                    permission_bus,
                    receiver,
                    ready_tx,
                ) {
                    error!("agent {log_name} exited with error: {:?}", err);
                }
            })
            .context("failed to spawn worker thread")?;
        let start_name = name.clone();
        ready_rx
            .await
            .map_err(|_| anyhow!("agent {start_name} failed to start"))??;

        Ok(Self { name, sender })
    }

    pub async fn new_session(
        &self,
        request: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::NewSession {
                request,
                respond: tx,
            })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        let result = rx
            .await
            .map_err(|_| anyhow!("agent {} stopped", self.name))?;
        result
    }

    pub async fn prompt(&self, request: acp::PromptRequest) -> Result<acp::PromptResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::Prompt {
                request,
                respond: tx,
            })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        let result = rx
            .await
            .map_err(|_| anyhow!("agent {} stopped", self.name))?;
        result
    }
}

enum AgentCommand {
    Initialize {
        request: acp::InitializeRequest,
        respond: oneshot::Sender<Result<acp::InitializeResponse>>,
    },
    NewSession {
        request: acp::NewSessionRequest,
        respond: oneshot::Sender<Result<acp::NewSessionResponse>>,
    },
    Prompt {
        request: acp::PromptRequest,
        respond: oneshot::Sender<Result<acp::PromptResponse>>,
    },
}

fn run_agent_worker(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,
    permission_bus: PermissionBusContainer,
    command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let runtime = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build runtime")?;

    runtime.block_on(async move {
        let local_set = LocalSet::new();
        local_set
            .run_until(agent_event_loop(
                agent_name,
                config,
                permission_store,
                session_bus,
                permission_bus,
                command_rx,
                ready_tx,
            ))
            .await
    })
}

async fn agent_event_loop(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,
    permission_bus: PermissionBusContainer,
    mut command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let mut command = if cfg!(target_os = "windows") {
        let mut shell_cmd = tokio::process::Command::new("cmd");
        let mut full_args = vec!["/C".to_string(), config.command.clone()];
        full_args.extend(config.args.iter().cloned());
        shell_cmd.args(&full_args);
        shell_cmd
    } else {
        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(&config.args);
        cmd
    };
    // let mut command = tokio::process::Command::new(&config.command);
    command.args(&config.args);
    command.envs(&config.env);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::inherit());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn agent {agent_name}"))?;
    let outgoing = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("agent {agent_name} missing stdin"))?
        .compat_write();
    let incoming = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("agent {agent_name} missing stdout"))?
        .compat();

    let client = GuiClient::new(
        agent_name.clone(),
        permission_store,
        session_bus,
        permission_bus,
    );
    let (conn, io_task) = acp::ClientSideConnection::new(client, outgoing, incoming, |fut| {
        tokio::task::spawn_local(fut);
    });

    let io_handle = tokio::task::spawn_local(async move {
        if let Err(err) = io_task.await {
            warn!("agent I/O task ended: {:?}", err);
        }
    });

    let init_result = conn
        .initialize(acp::InitializeRequest {
            protocol_version: acp::V1,
            client_capabilities: acp::ClientCapabilities::default(),
            client_info: Some(acp::Implementation {
                name: "cli-client".into(),
                title: Some("CLI Client".into()),
                version: env!("CARGO_PKG_VERSION").into(),
            }),
            meta: None,
        })
        .await;

    match init_result {
        Ok(_) => {
            let _ = ready_tx.send(Ok(()));
        }
        Err(err) => {
            let message = format!("failed to initialize agent {agent_name}: {:?}", err);
            let _ = ready_tx.send(Err(anyhow!(message.clone())));
            return Err(anyhow!(message));
        }
    }

    while let Some(command) = command_rx.recv().await {
        match command {
            AgentCommand::Initialize { request, respond } => {
                let result = conn.initialize(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::NewSession { request, respond } => {
                let result = conn.new_session(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::Prompt { request, respond } => {
                let result = conn.prompt(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
        }
    }

    drop(conn);
    let _ = io_handle.await;
    if child.id().is_some() {
        let _ = child.kill().await;
    }
    Ok(())
}

/// Convert from agent_client_protocol SessionUpdate to agent_client_protocol_schema SessionUpdate
///
/// Uses JSON serialization/deserialization as a bridge between the two incompatible versions
fn convert_session_update(update: &acp::SessionUpdate) -> schema::SessionUpdate {
    // Serialize the protocol version to JSON
    let json_value =
        serde_json::to_value(update).expect("Failed to serialize SessionUpdate from protocol");

    // Deserialize into the schema version
    serde_json::from_value(json_value)
        .expect("Failed to deserialize SessionUpdate to schema version")
}

/// GUI Client that publishes session updates to the event bus
struct GuiClient {
    agent_name: String,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,
    permission_bus: PermissionBusContainer,
}

impl GuiClient {
    pub fn new(
        agent_name: String,
        permission_store: Arc<PermissionStore>,
        session_bus: SessionUpdateBusContainer,
        permission_bus: PermissionBusContainer,
    ) -> Self {
        Self {
            agent_name,
            permission_store,
            session_bus,
            permission_bus,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for GuiClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let permission_id = self
            .permission_store
            .add(self.agent_name.clone(), args.session_id.to_string(), tx)
            .await;

        // Publish permission request event to the permission bus
        let event = PermissionRequestEvent {
            permission_id: permission_id.clone(),
            session_id: args.session_id.to_string(),
            agent_name: self.agent_name.clone(),
            tool_call: args.tool_call,
            options: args.options,
        };

        log::info!(
            "[GuiClient] Publishing permission request {} to permission bus for session '{}'",
            permission_id,
            event.session_id
        );
        self.permission_bus.publish(event);

        rx.await.map_err(|_| {
            acp::Error::internal_error().with_data("permission request channel closed")
        })
    }

    async fn write_text_file(
        &self,
        _args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn read_text_file(
        &self,
        _args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn create_terminal(
        &self,
        _args: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse, acp::Error> {
        Err(acp::Error::method_not_found())
    }

    async fn terminal_output(
        &self,
        _args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn release_terminal(
        &self,
        _args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn wait_for_terminal_exit(
        &self,
        _args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn kill_terminal_command(
        &self,
        _args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<(), acp::Error> {
        log::info!(
            "[GuiClient] Received session_notification from agent '{}' for session '{}'",
            self.agent_name,
            args.session_id
        );

        // Publish event to the session bus
        let event = SessionUpdateEvent {
            session_id: args.session_id.to_string(),
            update: Arc::new(convert_session_update(&args.update)),
        };

        log::info!("[GuiClient] Publishing SessionUpdateEvent to bus");
        self.session_bus.publish(event);
        Ok(())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
        Ok(())
    }
}

pub struct PendingPermission {
    agent: String,
    session_id: String,
    responder: oneshot::Sender<acp::RequestPermissionResponse>,
}

#[derive(Default)]
pub struct PermissionStore {
    pending: RwLock<HashMap<String, PendingPermission>>,
    next_id: AtomicU64,
}

impl PermissionStore {
    pub async fn add(
        &self,
        agent: String,
        session_id: String,
        responder: oneshot::Sender<acp::RequestPermissionResponse>,
    ) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
        self.pending.write().await.insert(
            id.clone(),
            PendingPermission {
                agent,
                session_id,
                responder,
            },
        );
        id
    }

    /// Respond to a permission request with the given response
    pub async fn respond(
        &self,
        id: &str,
        response: acp::RequestPermissionResponse,
    ) -> anyhow::Result<()> {
        let pending = self.remove(id).await;
        if let Some(pending) = pending {
            pending
                .responder
                .send(response)
                .map_err(|_| anyhow!("Failed to send permission response - receiver dropped"))
        } else {
            Err(anyhow!("Permission request ID not found: {}", id))
        }
    }

    async fn remove(&self, id: &str) -> Option<PendingPermission> {
        self.pending.write().await.remove(id)
    }
}
