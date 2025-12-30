// Test to verify MCP server config parsing
use std::fs;

#[test]
fn test_simplified_mcp_config_parsing() {
    // Read the test config file
    let config_json =
        fs::read_to_string("config.test.json").expect("Failed to read config.test.json");

    // Parse the config
    let config: crate::core::config::Config =
        serde_json::from_str(&config_json).expect("Failed to parse config.test.json");

    // Verify that MCP servers were parsed
    assert!(
        !config.mcp_servers.is_empty(),
        "MCP servers should not be empty"
    );

    // Verify filesystem server
    let filesystem = config
        .mcp_servers
        .get("filesystem")
        .expect("filesystem server not found");
    assert!(filesystem.enabled, "filesystem server should be enabled");

    // Verify github server
    let github = config
        .mcp_servers
        .get("github")
        .expect("github server not found");
    assert!(github.enabled, "github server should be enabled");

    println!("✓ Simplified MCP config parsing test passed!");
    println!("  Parsed {} MCP servers", config.mcp_servers.len());
    for (name, config) in &config.mcp_servers {
        println!("  - {}: {}", name, config.description);
    }
}
