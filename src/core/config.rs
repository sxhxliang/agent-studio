use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub agent_servers: HashMap<String, AgentProcessConfig>,
    #[serde(default = "default_upload_dir")]
    pub upload_dir: PathBuf,
    #[serde(default)]
    pub models: HashMap<String, ModelConfig>,
    #[serde(default, alias = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
    /// Global system prompts for AI features
    /// Keys: "doc_comment", "inline_comment", "explain", "improve"
    #[serde(default)]
    pub system_prompts: HashMap<String, String>,
    /// Max lines to show in tool call previews (0 disables truncation)
    #[serde(default = "default_tool_call_preview_max_lines")]
    pub tool_call_preview_max_lines: usize,
    /// Network proxy configuration
    #[serde(default)]
    pub proxy: ProxyConfig,
}

fn default_upload_dir() -> PathBuf {
    PathBuf::from(".")
}

pub const DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES: usize = 10;

fn default_tool_call_preview_max_lines() -> usize {
    DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentProcessConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Custom Node.js path (populated at runtime from AppSettings)
    #[serde(skip)]
    pub nodejs_path: Option<String>,
}

/// Model configuration for LLM providers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelConfig {
    pub enabled: bool,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
}

/// MCP (Model Context Protocol) server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl McpServerConfig {
    /// Convert to agent_client_protocol::McpServer
    pub fn to_acp_mcp_server(&self, name: String) -> acp::McpServer {
        // Try to deserialize into McpServerStdio via JSON
        let env_vars: Vec<serde_json::Value> = self
            .env
            .iter()
            .map(|(k, v)| {
                serde_json::json!({
                    "name": k,
                    "value": v
                })
            })
            .collect();

        let stdio_json = serde_json::json!({
            "name": name,
            "command": self.command,
            "args": self.args,
            "env": env_vars
        });

        match serde_json::from_value::<acp::McpServerStdio>(stdio_json) {
            Ok(stdio) => acp::McpServer::Stdio(stdio),
            Err(e) => {
                log::error!("Failed to create McpServerStdio for '{}': {}", name, e);
                // Fallback to a minimal valid config
                acp::McpServer::Stdio(
                    serde_json::from_value(serde_json::json!({
                        "name": name,
                        "command": self.command,
                        "args": self.args,
                        "env": []
                    }))
                    .unwrap(),
                )
            }
        }
    }
}

fn default_true() -> bool {
    true
}

/// Custom command/shortcut configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandConfig {
    pub description: String,
    pub template: String,
}

/// Network proxy configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProxyConfig {
    /// Enable proxy
    #[serde(default)]
    pub enabled: bool,
    /// Proxy type: http, https, socks5
    #[serde(default = "default_proxy_type")]
    pub proxy_type: String,
    /// Proxy host
    #[serde(default)]
    pub host: String,
    /// Proxy port
    #[serde(default)]
    pub port: u16,
    /// Username for proxy authentication
    #[serde(default)]
    pub username: String,
    /// Password for proxy authentication
    #[serde(default)]
    pub password: String,
}

fn default_proxy_type() -> String {
    "http".to_string()
}

impl ProxyConfig {
    /// Get proxy URL for environment variables
    pub fn to_env_value(&self) -> Option<String> {
        if !self.enabled || self.host.is_empty() {
            return None;
        }

        let auth = if !self.username.is_empty() {
            format!("{}:{}@", self.username, self.password)
        } else {
            String::new()
        };

        Some(format!(
            "{}://{}{}:{}",
            self.proxy_type, auth, self.host, self.port
        ))
    }
}
