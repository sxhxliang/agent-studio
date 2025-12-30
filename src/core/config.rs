use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, path::PathBuf};

// Simplified MCP server format (for compatibility)
#[derive(Debug, Clone, Deserialize)]
struct SimplifiedMcpServer {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

// Custom deserializer for mcp_servers that handles both formats
fn deserialize_mcp_servers<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, McpServerConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize as a raw JSON value first
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;

    match value {
        None => Ok(HashMap::new()),
        Some(v) => {
            if let serde_json::Value::Object(map) = v {
                let mut result = HashMap::new();

                for (name, server_value) in map {
                    // Try to parse as full McpServerConfig first
                    if let Ok(config) =
                        serde_json::from_value::<McpServerConfig>(server_value.clone())
                    {
                        result.insert(name, config);
                    }
                    // Try to parse as simplified format
                    else if let Ok(simplified) =
                        serde_json::from_value::<SimplifiedMcpServer>(server_value.clone())
                    {
                        log::info!(
                            "Converting simplified MCP config for '{}': {:?}",
                            name,
                            simplified
                        );

                        // Convert to JSON and then to McpServerStdio via deserialization
                        let env_vars: Vec<serde_json::Value> = simplified
                            .env
                            .into_iter()
                            .map(|(name, value)| {
                                serde_json::json!({
                                    "name": name,
                                    "value": value
                                })
                            })
                            .collect();

                        // Build the stdio config as JSON
                        let stdio_json = serde_json::json!({
                            "name": name.clone(),
                            "command": simplified.command,
                            "args": simplified.args,
                            "env": env_vars
                        });

                        // Deserialize into McpServerStdio
                        match serde_json::from_value::<agent_client_protocol::McpServerStdio>(
                            stdio_json,
                        ) {
                            Ok(stdio) => {
                                // Wrap in McpServer enum
                                let mcp_server = agent_client_protocol::McpServer::Stdio(stdio);

                                // Create full config
                                let config = McpServerConfig {
                                    enabled: true,
                                    description: format!("MCP server: {}", name),
                                    config: mcp_server,
                                };

                                result.insert(name, config);
                            }
                            Err(e) => {
                                log::warn!("Failed to create McpServerStdio for '{}': {}", name, e);
                            }
                        }
                    } else {
                        log::warn!(
                            "Failed to parse MCP server config for '{}', skipping. Value: {:?}",
                            name,
                            server_value
                        );
                    }
                }

                Ok(result)
            } else {
                log::warn!("mcp_servers is not an object, using empty map");
                Ok(HashMap::new())
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub agent_servers: HashMap<String, AgentProcessConfig>,
    #[serde(default = "default_upload_dir")]
    pub upload_dir: PathBuf,
    #[serde(default)]
    pub models: HashMap<String, ModelConfig>,
    #[serde(
        default,
        alias = "mcpServers",
        deserialize_with = "deserialize_mcp_servers"
    )]
    pub mcp_servers: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub commands: HashMap<String, CommandConfig>,
}

fn default_upload_dir() -> PathBuf {
    PathBuf::from(".")
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentProcessConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
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
    pub enabled: bool,
    pub description: String,
    pub config: agent_client_protocol::McpServer,
}

/// Custom command/shortcut configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandConfig {
    pub description: String,
    pub template: String,
}
