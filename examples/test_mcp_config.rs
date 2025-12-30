use agent_client_protocol::McpServer;
use agentx::Config;
use std::fs;

fn main() {
    println!("Testing MCP Config Parsing...\n");

    // Test the new config.test.json
    match fs::read_to_string("config.test.json") {
        Ok(config_json) => {
            match serde_json::from_str::<Config>(&config_json) {
                Ok(config) => {
                    println!("✓ Successfully parsed config.test.json");
                    println!("  Found {} MCP servers:", config.mcp_servers.len());

                    for (name, server_config) in &config.mcp_servers {
                        println!(
                            "    - {} ({})",
                            name,
                            if server_config.enabled {
                                "enabled"
                            } else {
                                "disabled"
                            }
                        );
                        println!("      Description: {}", server_config.description);

                        // Show config details
                        match &server_config.config {
                            McpServer::Stdio(stdio) => {
                                println!("      Transport: stdio");
                                println!("      Command: {:?}", stdio.command);
                                println!("      Args: {:?}", stdio.args);
                                println!("      Env vars: {}", stdio.env.len());
                            }
                            McpServer::Http(http) => {
                                println!("      Transport: HTTP");
                                println!("      URL: {}", http.url);
                            }
                            McpServer::Sse(sse) => {
                                println!("      Transport: SSE");
                                println!("      URL: {}", sse.url);
                            }
                            _ => {
                                println!("      Transport: Unknown");
                            }
                        }
                        println!();
                    }
                }
                Err(e) => {
                    eprintln!("✗ Failed to parse config.test.json: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to read config.test.json: {}", e);
            std::process::exit(1);
        }
    }

    println!("\n✓ All tests passed!");
}
