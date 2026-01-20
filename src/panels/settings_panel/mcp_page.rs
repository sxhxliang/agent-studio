use gpui::{AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled, Window, px};
use gpui_component::{
    ActiveTheme, IconName, Sizable, WindowExt as _,
    button::Button,
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputState, TabSize},
    label::Label,
    setting::{SettingGroup, SettingItem, SettingPage},
    tab::{Tab, TabBar},
    v_flex,
};
use rust_i18n::t;
use std::collections::HashMap;

use super::panel::SettingsPanel;
use crate::{AppState, core::config::McpServerConfig};

impl SettingsPanel {
    pub fn mcp_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new(t!("settings.mcp.title").to_string())
            .resettable(false)
            .groups(vec![
                SettingGroup::new().item(SettingItem::render({
                    let view = view.clone();
                    move |_options, window, cx| {
                        let active_tab = view.read(cx).mcp_active_tab;

                        v_flex()
                            .w_full()
                            .gap_4()
                            .child(
                                TabBar::new("mcp-tabs")
                                    .w_full()
                                    .segmented()
                                    .selected_index(active_tab)
                                    .on_click({
                                        let view = view.clone();
                                        move |ix: &usize, _window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.mcp_active_tab = *ix;
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child(Tab::new().flex_1().label(t!("settings.mcp.tab.interactive").to_string()))
                                    .child(Tab::new().flex_1().label(t!("settings.mcp.tab.json_editor").to_string())),
                            )
                            .child(
                                if active_tab == 0 {
                                    Self::render_interactive_editor(&view, window, cx)
                                } else {
                                    Self::render_json_editor(&view, window, cx)
                                }
                            )
                    }
                })),
            ])
    }

    fn render_interactive_editor(
        view: &Entity<Self>,
        _window: &mut Window,
        cx: &mut gpui::App
    ) -> gpui::AnyElement {
        let mcp_configs = view.read(cx).cached_mcp_servers.clone();

        let mut content = v_flex().w_full().gap_3().child(
            h_flex().w_full().justify_end().child(
                Button::new("add-mcp-btn")
                    .label(t!("settings.mcp.button.add").to_string())
                    .icon(IconName::Plus)
                    .small()
                    .on_click({
                        let view = view.clone();
                        move |_, window, cx| {
                            view.update(cx, |this, cx| {
                                this.show_add_mcp_dialog(window, cx);
                            });
                        }
                    }),
            ),
        );

        if mcp_configs.is_empty() {
            content = content.child(
                h_flex().w_full().p_4().justify_center().child(
                    Label::new(t!("settings.mcp.empty").to_string())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            );
        } else {
            for (idx, (name, config)) in mcp_configs.iter().enumerate() {
                let name_for_delete = name.clone();

                let mcp_info = v_flex().flex_1().gap_1().child(
                    Label::new(name.clone())
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD),
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
                                    Label::new(if config.enabled {
                                        t!("settings.mcp.status.enabled")
                                            .to_string()
                                    } else {
                                        t!("settings.mcp.status.disabled")
                                            .to_string()
                                    })
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    Button::new(("delete-mcp-btn", idx))
                                        .label(
                                            t!("settings.mcp.button.delete")
                                                .to_string(),
                                        )
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
                                        }),
                                ),
                        ),
                );
            }
        }

        IntoElement::into_any_element(content)
    }

    fn render_json_editor(
        view: &Entity<Self>,
        _window: &mut Window,
        cx: &mut gpui::App
    ) -> gpui::AnyElement {
        let json_editor = view.read(cx).mcp_json_editor.clone();
        let json_error = view.read(cx).mcp_json_error.clone();

        let content = v_flex()
            .w_full()
            .gap_3()
            .child(
                Label::new(t!("settings.mcp.json.description").to_string())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Button::new("load-mcp-json-btn")
                            .label(
                                t!("settings.mcp.json.button.load").to_string(),
                            )
                            .icon(IconName::ArrowDown)
                            .small()
                            .on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.load_mcp_servers_to_json(
                                            window, cx,
                                        );
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("validate-mcp-json-btn")
                            .label(
                                t!("settings.mcp.json.button.validate")
                                    .to_string(),
                            )
                            .icon(IconName::Check)
                            .small()
                            .on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.validate_mcp_json(window, cx);
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("save-mcp-json-btn")
                            .label(
                                t!("settings.mcp.json.button.save").to_string(),
                            )
                            .icon(IconName::ArrowUp)
                            .small()
                            .on_click({
                                let view = view.clone();
                                move |_, window, cx| {
                                    view.update(cx, |this, cx| {
                                        this.save_mcp_json(window, cx);
                                    });
                                }
                            }),
                    ),
            )
            .child(Input::new(&json_editor).h(px(300.)).w_full())
            .children(json_error.map(|error| {
                Label::new(error.clone()).text_sm().text_color(
                    if error.starts_with("✓") {
                        gpui::green()
                    } else {
                        gpui::red()
                    },
                )
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
                        Label::new(t!("settings.mcp.json.example").to_string())
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD),
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
}"#,
                        )
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                    ),
            );

        IntoElement::into_any_element(content)
    }

    pub fn show_add_mcp_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t!("settings.mcp.dialog.add.name.placeholder").to_string())
        });
        let config_input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .auto_grow(10, 20)
                .placeholder(t!("settings.mcp.dialog.add.config.placeholder").to_string())
        });

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(t!("settings.mcp.dialog.add.title").to_string())
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(t!("settings.mcp.dialog.add.ok").to_string())
                        .cancel_text(t!("settings.mcp.dialog.cancel").to_string()),
                )
                .on_ok({
                    let name_input = name_input.clone();
                    let config_input = config_input.clone();

                    move |_, _window, cx| {
                        let name = name_input.read(cx).text().to_string().trim().to_string();
                        let config_str =
                            config_input.read(cx).text().to_string().trim().to_string();

                        if name.is_empty() {
                            log::warn!("Name cannot be empty");
                            return false;
                        }

                        // Parse config JSON (without enabled field, we'll add it)
                        #[derive(serde::Deserialize)]
                        struct TempMcpConfig {
                            command: String,
                            #[serde(default)]
                            args: Vec<String>,
                            #[serde(default)]
                            env: std::collections::HashMap<String, String>,
                        }

                        let temp_config: TempMcpConfig = if !config_str.is_empty() {
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
                                command: temp_config.command,
                                args: temp_config.args,
                                env: temp_config.env,
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
                                .child(Label::new(
                                    t!("settings.mcp.dialog.add.name.label").to_string(),
                                ))
                                .child(Input::new(&name_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new(
                                    t!("settings.mcp.dialog.add.config.label").to_string(),
                                ))
                                .child(Input::new(&config_input)),
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
                .title(t!("settings.mcp.dialog.delete.title").to_string())
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(t!("settings.mcp.dialog.delete.ok").to_string())
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .cancel_text(t!("settings.mcp.dialog.cancel").to_string()),
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
                            "{}",
                            t!("settings.mcp.dialog.delete.message", name = server_name)
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
            .map_err(|e| t!("settings.mcp.json.error.invalid_json", error = e).to_string())?;

        let mcp_servers = value
            .as_object()
            .and_then(|obj| obj.get("mcpServers").or_else(|| obj.get("mcp_servers")))
            .ok_or_else(|| t!("settings.mcp.json.error.missing_field").to_string())?;

        serde_json::from_value::<HashMap<String, McpServerConfig>>(mcp_servers.clone())
            .map_err(|e| t!("settings.mcp.json.error.invalid_config", error = e).to_string())
    }

    pub fn load_mcp_servers_to_json(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let json = serde_json::json!({
            "mcpServers": self.cached_mcp_servers.iter().map(|(name, config)| {
                (name.clone(), serde_json::json!({
                    "enabled": config.enabled,
                    "command": config.command,
                    "args": config.args,
                    "env": config.env
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
            Ok(servers) => Some(t!("settings.mcp.json.valid", count = servers.len()).to_string()),
            Err(e) => Some(t!("settings.mcp.json.invalid", error = e).to_string()),
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
                    self.mcp_json_error = Some(t!("settings.mcp.json.saved").to_string());
                } else {
                    self.mcp_json_error =
                        Some(t!("settings.mcp.json.service_unavailable").to_string());
                }
            }
            Err(e) => {
                self.mcp_json_error = Some(t!("settings.mcp.json.invalid", error = e).to_string());
            }
        }
        cx.notify();
    }
}
