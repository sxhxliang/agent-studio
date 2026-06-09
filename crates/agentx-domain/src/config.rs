//! Configuration data: agents, models, MCP servers, commands, and proxy.
//!
//! Plain owned data so the [`ConfigStore`](crate::ports::ConfigStore) port can
//! be expressed without referencing any on-disk format or ACP type. The store
//! adapter ([`agentx-store`]) maps between this and the JSON file.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub agents: HashMap<String, AgentConfig>,
    pub models: HashMap<String, ModelConfig>,
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub commands: HashMap<String, CommandConfig>,
    pub system_prompts: HashMap<String, String>,
    pub proxy: ProxyConfig,
    pub upload_dir: PathBuf,
    pub tool_call_preview_max_lines: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agents: HashMap::new(),
            models: HashMap::new(),
            mcp_servers: HashMap::new(),
            commands: HashMap::new(),
            system_prompts: HashMap::new(),
            proxy: ProxyConfig::default(),
            upload_dir: PathBuf::from("."),
            tool_call_preview_max_lines: DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES,
        }
    }
}

/// How to launch an agent subprocess.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// An LLM provider/model the app can use for its own AI features.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelConfig {
    pub enabled: bool,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
}

/// An MCP (Model Context Protocol) server made available to agents.
///
/// `name` is the server's identifier (the key it is stored under in
/// [`Config::mcp_servers`]); the ACP adapter forwards it to the agent so it can
/// label the launched server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// A user-defined command/shortcut template.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandConfig {
    pub description: String,
    pub template: String,
}

/// Network proxy settings injected into agent subprocesses.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub http_proxy_url: String,
    pub https_proxy_url: String,
    pub all_proxy_url: String,
}

impl ProxyConfig {
    /// The proxy environment variables to inject, in both upper- and lowercase
    /// spellings. Empty when the proxy is disabled or no URLs are set.
    pub fn env_vars(&self) -> Vec<(String, String)> {
        let mut vars = Vec::new();
        if !self.enabled {
            return vars;
        }
        for (key, value) in [
            ("HTTP_PROXY", &self.http_proxy_url),
            ("HTTPS_PROXY", &self.https_proxy_url),
            ("ALL_PROXY", &self.all_proxy_url),
        ] {
            if !value.is_empty() {
                vars.push((key.to_string(), value.clone()));
                vars.push((key.to_lowercase(), value.clone()));
            }
        }
        vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_proxy_yields_no_env_vars() {
        let proxy = ProxyConfig {
            enabled: false,
            http_proxy_url: "http://localhost:8080".into(),
            ..Default::default()
        };
        assert!(proxy.env_vars().is_empty());
    }

    #[test]
    fn enabled_proxy_emits_upper_and_lowercase() {
        let proxy = ProxyConfig {
            enabled: true,
            http_proxy_url: "http://localhost:8080".into(),
            ..Default::default()
        };
        let vars = proxy.env_vars();
        assert!(vars.contains(&("HTTP_PROXY".into(), "http://localhost:8080".into())));
        assert!(vars.contains(&("http_proxy".into(), "http://localhost:8080".into())));
        // No HTTPS/ALL urls were set, so only the HTTP pair is present.
        assert_eq!(vars.len(), 2);
    }
}
