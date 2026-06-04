//! Filesystem implementation of [`ConfigStore`].
//!
//! The on-disk JSON keeps the legacy schema (`agent_servers`, the `mcpServers`
//! alias, etc.) so existing user config files keep working. [`ConfigDto`] is the
//! anti-corruption boundary: the rest of the system only ever sees the clean
//! domain [`Config`].

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use agentx_domain::{
    AgentConfig, CommandConfig, Config, ConfigStore, DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES,
    McpServerConfig, ModelConfig, ProxyConfig, StoreError,
};

use crate::{io_err, serde_err};

/// Reads and writes the application configuration as a JSON file.
pub struct FsConfigStore {
    path: PathBuf,
}

impl FsConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl ConfigStore for FsConfigStore {
    async fn load(&self) -> Result<Config, StoreError> {
        let data = tokio::fs::read_to_string(&self.path).await.map_err(io_err)?;
        let dto: ConfigDto = serde_json::from_str(&data).map_err(serde_err)?;
        Ok(dto.into())
    }

    async fn save(&self, config: &Config) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(io_err)?;
        }
        let dto = ConfigDto::from(config);
        let data = serde_json::to_string_pretty(&dto).map_err(serde_err)?;
        tokio::fs::write(&self.path, data).await.map_err(io_err)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// On-disk DTO (the anti-corruption layer for configuration)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigDto {
    #[serde(default)]
    agent_servers: HashMap<String, AgentDto>,
    #[serde(default = "default_upload_dir")]
    upload_dir: PathBuf,
    #[serde(default)]
    models: HashMap<String, ModelDto>,
    #[serde(default, alias = "mcpServers")]
    mcp_servers: HashMap<String, McpDto>,
    #[serde(default)]
    commands: HashMap<String, CommandDto>,
    #[serde(default)]
    system_prompts: HashMap<String, String>,
    #[serde(default = "default_preview_lines")]
    tool_call_preview_max_lines: usize,
    #[serde(default)]
    proxy: ProxyDto,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AgentDto {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ModelDto {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    model_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpDto {
    #[serde(default = "default_true")]
    enabled: bool,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

impl Default for McpDto {
    fn default() -> Self {
        Self {
            enabled: true,
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CommandDto {
    #[serde(default)]
    description: String,
    #[serde(default)]
    template: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProxyDto {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    http_proxy_url: String,
    #[serde(default)]
    https_proxy_url: String,
    #[serde(default)]
    all_proxy_url: String,
}

fn default_true() -> bool {
    true
}

fn default_upload_dir() -> PathBuf {
    PathBuf::from(".")
}

fn default_preview_lines() -> usize {
    DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES
}

// ---- DTO -> domain (load) ----

impl From<ConfigDto> for Config {
    fn from(dto: ConfigDto) -> Self {
        Config {
            agents: map_values(dto.agent_servers),
            models: map_values(dto.models),
            mcp_servers: map_values(dto.mcp_servers),
            commands: map_values(dto.commands),
            system_prompts: dto.system_prompts,
            proxy: dto.proxy.into(),
            upload_dir: dto.upload_dir,
            tool_call_preview_max_lines: dto.tool_call_preview_max_lines,
        }
    }
}

impl From<AgentDto> for AgentConfig {
    fn from(dto: AgentDto) -> Self {
        AgentConfig {
            command: dto.command,
            args: dto.args,
            env: dto.env,
        }
    }
}

impl From<ModelDto> for ModelConfig {
    fn from(dto: ModelDto) -> Self {
        ModelConfig {
            enabled: dto.enabled,
            provider: dto.provider,
            base_url: dto.base_url,
            api_key: dto.api_key,
            model_name: dto.model_name,
        }
    }
}

impl From<McpDto> for McpServerConfig {
    fn from(dto: McpDto) -> Self {
        McpServerConfig {
            enabled: dto.enabled,
            command: dto.command,
            args: dto.args,
            env: dto.env,
        }
    }
}

impl From<CommandDto> for CommandConfig {
    fn from(dto: CommandDto) -> Self {
        CommandConfig {
            description: dto.description,
            template: dto.template,
        }
    }
}

impl From<ProxyDto> for ProxyConfig {
    fn from(dto: ProxyDto) -> Self {
        ProxyConfig {
            enabled: dto.enabled,
            http_proxy_url: dto.http_proxy_url,
            https_proxy_url: dto.https_proxy_url,
            all_proxy_url: dto.all_proxy_url,
        }
    }
}

// ---- domain -> DTO (save) ----

impl From<&Config> for ConfigDto {
    fn from(config: &Config) -> Self {
        ConfigDto {
            agent_servers: map_ref_values(&config.agents),
            upload_dir: config.upload_dir.clone(),
            models: map_ref_values(&config.models),
            mcp_servers: map_ref_values(&config.mcp_servers),
            commands: map_ref_values(&config.commands),
            system_prompts: config.system_prompts.clone(),
            tool_call_preview_max_lines: config.tool_call_preview_max_lines,
            proxy: (&config.proxy).into(),
        }
    }
}

impl From<&AgentConfig> for AgentDto {
    fn from(config: &AgentConfig) -> Self {
        AgentDto {
            command: config.command.clone(),
            args: config.args.clone(),
            env: config.env.clone(),
        }
    }
}

impl From<&ModelConfig> for ModelDto {
    fn from(config: &ModelConfig) -> Self {
        ModelDto {
            enabled: config.enabled,
            provider: config.provider.clone(),
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            model_name: config.model_name.clone(),
        }
    }
}

impl From<&McpServerConfig> for McpDto {
    fn from(config: &McpServerConfig) -> Self {
        McpDto {
            enabled: config.enabled,
            command: config.command.clone(),
            args: config.args.clone(),
            env: config.env.clone(),
        }
    }
}

impl From<&CommandConfig> for CommandDto {
    fn from(config: &CommandConfig) -> Self {
        CommandDto {
            description: config.description.clone(),
            template: config.template.clone(),
        }
    }
}

impl From<&ProxyConfig> for ProxyDto {
    fn from(config: &ProxyConfig) -> Self {
        ProxyDto {
            enabled: config.enabled,
            http_proxy_url: config.http_proxy_url.clone(),
            https_proxy_url: config.https_proxy_url.clone(),
            all_proxy_url: config.all_proxy_url.clone(),
        }
    }
}

fn map_values<K, V, W>(map: HashMap<K, V>) -> HashMap<K, W>
where
    K: std::hash::Hash + Eq,
    W: From<V>,
{
    map.into_iter().map(|(k, v)| (k, W::from(v))).collect()
}

fn map_ref_values<K, V, W>(map: &HashMap<K, V>) -> HashMap<K, W>
where
    K: std::hash::Hash + Eq + Clone,
    W: for<'a> From<&'a V>,
{
    map.iter().map(|(k, v)| (k.clone(), W::from(v))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_missing_file_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsConfigStore::new(dir.path().join("absent.json"));
        match store.load().await {
            Err(StoreError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn parses_legacy_format_including_mcp_alias_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let json = r#"{
            "agent_servers": { "Claude": { "command": "npx", "args": ["x"], "env": {} } },
            "mcpServers": { "fs": { "command": "npx", "args": [] } },
            "proxy": { "enabled": true, "http_proxy_url": "http://p:1" }
        }"#;
        tokio::fs::write(&path, json).await.unwrap();

        let config = FsConfigStore::new(&path).load().await.unwrap();

        assert_eq!(config.agents.len(), 1);
        assert_eq!(config.agents["Claude"].command, "npx");
        // `mcpServers` alias is honored, and the missing `enabled` defaults to true.
        assert!(config.mcp_servers.contains_key("fs"));
        assert!(config.mcp_servers["fs"].enabled);
        assert!(config.proxy.enabled);
        // Absent field falls back to the domain default.
        assert_eq!(config.tool_call_preview_max_lines, 10);
    }

    #[tokio::test]
    async fn save_then_load_preserves_agents_and_proxy() {
        let dir = tempfile::tempdir().unwrap();
        // Nested path exercises create_dir_all.
        let path = dir.path().join("nested").join("config.json");
        let store = FsConfigStore::new(&path);

        let mut config = Config::default();
        config.agents.insert(
            "Claude".into(),
            AgentConfig {
                command: "npx".into(),
                args: vec!["a".into()],
                env: HashMap::new(),
            },
        );
        config.proxy.enabled = true;
        config.proxy.http_proxy_url = "http://p:1".into();

        store.save(&config).await.unwrap();
        let loaded = store.load().await.unwrap();

        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.agents["Claude"].command, "npx");
        assert_eq!(loaded.agents["Claude"].args, vec!["a".to_string()]);
        assert!(loaded.proxy.enabled);
        assert_eq!(loaded.proxy.http_proxy_url, "http://p:1");
    }
}
