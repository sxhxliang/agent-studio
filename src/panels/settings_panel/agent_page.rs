use gpui::{AppContext as _, Context, Entity, ParentElement as _, Styled, Window, px};
use gpui_component::{
    ActiveTheme, IconName, Sizable, WindowExt as _,
    button::Button,
    dialog::DialogButtonProps,
    h_flex,
    input::{Input, InputState},
    label::Label,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage},
    v_flex,
};
use std::collections::HashMap;

use super::panel::SettingsPanel;
use crate::{
    AppState,
    app::actions::{
        AddAgent, ChangeConfigPath, ReloadAgentConfig, RemoveAgent, RestartAgent, UpdateAgent,
    },
};

impl SettingsPanel {
    pub fn agent_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new("Agent Servers")
            .resettable(false)
            .groups(vec![
                SettingGroup::new()
                    .title("Configuration")
                    .items(vec![
                        SettingItem::new(
                            "Config File Path",
                            SettingField::render({
                                let view = view.clone();
                                move |_options, _window, cx| {
                                    let config_path = AppState::global(cx)
                                        .agent_config_service()
                                        .map(|s| s.config_path().to_string_lossy().to_string())
                                        .unwrap_or_else(|| "Not configured".to_string());

                                    v_flex()
                                        .w_full()
                                        .gap_2()
                                        .child(
                                            gpui::div()
                                                .w_full()
                                                .overflow_x_hidden()
                                                .child(
                                                    Label::new(config_path)
                                                        .text_sm()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .whitespace_nowrap()
                                                )
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    Button::new("browse-config")
                                                        .label("Browse...")
                                                        .icon(IconName::Folder)
                                                        .outline()
                                                        .small()
                                                        .on_click({
                                                            let view = view.clone();
                                                            move |_, window, cx| {
                                                                view.update(cx, |this, cx| {
                                                                    this.show_config_file_picker(window, cx);
                                                                });
                                                            }
                                                        })
                                                )
                                                .child(
                                                    Button::new("reload-config")
                                                        .label("Reload")
                                                        .icon(IconName::LoaderCircle)
                                                        .outline()
                                                        .small()
                                                        .on_click(move |_, window, cx| {
                                                            window.dispatch_action(
                                                                Box::new(ReloadAgentConfig),
                                                                cx
                                                            );
                                                        })
                                                )
                                        )
                                }
                            }),
                        )
                        .description("Path to agent configuration file (config.json)"),
                        SettingItem::new(
                            "Upload Directory",
                            SettingField::render({
                                let view = view.clone();
                                move |_options, _window, cx| {
                                    let upload_dir = view.read(cx).cached_upload_dir.to_string_lossy().to_string();
                                    let display = if upload_dir.is_empty() {
                                        "Not configured".to_string()
                                    } else {
                                        upload_dir
                                    };

                                    gpui::div()
                                        .w_full()
                                        .min_w(px(0.))
                                        .overflow_x_hidden()
                                        .child(
                                            Label::new(display)
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .whitespace_nowrap()
                                        )
                                }
                            }),
                        )
                        .description("Directory for uploaded files (edit via config.json)"),
                    ]),
                SettingGroup::new()
                    .title("Configured Agents")
                    .item(SettingItem::render({
                        let view = view.clone();
                        move |_options, window, cx| {
                            let agent_configs = view.read(cx).cached_agents.clone();

                            let mut content = v_flex()
                                .w_full()
                                .gap_3()
                                .child(
                                    // Add New Agent button
                                    h_flex()
                                        .w_full()
                                        .justify_end()
                                        .child(
                                            Button::new("add-agent-btn")
                                                .label("Add New Agent")
                                                .icon(IconName::Plus)
                                                .small()
                                                .on_click({
                                                    let view = view.clone();
                                                    move |_, window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            this.show_add_edit_agent_dialog(window, cx, None);
                                                        });
                                                    }
                                                })
                                        )
                                );

                            if agent_configs.is_empty() {
                                content = content.child(
                                    h_flex()
                                        .w_full()
                                        .p_4()
                                        .justify_center()
                                        .child(
                                            Label::new("No agents configured. Click 'Add New Agent' to get started.")
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                        )
                                );
                            } else {
                                for (idx, (name, config)) in agent_configs.iter().enumerate() {
                                    let name_for_edit = name.clone();
                                    let name_for_restart = name.clone();
                                    let name_for_remove = name.clone();

                                    let mut agent_info = v_flex()
                                        .flex_1()
                                        .gap_1()
                                        .child(
                                            Label::new(name.clone())
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                        )
                                        .child(
                                            Label::new(format!("Command: {}", config.command))
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                        );

                                    if !config.args.is_empty() {
                                        agent_info = agent_info.child(
                                            Label::new(format!("Args: {}", config.args.join(" ")))
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                        );
                                    }

                                    if !config.env.is_empty() {
                                        agent_info = agent_info.child(
                                            Label::new(format!("Env vars: {} defined", config.env.len()))
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                        );
                                    }

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
                                            .child(agent_info)
                                            .child(
                                                // Action buttons column
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Button::new(("edit-btn", idx))
                                                            .label("Edit")
                                                            .icon(IconName::Settings)
                                                            .outline()
                                                            .small()
                                                            .on_click({
                                                                let view = view.clone();
                                                                move |_, window, cx| {
                                                                    view.update(cx, |this, cx| {
                                                                        this.show_add_edit_agent_dialog(
                                                                            window,
                                                                            cx,
                                                                            Some(name_for_edit.clone())
                                                                        );
                                                                    });
                                                                }
                                                            })
                                                    )
                                                    .child(
                                                        Button::new(("restart-btn", idx))
                                                            .label("Restart")
                                                            .icon(IconName::LoaderCircle)
                                                            .outline()
                                                            .small()
                                                            .on_click(move |_, window, cx| {
                                                                log::info!("Restart agent: {}", name_for_restart);
                                                                window.dispatch_action(
                                                                    Box::new(RestartAgent {
                                                                        name: name_for_restart.clone(),
                                                                    }),
                                                                    cx
                                                                );
                                                            })
                                                    )
                                                    .child(
                                                        Button::new(("remove-btn", idx))
                                                            .label("Remove")
                                                            .icon(IconName::Delete)
                                                            .outline()
                                                            .small()
                                                            .on_click({
                                                                let view = view.clone();
                                                                move |_, window, cx| {
                                                                    view.update(cx, |this, cx| {
                                                                        this.show_delete_confirm_dialog(
                                                                            window,
                                                                            cx,
                                                                            name_for_remove.clone()
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
            ])
    }

    /// Show dialog to add or edit an agent
    pub fn show_add_edit_agent_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        agent_name: Option<String>,
    ) {
        let is_edit = agent_name.is_some();
        let title = if is_edit {
            "Edit Agent"
        } else {
            "Add New Agent"
        };

        // Get existing config if editing
        let existing_config = agent_name
            .as_ref()
            .and_then(|name| self.cached_agents.get(name).cloned());

        // Create input states
        let name_input = cx.new(|cx| {
            let mut state =
                InputState::new(window, cx).placeholder("Agent name (e.g., Claude Code)");
            if let Some(name) = &agent_name {
                state.set_value(name.clone(), window, cx);
            }
            state
        });

        let command_input = cx.new(|cx| {
            let mut state =
                InputState::new(window, cx).placeholder("Command (e.g., claude-code-acp)");
            if let Some(config) = &existing_config {
                state.set_value(config.command.clone(), window, cx);
            }
            state
        });

        let args_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder("Arguments (space-separated, e.g., --experimental-acp)");
            if let Some(config) = &existing_config {
                state.set_value(config.args.join(" "), window, cx);
            }
            state
        });

        let env_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder("Environment variables (KEY=VALUE, one per line)");
            if let Some(config) = &existing_config {
                let env_text = config
                    .env
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n");
                state.set_value(env_text, window, cx);
            }
            state
        });

        window.open_dialog(cx, move |dialog, window, cx| {
            dialog
                .title(title)
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(if is_edit { "Update" } else { "Add" })
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let name_input = name_input.clone();
                    let command_input = command_input.clone();
                    let args_input = args_input.clone();
                    let env_input = env_input.clone();
                    let agent_name = agent_name.clone();

                    move |_, window, cx| {
                        let name = name_input.read(cx).text().to_string();
                        let name = name.trim();
                        let command = command_input.read(cx).text().to_string();
                        let command = command.trim();
                        let args_text = args_input.read(cx).text().to_string();
                        let env_text = env_input.read(cx).text().to_string();

                        // Validate inputs
                        if name.is_empty() || command.is_empty() {
                            log::warn!("Agent name and command cannot be empty");
                            return false;
                        }

                        // Parse args and env
                        let args: Vec<String> =
                            args_text.split_whitespace().map(String::from).collect();

                        let mut env = HashMap::new();
                        for line in env_text.lines() {
                            if let Some((key, value)) = line.trim().split_once('=') {
                                env.insert(key.trim().to_string(), value.trim().to_string());
                            } else if !line.trim().is_empty() {
                                log::warn!("Invalid env format (should be KEY=VALUE): {}", line);
                                return false;
                            }
                        }

                        // Dispatch appropriate action
                        if is_edit {
                            window.dispatch_action(
                                Box::new(UpdateAgent {
                                    name: name.to_string(),
                                    command: command.to_string(),
                                    args,
                                    env,
                                }),
                                cx,
                            );
                        } else {
                            window.dispatch_action(
                                Box::new(AddAgent {
                                    name: name.to_string(),
                                    command: command.to_string(),
                                    args,
                                    env,
                                }),
                                cx,
                            );
                        }

                        true // Close dialog
                    }
                })
                .child(
                    v_flex()
                        .w_full()
                        .gap_4()
                        .p_4()
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    Label::new("Agent Name")
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD),
                                )
                                .child(
                                    Input::new(&name_input).disabled(is_edit), // Can't change name when editing
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    Label::new("Command")
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD),
                                )
                                .child(Input::new(&command_input))
                                .child(
                                    Label::new("Full path or command name in PATH")
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    Label::new("Arguments (optional)")
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD),
                                )
                                .child(Input::new(&args_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    Label::new("Environment Variables (optional)")
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD),
                                )
                                .child(Input::new(&env_input))
                                .child(
                                    Label::new("One per line, format: KEY=VALUE")
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        ),
                )
        });
    }

    /// Show confirmation dialog before deleting an agent
    pub fn show_delete_confirm_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        agent_name: String,
    ) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let name = agent_name.clone();
            dialog
                .title("Confirm Delete")
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Delete")
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .cancel_text("Cancel")
                )
                .on_ok(move |_, window, cx| {
                    log::info!("Deleting agent: {}", name);
                    window.dispatch_action(Box::new(RemoveAgent { name: name.clone() }), cx);
                    true
                })
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .p_4()
                        .child(Label::new(format!(
                            "Are you sure you want to delete the agent \"{}\"?\n\nThis action cannot be undone.",
                            agent_name
                        )).text_sm())
                )
        });
    }

    /// Show file picker to select config file
    pub fn show_config_file_picker(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let weak_entity = cx.entity().downgrade();

        // Use rfd to open file dialog
        cx.spawn(async move |_this, cx| {
            let task = rfd::AsyncFileDialog::new()
                .set_title("Select Config File")
                .add_filter("JSON", &["json"])
                .set_file_name("config.json")
                .pick_file();

            if let Some(file) = task.await {
                let path = file.path().to_path_buf();
                log::info!("Selected config file: {:?}", path);

                // Dispatch action to change config path
                _ = cx.update(|cx| {
                    if let Some(entity) = weak_entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            cx.dispatch_action(&ChangeConfigPath { path });
                        });
                    }
                });
            }
        })
        .detach();
    }
}
