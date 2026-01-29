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

#[cfg(test)]
mod tests {
    use super::*;

    // ============== Config tests ==============

    #[test]
    fn test_config_default_values() {
        let json = r#"{
            "agent_servers": {}
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();

        assert_eq!(config.upload_dir, PathBuf::from("."));
        assert!(config.models.is_empty());
        assert!(config.mcp_servers.is_empty());
        assert!(config.commands.is_empty());
        assert!(config.system_prompts.is_empty());
        assert_eq!(
            config.tool_call_preview_max_lines,
            DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES
        );
        assert!(!config.proxy.enabled);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut config = Config {
            agent_servers: HashMap::new(),
            upload_dir: PathBuf::from("/tmp/uploads"),
            models: HashMap::new(),
            mcp_servers: HashMap::new(),
            commands: HashMap::new(),
            system_prompts: HashMap::new(),
            tool_call_preview_max_lines: 20,
            proxy: ProxyConfig::default(),
        };

        config.agent_servers.insert(
            "test-agent".to_string(),
            AgentProcessConfig {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
                env: HashMap::new(),
                nodejs_path: None,
            },
        );

        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(config.upload_dir, restored.upload_dir);
        assert_eq!(
            config.tool_call_preview_max_lines,
            restored.tool_call_preview_max_lines
        );
        assert!(restored.agent_servers.contains_key("test-agent"));
    }

    #[test]
    fn test_config_minimal_json() {
        // Minimal valid JSON should deserialize with defaults
        let json = r#"{"agent_servers": {}}"#;
        let result: Result<Config, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    // ============== AgentProcessConfig tests ==============

    #[test]
    fn test_agent_process_config_serialization() {
        let config = AgentProcessConfig {
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@anthropic/claude".to_string()],
            env: {
                let mut env = HashMap::new();
                env.insert("API_KEY".to_string(), "secret".to_string());
                env
            },
            nodejs_path: Some("/usr/bin/node".to_string()), // should be skipped
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: AgentProcessConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.command, "npx");
        assert_eq!(restored.args.len(), 2);
        assert!(restored.env.contains_key("API_KEY"));
        // nodejs_path is skipped during serialization
        assert!(restored.nodejs_path.is_none());
    }

    // ============== ProxyConfig tests ==============

    #[test]
    fn test_proxy_config_to_env_value_disabled() {
        let config = ProxyConfig {
            enabled: false,
            proxy_type: "http".to_string(),
            host: "proxy.example.com".to_string(),
            port: 8080,
            username: String::new(),
            password: String::new(),
        };

        assert!(config.to_env_value().is_none());
    }

    #[test]
    fn test_proxy_config_to_env_value_no_auth() {
        let config = ProxyConfig {
            enabled: true,
            proxy_type: "http".to_string(),
            host: "proxy.example.com".to_string(),
            port: 8080,
            username: String::new(),
            password: String::new(),
        };

        let value = config.to_env_value().unwrap();
        assert_eq!(value, "http://proxy.example.com:8080");
    }

    #[test]
    fn test_proxy_config_to_env_value_with_auth() {
        let config = ProxyConfig {
            enabled: true,
            proxy_type: "socks5".to_string(),
            host: "proxy.example.com".to_string(),
            port: 1080,
            username: "user".to_string(),
            password: "pass".to_string(),
        };

        let value = config.to_env_value().unwrap();
        assert_eq!(value, "socks5://user:pass@proxy.example.com:1080");
    }

    #[test]
    fn test_proxy_config_to_env_value_empty_host() {
        let config = ProxyConfig {
            enabled: true,
            proxy_type: "http".to_string(),
            host: String::new(),
            port: 8080,
            username: String::new(),
            password: String::new(),
        };

        assert!(config.to_env_value().is_none());
    }

    // ============== ModelConfig tests ==============

    #[test]
    fn test_model_config_serialization() {
        let config = ModelConfig {
            enabled: true,
            provider: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-xxx".to_string(),
            model_name: "gpt-4".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: ModelConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.provider, "openai");
        assert_eq!(restored.model_name, "gpt-4");
    }

    // ============== McpServerConfig tests ==============

    #[test]
    fn test_mcp_server_config_default_enabled() {
        let json = r#"{
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"]
        }"#;
        let config: McpServerConfig = serde_json::from_str(json).unwrap();

        assert!(config.enabled); // default_true()
    }

    #[test]
    fn test_mcp_server_config_to_acp() {
        let config = McpServerConfig {
            enabled: true,
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "some-mcp-server".to_string()],
            env: {
                let mut env = HashMap::new();
                env.insert("API_KEY".to_string(), "test-key".to_string());
                env
            },
        };

        let acp_server = config.to_acp_mcp_server("test-server".to_string());

        match acp_server {
            acp::McpServer::Stdio(stdio) => {
                // stdio.name and stdio.command may be PathBuf or similar types
                // Just check they're non-empty
                assert!(!stdio.name.is_empty());
                assert!(!stdio.command.as_os_str().is_empty());
                assert_eq!(stdio.args.len(), 2);
            }
            _ => panic!("Expected Stdio variant"),
        }
    }
}
