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

use crate::core::config::AgentProcessConfig;
use crate::core::event_bus::{
    permission_bus::{PermissionBusContainer, PermissionRequestEvent},
    session_bus::{SessionUpdateBusContainer, SessionUpdateEvent},
};

use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Clone)]
pub struct AgentManager {
    agents: Arc<RwLock<HashMap<String, Arc<AgentHandle>>>>,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,
    permission_bus: PermissionBusContainer,
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
        Ok(Arc::new(Self {
            agents: Arc::new(RwLock::new(agents)),
            permission_store,
            session_bus,
            permission_bus,
        }))
    }

    pub async fn list_agents(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        let mut list = agents.keys().cloned().collect::<Vec<_>>();
        list.sort();
        list
    }

    pub async fn get(&self, name: &str) -> Option<Arc<AgentHandle>> {
        let agents = self.agents.read().await;
        agents.get(name).cloned()
    }

    /// Add a new agent to the manager
    pub async fn add_agent(&self, name: String, config: AgentProcessConfig) -> Result<()> {
        // Check if agent already exists
        {
            let agents = self.agents.read().await;
            if agents.contains_key(&name) {
                return Err(anyhow!("Agent '{}' already exists", name));
            }
        }

        // Spawn new agent
        let handle = AgentHandle::spawn(
            name.clone(),
            config,
            self.permission_store.clone(),
            self.session_bus.clone(),
            self.permission_bus.clone(),
        )
        .await?;

        // Add to agents map
        let mut agents = self.agents.write().await;
        agents.insert(name.clone(), Arc::new(handle));
        log::info!("Successfully added agent '{}'", name);
        Ok(())
    }

    /// Remove an agent from the manager
    pub async fn remove_agent(&self, name: &str) -> Result<()> {
        let handle = {
            let mut agents = self.agents.write().await;
            agents
                .remove(name)
                .ok_or_else(|| anyhow!("Agent '{}' not found", name))?
        };

        // Shutdown the agent
        handle.shutdown().await?;
        log::info!("Successfully removed agent '{}'", name);
        Ok(())
    }

    /// Restart an agent with new configuration
    pub async fn restart_agent(&self, name: &str, config: AgentProcessConfig) -> Result<()> {
        // Remove old agent
        let old_handle = {
            let mut agents = self.agents.write().await;
            agents
                .remove(name)
                .ok_or_else(|| anyhow!("Agent '{}' not found", name))?
        };

        // Shutdown old agent
        if let Err(e) = old_handle.shutdown().await {
            warn!("Failed to shutdown old agent '{}': {}", name, e);
        }

        // Spawn new agent
        let new_handle = AgentHandle::spawn(
            name.to_string(),
            config,
            self.permission_store.clone(),
            self.session_bus.clone(),
            self.permission_bus.clone(),
        )
        .await?;

        // Add new agent to map
        let mut agents = self.agents.write().await;
        agents.insert(name.to_string(), Arc::new(new_handle));
        log::info!("Successfully restarted agent '{}'", name);
        Ok(())
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

    /// Cancel an ongoing session operation
    pub async fn cancel(&self, session_id: String) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let request = acp::CancelNotification::new(acp::SessionId::from(session_id));
        self.sender
            .send(AgentCommand::Cancel {
                request,
                respond: tx,
            })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        rx.await
            .map_err(|_| anyhow!("agent {} cancel channel closed", self.name))?
    }

    /// Shutdown the agent gracefully
    pub async fn shutdown(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::Shutdown { respond: tx })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        rx.await
            .map_err(|_| anyhow!("agent {} shutdown channel closed", self.name))?
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
    LoadSession {
        request: acp::LoadSessionRequest,
        respond: oneshot::Sender<Result<acp::LoadSessionResponse>>,
    },
    #[cfg(feature = "unstable")]
    SetSessionModel{
        request: acp::SetSessionModelRequest,
        respond: oneshot::Sender<Result<acp::SetSessionModelResponse>>,
    },
    SetSessionMode{
        request: acp::SetSessionModeRequest,
        respond: oneshot::Sender<Result<acp::SetSessionModeResponse>>,
    },
    Cancel {
        request: acp::CancelNotification,
        respond: oneshot::Sender<Result<()>>,
    },
    Shutdown {
        respond: oneshot::Sender<Result<()>>,
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

    // Set environment variables and stdio for all platforms
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
    // Assuming `InitializeRequest` and `Implementation` have `new` methods or implement `Default`
    let version = env!("CARGO_PKG_VERSION").to_string();
    let mut client_info = acp::Implementation::new("agentx", version);
    client_info.name = "cli-client".into();
    client_info.title = Some("CLI Client".into());
    client_info.version = env!("CARGO_PKG_VERSION").into();

    let mut init_request = acp::InitializeRequest::new(acp::ProtocolVersion::V1);
    init_request.client_capabilities = acp::ClientCapabilities::default();
    init_request.client_info = Some(client_info);
    init_request.meta = None;
    let init_result = conn.initialize(init_request).await;

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
                log::info!("Agent {} received prompt command", agent_name);
                let result = conn.prompt(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::Cancel { request, respond } => {
                log::info!("Agent {} received cancel command", agent_name);
                let result = conn.cancel(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::LoadSession { request, respond } => {
                let result = conn.load_session(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            },
            AgentCommand::SetSessionMode { request, respond } => {
                log::info!("Agent {} received set session mode command", agent_name);
                let result = conn.set_session_mode(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            #[cfg(feature = "unstable")]
            AgentCommand::SetSessionModel { request, respond } => {
                let result = conn.set_session_model(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::Shutdown { respond } => {
                log::info!("Agent {} received shutdown command", agent_name);
                let _ = respond.send(Ok(()));
                break; // Exit the command loop to shutdown
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

        log::debug!(
            "[GuiClient] Publishing permission request {} to permission bus for session '{}'",
            permission_id,
            event.session_id
        );
        self.permission_bus.publish(event);

        rx.await
            .map_err(|_| acp::Error::internal_error().data("permission request channel closed"))
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
        log::debug!(
            "[GuiClient] Received session_notification from agent '{}' for session '{}, {:?}'",
            self.agent_name,
            args.session_id,
            args.update
        );

        // Publish event to the session bus
        let event = SessionUpdateEvent {
            session_id: args.session_id.to_string(),
            update: Arc::new(args.update),
        };

        log::debug!("[GuiClient] Publishing SessionUpdateEvent to bus");
        self.session_bus.publish(event);
        Ok(())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        log::debug!("[GuiClient] ext_method called");
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
        log::debug!("[GuiClient] Received ExtNotification");
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
