use gpui::{AppContext as _, Context, Entity, ParentElement as _, Styled, Window, px};
use gpui_component::{
    ActiveTheme, IconName, Sizable, WindowExt as _,
    button::Button,
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputState},
    label::Label,
    setting::{SettingGroup, SettingItem, SettingPage},
    v_flex,
};
use std::collections::HashMap;

use super::panel::SettingsPanel;
use crate::{AppState, core::config::McpServerConfig};

impl SettingsPanel {
    pub fn mcp_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new("MCP Servers")
            .resettable(false)
            .groups(vec![
                SettingGroup::new()
                    .title("MCP Server Configurations")
                    .item(SettingItem::render({
                        let view = view.clone();
                        move |_options, _window, cx| {
                            let mcp_configs = view.read(cx).cached_mcp_servers.clone();

                            let mut content = v_flex()
                                .w_full()
                                .gap_3()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_end()
                                        .child(
                                            Button::new("add-mcp-btn")
                                                .label("Add MCP Server")
                                                .icon(IconName::Plus)
                                                .small()
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            this.show_add_mcp_dialog(window, cx);
                                                        });
                                                    }
                                                })
                                        )
                                );

                            if mcp_configs.is_empty() {
                                content = content.child(
                                    h_flex()
                                        .w_full()
                                        .p_4()
                                        .justify_center()
                                        .child(
                                            Label::new("No MCP servers configured. Click 'Add MCP Server' to get started.")
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                        )
                                );
                            } else {
                                for (idx, (name, config)) in mcp_configs.iter().enumerate() {
                                    let name_for_edit = name.clone();
                                    let name_for_delete = name.clone();

                                    let mut mcp_info = v_flex()
                                        .flex_1()
                                        .gap_1()
                                        .child(
                                            Label::new(name.clone())
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                        );

                                    if !config.description.is_empty() {
                                        mcp_info = mcp_info.child(
                                            Label::new(config.description.clone())
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                        );
                                    }

                                    // Note: Config is a structured McpServer type
                                    mcp_info = mcp_info.child(
                                        Label::new("Config: Configured")
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                    );

                                    content = content.child(
                                        h_flex()
                                            .w_full()
                                            .items_start()
                                            .justify_between()
                                            .p_3()
                                            .gap_3()
                                            .rounded(px(6.))
                                            .bg(cx.theme().secondary)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .child(mcp_info)
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Label::new(if config.enabled { "Enabled" } else { "Disabled" })
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                    )
                                                    .child(
                                                        Button::new(("edit-mcp-btn", idx))
                                                            .label("Edit")
                                                            .icon(IconName::Settings)
                                                            .outline()
                                                            .small()
                                                            .on_click({
                                                                let view = view.clone();
                                                                move |_, window, cx| {
                                                                    view.update(cx, |this, cx| {
                                                                        this.show_edit_mcp_dialog(
                                                                            window,
                                                                            cx,
                                                                            name_for_edit.clone()
                                                                        );
                                                                    });
                                                                }
                                                            })
                                                    )
                                                    .child(
                                                        Button::new(("delete-mcp-btn", idx))
                                                            .label("Delete")
                                                            .icon(IconName::Delete)
                                                            .outline()
                                                            .small()
                                                            .on_click({
                                                                let view = view.clone();
                                                                move |_, window, cx| {
                                                                    view.update(cx, |this, cx| {
                                                                        this.show_delete_mcp_dialog(
                                                                            window,
                                                                            cx,
                                                                            name_for_delete.clone()
                                                                        );
                                                                    });
                                                                }
                                                            })
                                                    )
                                            )
                                    );
                                }
                            }

                            content
                        }
                    })),
                // JSON Editor Group
                SettingGroup::new()
                    .title("JSON Editor")
                    .item(SettingItem::render({
                        let view = view.clone();
                        move |_options, _window, cx| {
                            let json_editor = view.read(cx).mcp_json_editor.clone();
                            let json_error = view.read(cx).mcp_json_error.clone();

                            v_flex()
                                .w_full()
                                .gap_3()
                                .child(
                                    Label::new("Edit MCP servers configuration in JSON format. Supports both simplified and full formats.")
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .gap_2()
                                        .child(
                                            Button::new("load-mcp-json-btn")
                                                .label("Load from Config")
                                                .icon(IconName::ArrowDown)
                                                .small()
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            this.load_mcp_servers_to_json(window, cx);
                                                        });
                                                    }
                                                })
                                        )
                                        .child(
                                            Button::new("validate-mcp-json-btn")
                                                .label("Validate")
                                                .icon(IconName::Check)
                                                .small()
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            this.validate_mcp_json(window, cx);
                                                        });
                                                    }
                                                })
                                        )
                                        .child(
                                            Button::new("save-mcp-json-btn")
                                                .label("Save")
                                                .icon(IconName::ArrowUp)
                                                .small()
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            this.save_mcp_json(window, cx);
                                                        });
                                                    }
                                                })
                                        )
                                )
                                .child(
                                    Input::new(&json_editor)
                                        .h(px(300.))
                                        .w_full()
                                )
                                .children(json_error.map(|error| {
                                    Label::new(error.clone())
                                        .text_sm()
                                        .text_color(if error.starts_with("✓") {
                                            gpui::green()
                                        } else {
                                            gpui::red()
                                        })
                                }))
                                .child(
                                    v_flex()
                                        .gap_2()
                                        .p_3()
                                        .rounded(px(6.))
                                        .bg(cx.theme().secondary)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            Label::new("Example (Simplified Format):")
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                        )
                                        .child(
                                            Label::new(
r#"{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"],
      "env": {
        "DEBUG": "true"
      }
    }
  }
}"#
                                            )
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                        )
                                )
                        }
                    })),
            ])
    }

    pub fn show_add_mcp_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Server name"));
        let desc_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));
        let config_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Config JSON (e.g., {\"key\": \"value\"})")
        });

        window.open_dialog(cx, move |dialog, _window, cx| {
            dialog
                .title("Add MCP Server")
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Add")
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let name_input = name_input.clone();
                    let desc_input = desc_input.clone();
                    let config_input = config_input.clone();

                    move |_, _window, cx| {
                        let name = name_input.read(cx).text().to_string().trim().to_string();
                        let desc = desc_input.read(cx).text().to_string().trim().to_string();
                        let config_str =
                            config_input.read(cx).text().to_string().trim().to_string();

                        if name.is_empty() {
                            log::warn!("Name cannot be empty");
                            return false;
                        }

                        // Parse config JSON
                        let mcp_server_config: agent_client_protocol::McpServer =
                            if !config_str.is_empty() {
                                match serde_json::from_str(&config_str) {
                                    Ok(config) => config,
                                    Err(e) => {
                                        log::error!("Failed to parse MCP server config: {}", e);
                                        return false;
                                    }
                                }
                            } else {
                                log::error!("MCP server config cannot be empty");
                                return false;
                            };

                        // Save to config file
                        if let Some(service) = AppState::global(cx).agent_config_service() {
                            let service = service.clone();
                            let config = crate::core::config::McpServerConfig {
                                enabled: true,
                                description: desc,
                                config: mcp_server_config,
                            };

                            cx.spawn(async move |cx| {
                                match service.add_mcp_server(name.clone(), config).await {
                                    Ok(_) => {
                                        log::info!("Successfully added MCP server: {}", name);
                                        _ = cx.update(|_cx| {});
                                    }
                                    Err(e) => {
                                        log::error!("Failed to add MCP server: {}", e);
                                    }
                                }
                            })
                            .detach();
                        }

                        true
                    }
                })
                .child(
                    v_flex()
                        .w_full()
                        .gap_3()
                        .p_4()
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Name"))
                                .child(Input::new(&name_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Description"))
                                .child(Input::new(&desc_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Configuration"))
                                .child(Input::new(&config_input)),
                        ),
                )
        });
    }

    pub fn show_edit_mcp_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        server_name: String,
    ) {
        let config = self.cached_mcp_servers.get(&server_name).cloned();
        if config.is_none() {
            log::warn!("MCP server config not found: {}", server_name);
            return;
        }
        let config = config.unwrap();
        let entity = cx.entity().downgrade();

        let desc_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.description.clone(), window, cx);
            state
        });

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(format!("Edit MCP Server: {}", server_name))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Save")
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let desc_input = desc_input.clone();
                    let server_name = server_name.clone();
                    let enabled = config.enabled;
                    let mcp_server_config = config.config.clone();
                    let entity = entity.clone();

                    move |_, _window, cx| {
                        let desc = desc_input.read(cx).text().to_string().trim().to_string();

                        // Save to config file
                        if let Some(service) = AppState::global(cx).agent_config_service() {
                            let service = service.clone();
                            let server_name_for_async = server_name.clone();
                            let config = crate::core::config::McpServerConfig {
                                enabled,
                                description: desc,
                                config: mcp_server_config.clone(),
                            };
                            let entity = entity.clone();

                            cx.spawn(async move |cx| {
                                match service
                                    .update_mcp_server(&server_name_for_async, config.clone())
                                    .await
                                {
                                    Ok(_) => {
                                        log::info!(
                                            "Successfully updated MCP server: {}",
                                            server_name_for_async
                                        );
                                        // Update UI
                                        _ = cx.update(|cx| {
                                            if let Some(panel) = entity.upgrade() {
                                                panel.update(cx, |this, cx| {
                                                    this.cached_mcp_servers
                                                        .insert(server_name_for_async, config);
                                                    cx.notify();
                                                });
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        log::error!("Failed to update MCP server: {}", e);
                                    }
                                }
                            })
                            .detach();
                        }

                        true
                    }
                })
                .child(
                    v_flex().w_full().gap_3().p_4().child(
                        v_flex()
                            .gap_2()
                            .child(Label::new("Description"))
                            .child(Input::new(&desc_input)),
                    ),
                )
        });
    }

    pub fn show_delete_mcp_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        server_name: String,
    ) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let name = server_name.clone();
            dialog
                .title("Confirm Delete")
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Delete")
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .cancel_text("Cancel"),
                )
                .on_ok(move |_, _window, cx| {
                    if let Some(service) = AppState::global(cx).agent_config_service() {
                        let service = service.clone();
                        let name = name.clone();
                        cx.spawn(async move |cx| {
                            if let Err(e) = service.remove_mcp_server(&name).await {
                                log::error!("Failed to delete MCP server: {}", e);
                            } else {
                                log::info!("Successfully deleted MCP server: {}", name);
                            }
                            let _ = cx.update(|_cx| {});
                        })
                        .detach();
                    }
                    true
                })
                .child(
                    v_flex().w_full().gap_2().p_4().child(
                        Label::new(format!(
                            "Are you sure you want to delete the MCP server \"{}\"?",
                            server_name
                        ))
                        .text_sm(),
                    ),
                )
        });
    }

    // MCP JSON Editor helpers
    pub fn parse_mcp_json(
        &self,
        cx: &Context<Self>,
    ) -> Result<HashMap<String, McpServerConfig>, String> {
        let json_text = self.mcp_json_editor.read(cx).text().to_string();

        let value = serde_json::from_str::<serde_json::Value>(&json_text)
            .map_err(|e| format!("Invalid JSON: {}", e))?;

        let mcp_servers = value
            .as_object()
            .and_then(|obj| obj.get("mcpServers").or_else(|| obj.get("mcp_servers")))
            .ok_or_else(|| "Missing 'mcpServers' or 'mcp_servers' field".to_string())?;

        serde_json::from_value::<HashMap<String, McpServerConfig>>(mcp_servers.clone())
            .map_err(|e| format!("Invalid MCP config: {}", e))
    }

    pub fn load_mcp_servers_to_json(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let json = serde_json::json!({
            "mcpServers": self.cached_mcp_servers.iter().map(|(name, config)| {
                (name.clone(), serde_json::json!({
                    "enabled": config.enabled,
                    "description": config.description,
                    "config": config.config
                }))
            }).collect::<serde_json::Map<String, serde_json::Value>>()
        });

        let json_str = serde_json::to_string_pretty(&json)
            .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize: {}\"}}", e));

        self.mcp_json_editor.update(cx, |input, cx| {
            input.set_value(json_str, window, cx);
        });

        self.mcp_json_error = None;
        cx.notify();
    }

    pub fn validate_mcp_json(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.mcp_json_error = match self.parse_mcp_json(cx) {
            Ok(servers) => Some(format!("✓ Valid! Found {} MCP server(s)", servers.len())),
            Err(e) => Some(format!("✗ {}", e)),
        };
        cx.notify();
    }

    pub fn save_mcp_json(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        match self.parse_mcp_json(cx) {
            Ok(servers) => {
                if let Some(service) = AppState::global(cx).agent_config_service() {
                    let service = service.clone();
                    cx.spawn_in(_window, async move |_this, _cx| {
                        // Remove old servers
                        for (name, _) in service.list_mcp_servers().await {
                            let _ = service.remove_mcp_server(&name).await;
                        }
                        // Add new servers
                        for (name, config) in servers {
                            let _ = service.add_mcp_server(name, config).await;
                        }
                        log::info!("MCP servers saved successfully");
                    })
                    .detach();
                    self.mcp_json_error = Some("✓ Saved successfully!".to_string());
                } else {
                    self.mcp_json_error = Some("✗ Agent config service not available".to_string());
                }
            }
            Err(e) => {
                self.mcp_json_error = Some(format!("✗ {}", e));
            }
        }
        cx.notify();
    }
}
