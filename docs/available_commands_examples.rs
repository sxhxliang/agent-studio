/// Example: How to use Available Commands API
///
/// This example demonstrates how to query and display available commands
/// for a session in the AgentX application.

use agent_client_protocol::AvailableCommand;
use anyhow::Result;
use gpui::App;

use crate::app::AppState;

/// Example 1: Get commands for a session
pub fn example_get_commands(session_id: &str, cx: &App) -> Result<()> {
    let message_service = AppState::global(cx)
        .message_service()
        .ok_or_else(|| anyhow::anyhow!("MessageService not initialized"))?;

    // Get commands by session_id (automatically looks up agent name)
    if let Some(commands) = message_service.get_commands_by_session_id(session_id) {
        println!("📋 Available Commands for session {}:", session_id);
        for cmd in &commands {
            println!("  /{} - {}", cmd.name, cmd.description.as_deref().unwrap_or("No description"));
        }
        Ok(())
    } else {
        Err(anyhow::anyhow!("Session not found: {}", session_id))
    }
}

/// Example 2: Get commands with agent name
pub fn example_get_commands_with_agent(agent_name: &str, session_id: &str, cx: &App) -> Result<()> {
    let message_service = AppState::global(cx)
        .message_service()
        .ok_or_else(|| anyhow::anyhow!("MessageService not initialized"))?;

    // Get commands by agent_name and session_id
    if let Some(commands) = message_service.get_session_commands(agent_name, session_id) {
        println!("📋 Available Commands for {} session {}:", agent_name, session_id);
        for cmd in &commands {
            println!("  /{} - {}", cmd.name, cmd.description.as_deref().unwrap_or("No description"));
        }
        Ok(())
    } else {
        Err(anyhow::anyhow!("Session not found: {}", session_id))
    }
}

/// Example 3: Display commands from session info
pub fn example_display_from_session_info(agent_name: &str, session_id: &str, cx: &App) -> Result<()> {
    let agent_service = AppState::global(cx)
        .agent_service()
        .ok_or_else(|| anyhow::anyhow!("AgentService not initialized"))?;

    if let Some(session_info) = agent_service.get_session_info(agent_name, session_id) {
        println!("📊 Session Information:");
        println!("  ID: {}", session_info.session_id);
        println!("  Agent: {}", session_info.agent_name);
        println!("  Status: {:?}", session_info.status);
        println!("  Created: {}", session_info.created_at);
        println!("  Last Active: {}", session_info.last_active);
        println!("  Available Commands: {}", session_info.available_commands.len());

        for cmd in &session_info.available_commands {
            println!("    /{} - {}", cmd.name, cmd.description.as_deref().unwrap_or("No description"));
        }
        Ok(())
    } else {
        Err(anyhow::anyhow!("Session not found: {}", session_id))
    }
}

/// Example 4: Filter commands by name prefix
pub fn example_filter_commands(session_id: &str, prefix: &str, cx: &App) -> Result<Vec<AvailableCommand>> {
    let message_service = AppState::global(cx)
        .message_service()
        .ok_or_else(|| anyhow::anyhow!("MessageService not initialized"))?;

    if let Some(commands) = message_service.get_commands_by_session_id(session_id) {
        let filtered: Vec<_> = commands
            .into_iter()
            .filter(|cmd| cmd.name.starts_with(prefix))
            .collect();

        println!("🔍 Commands matching '{}': {}", prefix, filtered.len());
        for cmd in &filtered {
            println!("  /{} - {}", cmd.name, cmd.description.as_deref().unwrap_or("No description"));
        }

        Ok(filtered)
    } else {
        Err(anyhow::anyhow!("Session not found: {}", session_id))
    }
}

/// Example 5: Check if a specific command is available
pub fn example_check_command_available(session_id: &str, command_name: &str, cx: &App) -> Result<bool> {
    let message_service = AppState::global(cx)
        .message_service()
        .ok_or_else(|| anyhow::anyhow!("MessageService not initialized"))?;

    if let Some(commands) = message_service.get_commands_by_session_id(session_id) {
        let available = commands.iter().any(|cmd| cmd.name == command_name);
        println!("Command '{}' is {}", command_name, if available { "available ✓" } else { "not available ✗" });
        Ok(available)
    } else {
        Err(anyhow::anyhow!("Session not found: {}", session_id))
    }
}

/// Example 6: Subscribe to command updates in UI component
///
/// This example shows how to react to AvailableCommandsUpdate events in real-time.
#[cfg(feature = "ui_example")]
pub fn example_subscribe_to_updates(
    entity: &gpui::Entity<ConversationPanel>,
    session_id: String,
    cx: &mut gpui::App,
) {
    use agent_client_protocol::SessionUpdate;

    let weak_entity = entity.downgrade();
    let message_service = match AppState::global(cx).message_service() {
        Some(service) => service.clone(),
        None => {
            log::error!("MessageService not initialized");
            return;
        }
    };

    let mut rx = message_service.subscribe_session_updates(Some(session_id.clone()));

    cx.spawn(async move |cx| {
        while let Some(update) = rx.recv().await {
            if let SessionUpdate::AvailableCommandsUpdate(commands_update) = update {
                log::info!("Received command update: {} commands", commands_update.available_commands.len());

                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            // Update local state
                            // this.available_commands = commands_update.available_commands;
                            log::info!("Updated UI with new commands");
                            cx.notify(); // Trigger re-render
                        });
                    }
                });
            }
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_filtering() {
        // This is a conceptual test - actual testing would require mocking
        // the AppState and services

        // Example: Test that commands are properly filtered by prefix
        let commands = vec![
            AvailableCommand {
                name: "compact".to_string(),
                description: Some("Clear history".to_string()),
                input: None,
                meta: None,
            },
            AvailableCommand {
                name: "init".to_string(),
                description: Some("Initialize CLAUDE.md".to_string()),
                input: None,
                meta: None,
            },
            AvailableCommand {
                name: "review".to_string(),
                description: Some("Review PR".to_string()),
                input: None,
                meta: None,
            },
        ];

        let filtered: Vec<_> = commands
            .into_iter()
            .filter(|cmd| cmd.name.starts_with("re"))
            .collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "review");
    }
}
