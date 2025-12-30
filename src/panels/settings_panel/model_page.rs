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
    pub fn model_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new("Models").resettable(false).groups(vec![
            SettingGroup::new()
                .title("Model Providers")
                .item(SettingItem::render({
                    let view = view.clone();
                    move |_options, _window, cx| {
                        let model_configs = view.read(cx).cached_models.clone();

                        let mut content = v_flex().w_full().gap_3().child(
                            h_flex().w_full().justify_end().child(
                                Button::new("add-model-btn")
                                    .label("Add Model")
                                    .icon(IconName::Plus)
                                    .small()
                                    .on_click({
                                        let view = view.clone();
                                        move |_, window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.show_add_model_dialog(window, cx);
                                            });
                                        }
                                    }),
                            ),
                        );

                        if model_configs.is_empty() {
                            content = content.child(
                                h_flex().w_full().p_4().justify_center().child(
                                    Label::new(
                                        "No models configured. Click 'Add Model' to get started.",
                                    )
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                                ),
                            );
                        } else {
                            for (idx, (name, config)) in model_configs.iter().enumerate() {
                                let name_for_edit = name.clone();
                                let name_for_delete = name.clone();

                                let mut model_info = v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        Label::new(name.clone())
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD),
                                    )
                                    .child(
                                        Label::new(format!("Provider: {}", config.provider))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        Label::new(format!("URL: {}", config.base_url))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    );

                                if !config.model_name.is_empty() {
                                    model_info = model_info.child(
                                        Label::new(format!("Model: {}", config.model_name))
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
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
                                        .child(model_info)
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    Label::new(if config.enabled {
                                                        "Enabled"
                                                    } else {
                                                        "Disabled"
                                                    })
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground),
                                                )
                                                .child(
                                                    Button::new(("edit-model-btn", idx))
                                                        .label("Edit")
                                                        .icon(IconName::Settings)
                                                        .outline()
                                                        .small()
                                                        .on_click({
                                                            let view = view.clone();
                                                            move |_, window, cx| {
                                                                view.update(cx, |this, cx| {
                                                                    this.show_edit_model_dialog(
                                                                        window,
                                                                        cx,
                                                                        name_for_edit.clone(),
                                                                    );
                                                                });
                                                            }
                                                        }),
                                                )
                                                .child(
                                                    Button::new(("delete-model-btn", idx))
                                                        .label("Delete")
                                                        .icon(IconName::Delete)
                                                        .outline()
                                                        .small()
                                                        .on_click({
                                                            let view = view.clone();
                                                            move |_, window, cx| {
                                                                view.update(cx, |this, cx| {
                                                                    this.show_delete_model_dialog(
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

    pub fn show_add_model_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Model name (e.g., GPT-4)"));
        let provider_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider (e.g., OpenAI)"));
        let url_input = cx.new(|cx| InputState::new(window, cx).placeholder("Base URL"));
        let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("API Key"));
        let model_input = cx.new(|cx| InputState::new(window, cx).placeholder("Model name"));
        let entity = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("Add Model Configuration")
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Add")
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let name_input = name_input.clone();
                    let provider_input = provider_input.clone();
                    let url_input = url_input.clone();
                    let key_input = key_input.clone();
                    let model_input = model_input.clone();
                    let entity = entity.clone();

                    move |_, _window, cx| {
                        let name = name_input.read(cx).text().to_string().trim().to_string();
                        let provider = provider_input
                            .read(cx)
                            .text()
                            .to_string()
                            .trim()
                            .to_string();
                        let url = url_input.read(cx).text().to_string().trim().to_string();
                        let key = key_input.read(cx).text().to_string().trim().to_string();
                        let model = model_input.read(cx).text().to_string().trim().to_string();

                        if name.is_empty() || provider.is_empty() || url.is_empty() {
                            log::warn!("Name, provider, and URL cannot be empty");
                            return false;
                        }

                        // Save to config file
                        if let Some(service) = AppState::global(cx).agent_config_service() {
                            let service = service.clone();
                            let config = crate::core::config::ModelConfig {
                                enabled: true,
                                provider,
                                base_url: url,
                                api_key: key,
                                model_name: model,
                            };
                            let name_clone = name.clone();
                            let entity = entity.clone();

                            cx.spawn(async move |cx| {
                                match service.add_model(name_clone.clone(), config.clone()).await {
                                    Ok(_) => {
                                        log::info!("Successfully added model: {}", name_clone);
                                        // Update UI
                                        _ = cx.update(|cx| {
                                            if let Some(panel) = entity.upgrade() {
                                                panel.update(cx, |this, cx| {
                                                    this.cached_models.insert(name_clone, config);
                                                    cx.notify();
                                                });
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        log::error!("Failed to add model: {}", e);
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
                                .child(Label::new("Provider"))
                                .child(Input::new(&provider_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Base URL"))
                                .child(Input::new(&url_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("API Key"))
                                .child(Input::new(&key_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Model Name"))
                                .child(Input::new(&model_input)),
                        ),
                )
        });
    }

    pub fn show_edit_model_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        model_name: String,
    ) {
        let Some(config) = self.cached_models.get(&model_name).cloned() else {
            log::warn!("Model config not found: {}", model_name);
            return;
        };

        let provider_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.provider.clone(), window, cx);
            state
        });
        let url_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.base_url.clone(), window, cx);
            state
        });
        let key_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.api_key.clone(), window, cx);
            state
        });
        let model_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(config.model_name.clone(), window, cx);
            state
        });
        let enabled = config.enabled;

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title(format!("Edit Model: {}", model_name))
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Save")
                        .cancel_text("Cancel"),
                )
                .on_ok({
                    let provider_input = provider_input.clone();
                    let url_input = url_input.clone();
                    let key_input = key_input.clone();
                    let model_input = model_input.clone();
                    let model_name = model_name.clone();

                    move |_, _window, cx| {
                        let provider = provider_input.read(cx).text().to_string();
                        let provider = provider.trim();
                        let url = url_input.read(cx).text().to_string();
                        let url = url.trim();
                        let key = key_input.read(cx).text().to_string();
                        let key = key.trim();
                        let model = model_input.read(cx).text().to_string();
                        let model = model.trim();

                        if provider.is_empty() || url.is_empty() {
                            log::warn!("Provider and URL cannot be empty");
                            return false;
                        }

                        if let Some(service) = AppState::global(cx).agent_config_service() {
                            let service = service.clone();
                            let name = model_name.clone();
                            let config = crate::core::config::ModelConfig {
                                enabled,
                                provider: provider.to_string(),
                                base_url: url.to_string(),
                                api_key: key.to_string(),
                                model_name: model.to_string(),
                            };

                            cx.spawn(async move |cx| {
                                if let Err(e) = service.update_model(&name, config).await {
                                    log::error!("Failed to update model: {}", e);
                                } else {
                                    log::info!("Successfully updated model: {}", name);
                                }
                                let _ = cx.update(|_cx| {});
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
                                .child(Label::new("Provider"))
                                .child(Input::new(&provider_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Base URL"))
                                .child(Input::new(&url_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("API Key"))
                                .child(Input::new(&key_input)),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(Label::new("Model Name"))
                                .child(Input::new(&model_input)),
                        ),
                )
        });
    }

    pub fn show_delete_model_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        model_name: String,
    ) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let name = model_name.clone();
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
                            if let Err(e) = service.remove_model(&name).await {
                                log::error!("Failed to delete model: {}", e);
                            } else {
                                log::info!("Successfully deleted model: {}", name);
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
                            "Are you sure you want to delete the model \"{}\"?",
                            model_name
                        ))
                        .text_sm(),
                    ),
                )
        });
    }
}
