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

use super::panel::SettingsPanel;
use crate::AppState;

impl SettingsPanel {
    pub fn command_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new("Commands").resettable(false).groups(vec![
            SettingGroup::new()
                .title("Custom Commands")
                .item(SettingItem::render({
                    let view = view.clone();
                    move |_options, _window, cx| {
                        let command_configs = view.read(cx).cached_commands.clone();

                        let mut content = v_flex().w_full().gap_3().child(
                            h_flex().w_full().justify_end().child(
                                Button::new("add-command-btn")
                                    .label("Add Command")
                                    .icon(IconName::Plus)
                                    .small()
                                    .on_click({
                                        let view = view.clone();
                                        move |_, window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.show_add_command_dialog(window, cx);
                                            });
                                        }
                                    }),
                            ),
                        );

                        if command_configs.is_empty() {
                            content = content.child(
                            h_flex().w_full().p_4().justify_center().child(
                                Label::new(
                                    "No commands configured. Click 'Add Command' to get started.",
                                )
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                            ),
                        );
                        } else {
                            for (idx, (name, config)) in command_configs.iter().enumerate() {
                                let name_for_edit = name.clone();
                                let name_for_delete = name.clone();

                                let command_info = v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        Label::new(format!("/{}", name))
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD),
                                    )
                                    .child(
                                        Label::new(config.description.clone())
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
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
                                        .child(command_info)
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    Button::new(("edit-command-btn", idx))
                                                        .label("Edit")
                                                        .icon(IconName::Settings)
                                                        .outline()
                                                        .small()
                                                        .on_click({
                                                            let view = view.clone();
                                                            move |_, window, cx| {
                                                                view.update(cx, |this, cx| {
                                                                    this.show_edit_command_dialog(
                                                                        window,
                                                                        cx,
                                                                        name_for_edit.clone(),
                                                                    );
                                                                });
                                                            }
                                                        }),
                                                )
                                                .child(
                                                    Button::new(("delete-command-btn", idx))
                                                        .label("Delete")
                                                        .icon(IconName::Delete)
                                                        .outline()
                                                        .small()
                                                        .on_click({
                                                            let view = view.clone();
                                                            move |_, window, cx| {
                                                                view.update(cx, |this, cx| {
                                                                    this.show_delete_command_dialog(
                                                                    window,
                                                                    cx,
                                                                    name_for_delete.clone(),
                                                                );
                                                                });
                                                            }
                                                        }),
                                                ),
                                        ),
                                );
                            }
                        }

                        content
                    }
                })),
        ])
    }

    pub fn show_add_command_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Command name (without /)"));
        let desc_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));
        let template_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Template/Content"));

        window.open_dialog(cx, move |dialog, _window, cx| {
            dialog
                .title("Add Custom Command")
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Add")
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let name_input = name_input.clone();
                    let desc_input = desc_input.clone();
                    let template_input = template_input.clone();

                    move |_, _window, cx| {
                        let name = name_input.read(cx).text().to_string().trim().to_string();
                        let desc = desc_input.read(cx).text().to_string().trim().to_string();
                        let template = template_input
                            .read(cx)
                            .text()
                            .to_string()
                            .trim()
                            .to_string();

                        if name.is_empty() || desc.is_empty() || template.is_empty() {
                            log::warn!("Name, description, and template cannot be empty");
                            return false;
                        }

                        // Save to config file
                        if let Some(service) = AppState::global(cx).agent_config_service() {
                            let service = service.clone();
                            let config = crate::core::config::CommandConfig {
                                description: desc,
                                template,
                            };

                            cx.spawn(async move |cx| {
                                match service.add_command(name.clone(), config).await {
                                    Ok(_) => {
                                        log::info!("Successfully added command: {}", name);
                                        _ = cx.update(|_cx| {});
                                    }
                                    Err(e) => {
                                        log::error!("Failed to add command: {}", e);
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
                                .child(Label::new("Command Name"))
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
                                .child(Label::new("Template"))
                                .child(Input::new(&template_input)),
                        ),
                )
        });
    }

    pub fn show_edit_command_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        command_name: String,
    ) {
        let config = self.cached_commands.get(&command_name).cloned();
        if config.is_none() {
            log::warn!("Command config not found: {}", command_name);
            return;
        }
        let config = config.unwrap();
        let entity = cx.entity().downgrade();

        let desc_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.description.clone(), window, cx);
            state
        });
        let template_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.template.clone(), window, cx);
            state
        });

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(format!("Edit Command: /{}", command_name))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Save")
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let desc_input = desc_input.clone();
                    let template_input = template_input.clone();
                    let command_name = command_name.clone();
                    let entity = entity.clone();

                    move |_, _window, cx| {
                        let desc = desc_input.read(cx).text().to_string().trim().to_string();
                        let template = template_input
                            .read(cx)
                            .text()
                            .to_string()
                            .trim()
                            .to_string();

                        if desc.is_empty() || template.is_empty() {
                            log::warn!("Description and template cannot be empty");
                            return false;
                        }

                        // Save to config file
                        if let Some(service) = AppState::global(cx).agent_config_service() {
                            let service = service.clone();
                            let command_name_for_async = command_name.clone();
                            let config = crate::core::config::CommandConfig {
                                description: desc,
                                template,
                            };
                            let entity = entity.clone();

                            cx.spawn(async move |cx| {
                                match service
                                    .update_command(&command_name_for_async, config.clone())
                                    .await
                                {
                                    Ok(_) => {
                                        log::info!(
                                            "Successfully updated command: {}",
                                            command_name_for_async
                                        );
                                        // Update UI
                                        _ = cx.update(|cx| {
                                            if let Some(panel) = entity.upgrade() {
                                                panel.update(cx, |this, cx| {
                                                    this.cached_commands
                                                        .insert(command_name_for_async, config);
                                                    cx.notify();
                                                });
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        log::error!("Failed to update command: {}", e);
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
                                .child(Label::new("Description"))
                                .child(Input::new(&desc_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Template"))
                                .child(Input::new(&template_input)),
                        ),
                )
        });
    }

    pub fn show_delete_command_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        command_name: String,
    ) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let name = command_name.clone();
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
                            if let Err(e) = service.remove_command(&name).await {
                                log::error!("Failed to delete command: {}", e);
                            } else {
                                log::info!("Successfully deleted command: {}", name);
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
                            "Are you sure you want to delete the command \"/{}\"?",
                            command_name
                        ))
                        .text_sm(),
                    ),
                )
        });
    }
}
