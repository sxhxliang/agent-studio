//! Agent Configuration Service
//!
//! This service manages agent configuration CRUD operations, validation,
//! persistence, and hot-reload functionality.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::agent::AgentManager;
use crate::core::config::{AgentProcessConfig, Config};
use crate::core::event_bus::{AgentConfigBusContainer, AgentConfigEvent};
use crate::core::services::AgentService;
use anyhow::{Context, Result, anyhow};

/// Agent Configuration Service
///
/// Manages agent configuration with CRUD operations, validation, and persistence.
pub struct AgentConfigService {
    /// Current configuration state (agent_servers + upload_dir)
    config: Arc<RwLock<Config>>,
    /// Path to the configuration file
    config_path: PathBuf,
    /// Reference to AgentManager for hot-reload operations
    agent_manager: Arc<AgentManager>,
    /// Reference to AgentService to check active sessions
    agent_service: Option<Arc<AgentService>>,
    /// Event bus for publishing configuration changes
    event_bus: AgentConfigBusContainer,
}

impl AgentConfigService {
    /// Create a new AgentConfigService
    pub fn new(
        initial_config: Config,
        config_path: PathBuf,
        agent_manager: Arc<AgentManager>,
        event_bus: AgentConfigBusContainer,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(initial_config)),
            config_path,
            agent_manager,
            agent_service: None,
            event_bus,
        }
    }

    /// Set the AgentService reference (for checking active sessions)
    pub fn set_agent_service(&mut self, agent_service: Arc<AgentService>) {
        self.agent_service = Some(agent_service);
    }

    // ========== Query Operations ==========

    /// List all configured agents
    pub async fn list_agents(&self) -> Vec<(String, AgentProcessConfig)> {
        let config = self.config.read().await;
        let mut agents: Vec<_> = config
            .agent_servers
            .iter()
            .map(|(name, cfg)| (name.clone(), cfg.clone()))
            .collect();
        agents.sort_by(|a, b| a.0.cmp(&b.0));
        agents
    }

    /// Get a specific agent's configuration
    pub async fn get_agent(&self, name: &str) -> Option<AgentProcessConfig> {
        let config = self.config.read().await;
        config.agent_servers.get(name).cloned()
    }

    /// Get the upload directory
    pub async fn get_upload_dir(&self) -> PathBuf {
        let config = self.config.read().await;
        config.upload_dir.clone()
    }

    /// Get proxy configuration (sync)
    pub fn proxy_config(&self) -> crate::core::config::ProxyConfig {
        self.config.blocking_read().proxy.clone()
    }

    /// Get the config file path
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Check if an agent has active sessions
    pub async fn has_active_sessions(&self, agent_name: &str) -> bool {
        if let Some(agent_service) = &self.agent_service {
            let sessions = agent_service.list_workspace_sessions_for_agent(agent_name);
            !sessions.is_empty()
        } else {
            false
        }
    }

    /// List all configured models
    pub async fn list_models(&self) -> Vec<(String, crate::core::config::ModelConfig)> {
        let config = self.config.read().await;
        let mut models: Vec<_> = config
            .models
            .iter()
            .map(|(name, cfg)| (name.clone(), cfg.clone()))
            .collect();
        models.sort_by(|a, b| a.0.cmp(&b.0));
        models
    }

    /// List all configured MCP servers
    pub async fn list_mcp_servers(&self) -> Vec<(String, crate::core::config::McpServerConfig)> {
        let config = self.config.read().await;
        let mut mcp_servers: Vec<_> = config
            .mcp_servers
            .iter()
            .map(|(name, cfg)| (name.clone(), cfg.clone()))
            .collect();
        mcp_servers.sort_by(|a, b| a.0.cmp(&b.0));
        mcp_servers
    }

    /// List all configured commands
    pub async fn list_commands(&self) -> Vec<(String, crate::core::config::CommandConfig)> {
        let config = self.config.read().await;
        let mut commands: Vec<_> = config
            .commands
            .iter()
            .map(|(name, cfg)| (name.clone(), cfg.clone()))
            .collect();
        commands.sort_by(|a, b| a.0.cmp(&b.0));
        commands
    }

    // ========== Validation ==========

    /// Validate that a command exists and is executable
    ///
    /// On Windows, commands are executed via `cmd /C`, so we allow any command
    /// that can be found in PATH. On Unix-like systems, we check if the file exists.
    pub fn validate_command(&self, command: &str) -> Result<()> {
        // Check if command is an absolute path
        let command_path = Path::new(command);

        if command_path.is_absolute() {
            // Absolute path - check if file exists
            if !command_path.exists() {
                return Err(anyhow!(
                    "Command path does not exist: {}",
                    command_path.display()
                ));
            }

            if !command_path.is_file() {
                return Err(anyhow!(
                    "Command path is not a file: {}",
                    command_path.display()
                ));
            }

            Ok(())
        } else {
            // Relative path or command name - try to find in PATH
            // On Windows, commands are executed via `cmd /C`, so we trust the shell
            // to find the command. We just verify it's findable via `which`.
            // On Unix-like systems, we also verify via `which`.
            if let Ok(resolved) = which::which(command) {
                log::info!("Resolved command '{}' to: {:?}", command, resolved);

                // On Windows, cmd.exe will handle .cmd, .bat, .exe files
                // so we don't need additional validation
                #[cfg(target_os = "windows")]
                {
                    Ok(())
                }

                // On Unix-like systems, verify the resolved path exists and is executable
                #[cfg(not(target_os = "windows"))]
                {
                    if resolved.exists() && resolved.is_file() {
                        Ok(())
                    } else {
                        Err(anyhow!(
                            "Resolved command path does not exist or is not a file: {}",
                            resolved.display()
                        ))
                    }
                }
            } else {
                Err(anyhow!(
                    "Command '{}' not found in PATH. Please provide an absolute path or ensure the command is in your system PATH.",
                    command
                ))
            }
        }
    }

    // ========== CRUD Operations ==========

    /// Add a new agent
    pub async fn add_agent(&self, name: String, config: AgentProcessConfig) -> Result<()> {
        // Validate command
        self.validate_command(&config.command)?;

        // Check for duplicate
        {
            let current_config = self.config.read().await;
            if current_config.agent_servers.contains_key(&name) {
                return Err(anyhow!("Agent '{}' already exists", name));
            }
        }

        // Add to AgentManager (spawns new process)
        self.agent_manager
            .add_agent(name.clone(), config.clone())
            .await?;

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config
                .agent_servers
                .insert(name.clone(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::AgentAdded {
            name: name.clone(),
            config: config.clone(),
        });

        log::info!("Successfully added agent '{}'", name);
        Ok(())
    }

    /// Update an existing agent's configuration
    pub async fn update_agent(&self, name: &str, config: AgentProcessConfig) -> Result<()> {
        // Validate command
        self.validate_command(&config.command)?;

        // Check if agent exists
        {
            let current_config = self.config.read().await;
            if !current_config.agent_servers.contains_key(name) {
                return Err(anyhow!("Agent '{}' not found", name));
            }
        }

        // Restart agent with new config (hot-reload)
        self.agent_manager
            .restart_agent(name, config.clone())
            .await?;

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config
                .agent_servers
                .insert(name.to_string(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::AgentUpdated {
            name: name.to_string(),
            config: config.clone(),
        });

        log::info!("Successfully updated agent '{}'", name);
        Ok(())
    }

    /// Remove an agent
    pub async fn remove_agent(&self, name: &str) -> Result<()> {
        // Check if agent exists
        {
            let current_config = self.config.read().await;
            if !current_config.agent_servers.contains_key(name) {
                return Err(anyhow!("Agent '{}' not found", name));
            }
        }

        // Remove from AgentManager (shuts down process)
        match self.agent_manager.remove_agent_if_present(name).await {
            Ok(true) => {}
            Ok(false) => {
                log::warn!("Agent '{}' not running; removing config only.", name);
            }
            Err(err) => return Err(err),
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.agent_servers.remove(name);
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::AgentRemoved {
            name: name.to_string(),
        });

        log::info!("Successfully removed agent '{}'", name);
        Ok(())
    }

    /// Update proxy configuration
    pub async fn update_proxy_config(&self, proxy: crate::core::config::ProxyConfig) -> Result<()> {
        let updated_config = {
            let mut config = self.config.write().await;
            config.proxy = proxy;
            config.clone()
        };

        self.save_to_file().await?;

        self.event_bus.publish(AgentConfigEvent::ConfigReloaded {
            config: Box::new(updated_config),
        });

        Ok(())
    }

    // ========== Model Configuration Operations ==========

    /// Add a new model configuration
    pub async fn add_model(
        &self,
        name: String,
        config: crate::core::config::ModelConfig,
    ) -> Result<()> {
        // Check for duplicate
        {
            let current_config = self.config.read().await;
            if current_config.models.contains_key(&name) {
                return Err(anyhow!("Model '{}' already exists", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.models.insert(name.clone(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::ModelAdded {
            name: name.clone(),
            config: config.clone(),
        });

        log::info!("Successfully added model '{}'", name);
        Ok(())
    }

    /// Update an existing model configuration
    pub async fn update_model(
        &self,
        name: &str,
        config: crate::core::config::ModelConfig,
    ) -> Result<()> {
        // Check if model exists
        {
            let current_config = self.config.read().await;
            if !current_config.models.contains_key(name) {
                return Err(anyhow!("Model '{}' not found", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config
                .models
                .insert(name.to_string(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::ModelUpdated {
            name: name.to_string(),
            config: config.clone(),
        });

        log::info!("Successfully updated model '{}'", name);
        Ok(())
    }

    /// Remove a model configuration
    pub async fn remove_model(&self, name: &str) -> Result<()> {
        // Check if model exists
        {
            let current_config = self.config.read().await;
            if !current_config.models.contains_key(name) {
                return Err(anyhow!("Model '{}' not found", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.models.remove(name);
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::ModelRemoved {
            name: name.to_string(),
        });

        log::info!("Successfully removed model '{}'", name);
        Ok(())
    }

    // ========== MCP Server Configuration Operations ==========

    /// Add a new MCP server configuration
    pub async fn add_mcp_server(
        &self,
        name: String,
        config: crate::core::config::McpServerConfig,
    ) -> Result<()> {
        // Check for duplicate
        {
            let current_config = self.config.read().await;
            if current_config.mcp_servers.contains_key(&name) {
                return Err(anyhow!("MCP server '{}' already exists", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config
                .mcp_servers
                .insert(name.clone(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::McpServerAdded {
            name: name.clone(),
            config: config.clone(),
        });

        log::info!("Successfully added MCP server '{}'", name);
        Ok(())
    }

    /// Update an existing MCP server configuration
    pub async fn update_mcp_server(
        &self,
        name: &str,
        config: crate::core::config::McpServerConfig,
    ) -> Result<()> {
        // Check if MCP server exists
        {
            let current_config = self.config.read().await;
            if !current_config.mcp_servers.contains_key(name) {
                return Err(anyhow!("MCP server '{}' not found", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config
                .mcp_servers
                .insert(name.to_string(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::McpServerUpdated {
            name: name.to_string(),
            config: config.clone(),
        });

        log::info!("Successfully updated MCP server '{}'", name);
        Ok(())
    }

    /// Remove an MCP server configuration
    pub async fn remove_mcp_server(&self, name: &str) -> Result<()> {
        // Check if MCP server exists
        {
            let current_config = self.config.read().await;
            if !current_config.mcp_servers.contains_key(name) {
                return Err(anyhow!("MCP server '{}' not found", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.mcp_servers.remove(name);
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::McpServerRemoved {
            name: name.to_string(),
        });

        log::info!("Successfully removed MCP server '{}'", name);
        Ok(())
    }

    // ========== Command Configuration Operations ==========

    /// Add a new command configuration
    pub async fn add_command(
        &self,
        name: String,
        config: crate::core::config::CommandConfig,
    ) -> Result<()> {
        // Check for duplicate
        {
            let current_config = self.config.read().await;
            if current_config.commands.contains_key(&name) {
                return Err(anyhow!("Command '{}' already exists", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.commands.insert(name.clone(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::CommandAdded {
            name: name.clone(),
            config: config.clone(),
        });

        log::info!("Successfully added command '{}'", name);
        Ok(())
    }

    /// Update an existing command configuration
    pub async fn update_command(
        &self,
        name: &str,
        config: crate::core::config::CommandConfig,
    ) -> Result<()> {
        // Check if command exists
        {
            let current_config = self.config.read().await;
            if !current_config.commands.contains_key(name) {
                return Err(anyhow!("Command '{}' not found", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config
                .commands
                .insert(name.to_string(), config.clone());
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::CommandUpdated {
            name: name.to_string(),
            config: config.clone(),
        });

        log::info!("Successfully updated command '{}'", name);
        Ok(())
    }

    /// Remove a command configuration
    pub async fn remove_command(&self, name: &str) -> Result<()> {
        // Check if command exists
        {
            let current_config = self.config.read().await;
            if !current_config.commands.contains_key(name) {
                return Err(anyhow!("Command '{}' not found", name));
            }
        }

        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.commands.remove(name);
        }

        // Save to file
        self.save_to_file().await?;

        // Publish event
        self.event_bus.publish(AgentConfigEvent::CommandRemoved {
            name: name.to_string(),
        });

        log::info!("Successfully removed command '{}'", name);
        Ok(())
    }

    /// Restart an agent with its current configuration
    pub async fn restart_agent(&self, name: &str) -> Result<()> {
        let config = {
            let current_config = self.config.read().await;
            current_config
                .agent_servers
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow!("Agent '{}' not found", name))?
        };

        // Validate command before restart
        self.validate_command(&config.command)?;

        // Restart agent
        self.agent_manager
            .restart_agent(name, config.clone())
            .await?;

        log::info!("Successfully restarted agent '{}'", name);
        Ok(())
    }

    /// Set the upload directory
    pub async fn set_upload_dir(&self, path: PathBuf) -> Result<()> {
        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.upload_dir = path.clone();
        }

        // Save to file
        self.save_to_file().await?;

        log::info!("Successfully updated upload_dir to: {:?}", path);
        Ok(())
    }

    /// Update system prompts configuration
    pub async fn update_system_prompts(
        &self,
        system_prompts: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        // Update config
        {
            let mut current_config = self.config.write().await;
            current_config.system_prompts = system_prompts.clone();
        }

        // Save to file
        self.save_to_file().await?;

        // Publish config reload event
        let config = self.config.read().await;
        self.event_bus.publish(AgentConfigEvent::ConfigReloaded {
            config: Box::new(config.clone()),
        });

        log::info!("Successfully updated system prompts");
        Ok(())
    }

    // ========== Persistence ==========

    /// Save configuration to file
    async fn save_to_file(&self) -> Result<()> {
        let config = self.config.read().await;

        // Create backup before saving
        if self.config_path.exists() {
            let backup_path = self.config_path.with_extension("json.backup");
            if let Err(e) = std::fs::copy(&self.config_path, &backup_path) {
                log::warn!("Failed to create backup: {}", e);
            }
        }

        // Serialize config
        let json =
            serde_json::to_string_pretty(&*config).context("Failed to serialize configuration")?;

        // Write to file (atomic write using temp file)
        let temp_path = self.config_path.with_extension("json.tmp");
        std::fs::write(&temp_path, json).context("Failed to write configuration to temp file")?;

        std::fs::rename(&temp_path, &self.config_path)
            .context("Failed to replace configuration file")?;

        log::info!("Configuration saved to: {:?}", self.config_path);
        Ok(())
    }

    /// Reload configuration from file
    pub async fn reload_from_file(&self) -> Result<()> {
        // Read file
        let json = std::fs::read_to_string(&self.config_path)
            .with_context(|| format!("Failed to read config file: {:?}", self.config_path))?;

        // Parse config
        let new_config: Config =
            serde_json::from_str(&json).context("Failed to parse configuration file")?;

        // Update internal config
        {
            let mut config = self.config.write().await;
            *config = new_config.clone();
        }

        // Publish reload event with full config
        self.event_bus.publish(AgentConfigEvent::ConfigReloaded {
            config: Box::new(new_config),
        });

        log::info!("Configuration reloaded from: {:?}", self.config_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::core::config::ProxyConfig;

    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_validate_command_absolute_path() {
        let service = create_test_service();

        // Test with non-existent absolute path
        let result = service.validate_command("/nonexistent/command");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_command_in_path() {
        let service = create_test_service();

        // Test with common system command
        #[cfg(target_os = "windows")]
        let result = service.validate_command("cmd");

        #[cfg(not(target_os = "windows"))]
        let result = service.validate_command("ls");

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_duplicate_agent() {
        let _service = create_test_service();

        let _config = AgentProcessConfig {
            command: if cfg!(target_os = "windows") {
                "cmd".to_string()
            } else {
                "ls".to_string()
            },
            args: vec![],
            env: HashMap::new(),
            nodejs_path: None,
        };

        // First add should work (would fail without actual AgentManager, but tests structure)
        // Second add should fail
        // Note: This test requires mocking AgentManager for full coverage
    }

    fn create_test_service() -> AgentConfigService {
        // Create test dependencies
        let config = Config {
            agent_servers: HashMap::new(),
            upload_dir: PathBuf::from("."),
            models: HashMap::new(),
            mcp_servers: HashMap::new(),
            commands: HashMap::new(),
            system_prompts: HashMap::new(),
            tool_call_preview_max_lines: 10,
            proxy: ProxyConfig::default(),
        };

        let event_bus = AgentConfigBusContainer::new();
        let config_path = std::env::temp_dir().join("test-config.json");

        // Mock agent manager for testing
        let agent_manager = Arc::new(crate::core::agent::AgentManager::new(
            HashMap::new(),
            Arc::new(Default::default()),
            crate::core::event_bus::SessionUpdateBusContainer::new(),
            crate::core::event_bus::PermissionBusContainer::new(),
            ProxyConfig::default(),
        ));

        AgentConfigService::new(config, config_path, agent_manager, event_bus)
    }
}
