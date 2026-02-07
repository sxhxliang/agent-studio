//! A CLI interactive execution program that can talk to multiple ACP agents concurrently.
//!
//! The service reads `config.json` (configurable via `--config`) to determine
//! which agent binaries to spawn, and provides a REPL to interact with them.

use std::{
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    thread,
};

use agent_client_protocol::{self as acp, Agent as _};

use anyhow::{Context, Result, anyhow};
use log::{error, warn};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{RwLock, mpsc, oneshot},
    task::LocalSet,
};

use agentx_event_bus::{EventHub, PermissionRequestEvent, SessionUpdateEvent};
use agentx_types::{AgentProcessConfig, ProxyConfig};

use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[derive(Clone)]
pub struct AgentManager {
    agents: Arc<RwLock<HashMap<String, Arc<AgentHandle>>>>,
    permission_store: Arc<PermissionStore>,
    event_hub: EventHub,
    proxy_config: Arc<RwLock<ProxyConfig>>,
}

impl AgentManager {
    #[cfg(any(test, feature = "test-support"))]
    pub fn new(
        configs: HashMap<String, AgentProcessConfig>,
        permission_store: Arc<PermissionStore>,
        event_hub: EventHub,
        proxy_config: ProxyConfig,
    ) -> Self {
        let _ = configs;
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            permission_store,
            event_hub,
            proxy_config: Arc::new(RwLock::new(proxy_config)),
        }
    }

    pub async fn initialize(
        configs: HashMap<String, AgentProcessConfig>,
        permission_store: Arc<PermissionStore>,
        event_hub: EventHub,
        proxy_config: ProxyConfig,
    ) -> Result<Arc<Self>> {
        if configs.is_empty() {
            return Err(anyhow!("no agents defined in config"));
        }
        let proxy_config = Arc::new(RwLock::new(proxy_config));
        let manager = Arc::new(Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            permission_store,
            event_hub,
            proxy_config,
        });
        let remaining = Arc::new(AtomicUsize::new(configs.len()));

        // Initialize agents in parallel and insert them as soon as each is ready.
        for (name, cfg) in configs {
            let manager = manager.clone();
            let remaining = remaining.clone();
            smol::spawn(async move {
                if let Err(e) = manager.add_agent(name.clone(), cfg).await {
                    warn!("Failed to initialize agent '{}': {}", name, e);
                }
                if remaining.fetch_sub(1, Ordering::SeqCst) == 1
                    && manager.list_agents().await.is_empty()
                {
                    warn!("No agents could be initialized, continuing without agents");
                }
            })
            .detach();
        }

        Ok(manager)
    }

    pub async fn list_agents(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        let mut list = agents.keys().cloned().collect::<Vec<_>>();
        list.sort();
        list
    }

    /// Get the initialize response for a specific agent
    pub async fn get_agent_init_response(&self, name: &str) -> Option<acp::InitializeResponse> {
        let agents = self.agents.read().await;
        agents
            .get(name)
            .and_then(|handle| handle.get_init_response())
    }

    /// Get all agents with their initialize responses
    pub async fn list_agents_with_info(&self) -> Vec<(String, Option<acp::InitializeResponse>)> {
        let agents = self.agents.read().await;
        let mut list: Vec<_> = agents
            .iter()
            .map(|(name, handle)| (name.clone(), handle.get_init_response()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
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
            self.event_hub.clone(),
            self.proxy_config.read().await.clone(),
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

    /// Remove an agent if present, returning whether it was found.
    pub async fn remove_agent_if_present(&self, name: &str) -> Result<bool> {
        let handle = {
            let mut agents = self.agents.write().await;
            agents.remove(name)
        };

        let Some(handle) = handle else {
            return Ok(false);
        };

        // Shutdown the agent
        handle.shutdown().await?;
        log::info!("Successfully removed agent '{}'", name);
        Ok(true)
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
            self.event_hub.clone(),
            self.proxy_config.read().await.clone(),
        )
        .await?;

        // Add new agent to map
        let mut agents = self.agents.write().await;
        agents.insert(name.to_string(), Arc::new(new_handle));
        log::info!("Successfully restarted agent '{}'", name);
        Ok(())
    }

    /// Update proxy configuration and restart all agents
    pub async fn update_proxy_config(&self, proxy_config: ProxyConfig) -> Result<()> {
        log::info!("Updating proxy configuration");

        // Update stored proxy config
        *self.proxy_config.write().await = proxy_config.clone();

        // Get all agent names and configs
        let agents_to_restart: Vec<(String, AgentProcessConfig)> = {
            let agents = self.agents.read().await;
            // We need to get agent configs from somewhere - this is a limitation
            // For now, we'll just log a message
            log::info!(
                "Proxy config updated. Restart agents manually to apply changes to {} agents.",
                agents.len()
            );
            vec![]
        };

        // Note: In a production implementation, you would need to store agent configs
        // alongside the handles so they can be restarted with the new proxy settings
        Ok(())
    }

    /// Get current proxy configuration
    pub async fn get_proxy_config(&self) -> ProxyConfig {
        self.proxy_config.read().await.clone()
    }
}

pub struct AgentHandle {
    name: String,
    sender: mpsc::Sender<AgentCommand>,
    /// Initialize response from the agent
    init_response: Arc<std::sync::RwLock<Option<acp::InitializeResponse>>>,
}

impl AgentHandle {
    async fn spawn(
        name: String,
        config: AgentProcessConfig,
        permission_store: Arc<PermissionStore>,
        event_hub: EventHub,
        proxy_config: ProxyConfig,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel(32);
        let (ready_tx, ready_rx) = oneshot::channel();
        let init_response = Arc::new(std::sync::RwLock::new(None));
        let init_response_clone = init_response.clone();
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
                    event_hub,
                    receiver,
                    ready_tx,
                    init_response_clone,
                    proxy_config,
                ) {
                    error!("agent {log_name} exited with error: {:?}", err);
                }
            })
            .context("failed to spawn worker thread")?;
        let start_name = name.clone();
        ready_rx
            .await
            .map_err(|_| anyhow!("agent {start_name} failed to start"))??;

        Ok(Self {
            name,
            sender,
            init_response,
        })
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

    pub async fn resume_session(
        &self,
        request: acp::ResumeSessionRequest,
    ) -> Result<acp::ResumeSessionResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::ResumeSession {
                request: Box::new(request),
                respond: tx,
            })
            .await
            .map_err(|_| anyhow!("agent {} is not running", self.name))?;
        let result = rx
            .await
            .map_err(|_| anyhow!("agent {} stopped", self.name))?;
        result
    }

    pub async fn load_session(
        &self,
        request: acp::LoadSessionRequest,
    ) -> Result<acp::LoadSessionResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::LoadSession {
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

    pub async fn list_sessions(
        &self,
        request: acp::ListSessionsRequest,
    ) -> Result<acp::ListSessionsResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::ListSession {
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

    /// Set the session mode
    pub async fn set_session_mode(
        &self,
        request: acp::SetSessionModeRequest,
    ) -> Result<acp::SetSessionModeResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::SetSessionMode {
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

    /// Set the session model
    #[cfg(feature = "unstable")]
    pub async fn set_session_model(
        &self,
        request: acp::SetSessionModelRequest,
    ) -> Result<acp::SetSessionModelResponse> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(AgentCommand::SetSessionModel {
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

    /// Get the initialize response from the agent
    pub fn get_init_response(&self) -> Option<acp::InitializeResponse> {
        self.init_response.read().unwrap().clone()
    }
}

enum AgentCommand {
    Initialize {
        request: Box<acp::InitializeRequest>,
        respond: oneshot::Sender<Result<acp::InitializeResponse>>,
    },
    NewSession {
        request: acp::NewSessionRequest,
        respond: oneshot::Sender<Result<acp::NewSessionResponse>>,
    },
    ResumeSession {
        request: Box<acp::ResumeSessionRequest>,
        respond: oneshot::Sender<Result<acp::ResumeSessionResponse>>,
    },
    Prompt {
        request: acp::PromptRequest,
        respond: oneshot::Sender<Result<acp::PromptResponse>>,
    },
    LoadSession {
        request: acp::LoadSessionRequest,
        respond: oneshot::Sender<Result<acp::LoadSessionResponse>>,
    },
    ListSession {
        request: acp::ListSessionsRequest,
        respond: oneshot::Sender<Result<acp::ListSessionsResponse>>,
    },
    #[cfg(feature = "unstable")]
    SetSessionModel {
        request: acp::SetSessionModelRequest,
        respond: oneshot::Sender<Result<acp::SetSessionModelResponse>>,
    },
    SetSessionMode {
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
    event_hub: EventHub,
    command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<agent_client_protocol::InitializeResponse>>,
    init_response: Arc<std::sync::RwLock<Option<acp::InitializeResponse>>>,
    proxy_config: ProxyConfig,
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
                event_hub,
                command_rx,
                ready_tx,
                init_response,
                proxy_config,
            ))
            .await
    })
}

async fn agent_event_loop(
    agent_name: String,
    config: AgentProcessConfig,
    permission_store: Arc<PermissionStore>,
    event_hub: EventHub,
    mut command_rx: mpsc::Receiver<AgentCommand>,
    ready_tx: oneshot::Sender<Result<agent_client_protocol::InitializeResponse>>,
    init_response: Arc<std::sync::RwLock<Option<acp::InitializeResponse>>>,
    proxy_config: ProxyConfig,
) -> Result<()> {
    // Node.js environment validation
    let requires_nodejs = config.command.ends_with(".js")
        || config.command.ends_with(".ts")
        || config.command.contains("node")
        || config.command.contains("npx");

    if requires_nodejs {
        log::info!(
            "Agent '{}' requires Node.js, validating environment...",
            agent_name
        );

        use crate::nodejs::NodeJsChecker;
        use std::path::PathBuf;

        let custom_path = config.nodejs_path.as_ref().map(PathBuf::from);
        let nodejs_checker = NodeJsChecker::new(custom_path);

        match nodejs_checker.check_nodejs_available().await {
            Ok(result) if result.available => {
                log::info!(
                    "Node.js found for '{}': {} ({})",
                    agent_name,
                    result.path.unwrap().display(),
                    result.version.unwrap()
                );
            }
            Ok(result) => {
                let error_msg = format!(
                    "Node.js required but not found for agent '{}'.\n\n{}",
                    agent_name,
                    result.install_hint.unwrap_or_default()
                );
                log::error!("{}", error_msg);
                let _ = ready_tx.send(Err(anyhow!(error_msg.clone())));
                return Err(anyhow!(error_msg));
            }
            Err(e) => {
                let error_msg = format!("Failed to validate Node.js for '{}': {}", agent_name, e);
                log::error!("{}", error_msg);
                let _ = ready_tx.send(Err(anyhow!(error_msg.clone())));
                return Err(anyhow!(error_msg));
            }
        }
    }

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

    // Hide console window for child processes on Windows
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    // Set environment variables from config
    command.envs(&config.env);

    // Set proxy environment variables if enabled
    let proxy_envs = proxy_config.env_vars();
    if !proxy_envs.is_empty() {
        log::info!("Setting proxy env vars for agent '{}'", agent_name);
        for (key, value) in proxy_envs {
            command.env(key, value);
        }
    }

    // Set stdio for all platforms
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

    let client = GuiClient::new(agent_name.clone(), permission_store, event_hub);
    let (conn, io_task) = acp::ClientSideConnection::new(client, outgoing, incoming, |fut| {
        tokio::task::spawn_local(fut);
    });
    let conn = Rc::new(conn);

    let io_handle = tokio::task::spawn_local(async move {
        if let Err(err) = io_task.await {
            error!("agent I/O task ended: {:?}", err);
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
    log::info!(
        "Agent {} initialized  === >>> {:?}",
        agent_name,
        init_result
    );
    match init_result {
        Ok(res) => {
            // Save the initialize response
            *init_response.write().unwrap() = Some(res.clone());
            let _ = ready_tx.send(Ok(res));
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
                let result = conn.initialize(*request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::NewSession { request, respond } => {
                log::info!(
                    "Agent {} received new_session command with cwd: {:?}",
                    agent_name,
                    request.cwd
                );

                // Check if child process is still alive
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let error_msg = format!(
                            "Agent {} process exited with status: {:?}",
                            agent_name, status
                        );
                        log::error!("{}", error_msg);
                        let _ = respond.send(Err(anyhow!(error_msg)));
                        continue;
                    }
                    Ok(None) => {
                        // Process is still running, continue
                    }
                    Err(e) => {
                        log::warn!("Failed to check agent {} process status: {}", agent_name, e);
                    }
                }

                let result = conn.new_session(request).await.map_err(|err| {
                    log::error!("Agent {} new_session failed: {:?}", agent_name, err);
                    anyhow!(err)
                });

                if let Err(ref e) = result {
                    log::error!("Agent {} new_session error details: {}", agent_name, e);
                }

                let _ = respond.send(result);
            }
            AgentCommand::ResumeSession { request, respond } => {
                let result = conn
                    .resume_session(*request)
                    .await
                    .map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::Prompt { request, respond } => {
                let conn = conn.clone();
                let agent_name = agent_name.clone();
                tokio::task::spawn_local(async move {
                    log::info!("Agent {} received prompt command", agent_name);
                    let result = conn.prompt(request).await.map_err(|err| anyhow!(err));
                    let _ = respond.send(result);
                });
            }
            AgentCommand::Cancel { request, respond } => {
                log::info!("Agent {} received cancel command", agent_name);
                let result = conn.cancel(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::LoadSession { request, respond } => {
                let result = conn.load_session(request).await.map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::ListSession { request, respond } => {
                let result = conn
                    .list_sessions(request)
                    .await
                    .map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::SetSessionMode { request, respond } => {
                log::info!("Agent {} received set session mode command", agent_name);
                let result = conn
                    .set_session_mode(request)
                    .await
                    .map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            #[cfg(feature = "unstable")]
            AgentCommand::SetSessionModel { request, respond } => {
                let result = conn
                    .set_session_model(request)
                    .await
                    .map_err(|err| anyhow!(err));
                let _ = respond.send(result);
            }
            AgentCommand::Shutdown { respond } => {
                log::info!("Agent {} received shutdown command", agent_name);
                let _ = respond.send(Ok(()));
                break; // Exit the command loop to shutdown
            }
        }
    }

    log::info!("Agent {} command loop ended, cleaning up", agent_name);

    drop(conn);
    let _ = io_handle.await;

    // Check if child process is still running
    match child.try_wait() {
        Ok(Some(status)) => {
            log::warn!(
                "Agent {} process already exited with status: {:?}",
                agent_name,
                status
            );
        }
        Ok(None) => {
            // Process is still running, kill it
            log::info!("Agent {} process still running, killing it", agent_name);
            if let Err(e) = child.kill().await {
                log::error!("Failed to kill agent {} process: {}", agent_name, e);
            }
        }
        Err(e) => {
            log::error!("Failed to check agent {} process status: {}", agent_name, e);
        }
    }

    Ok(())
}

/// GUI Client that publishes session updates to the event bus
struct GuiClient {
    agent_name: String,
    permission_store: Arc<PermissionStore>,
    event_hub: EventHub,
}

impl GuiClient {
    pub fn new(
        agent_name: String,
        permission_store: Arc<PermissionStore>,
        event_hub: EventHub,
    ) -> Self {
        Self {
            agent_name,
            permission_store,
            event_hub,
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
        self.event_hub.publish_permission_request(event);

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
            agent_name: Some(self.agent_name.clone()),
            update: Arc::new(args.update),
        };

        log::debug!("[GuiClient] Publishing SessionUpdateEvent to bus");
        self.event_hub.publish_session_update(event);
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
