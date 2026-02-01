use gpui::{App, Entity, SharedString};
use gpui_component::setting::{SettingField, SettingGroup, SettingItem, SettingPage};
use rust_i18n::t;

use super::panel::SettingsPanel;
use crate::AppState;

impl SettingsPanel {
    pub fn network_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new(t!("settings.network.title").to_string())
            .default_open(false)
            .groups(vec![SettingGroup::new()
                .title(t!("settings.network.group.proxy").to_string())
                .items(vec![
                    SettingItem::new(
                        t!("settings.network.proxy.enable.label").to_string(),
                        SettingField::switch(
                            {
                                let view = view.clone();
                                move |cx: &App| view.read(cx).cached_proxy.enabled
                            },
                            {
                                let view = view.clone();
                                move |val: bool, cx: &mut App| {
                                    view.update(cx, |this, cx| {
                                        this.cached_proxy.enabled = val;
                                        cx.notify();
                                    });

                                    if let Some(service) =
                                        AppState::global(cx).agent_config_service()
                                    {
                                        let service = service.clone();
                                        let proxy = view.read(cx).cached_proxy.clone();
                                        let _ = cx.spawn(async move |_cx| {
                                            if let Err(err) =
                                                service.update_proxy_config(proxy).await
                                            {
                                                log::error!(
                                                    "Failed to update proxy config: {}",
                                                    err
                                                );
                                            }
                                        });
                                    }
                                }
                            },
                        )
                        .default_value(false),
                    )
                    .description(t!("settings.network.proxy.enable.description").to_string()),
                    SettingItem::new(
                        t!("settings.network.proxy.http.label").to_string(),
                        SettingField::input(
                            {
                                let view = view.clone();
                                move |cx: &App| {
                                    SharedString::from(
                                        view.read(cx).cached_proxy.http_proxy_url.clone(),
                                    )
                                }
                            },
                            {
                                let view = view.clone();
                                move |val: SharedString, cx: &mut App| {
                                    view.update(cx, |this, cx| {
                                        this.cached_proxy.http_proxy_url = val.to_string();
                                        cx.notify();
                                    });

                                    if let Some(service) =
                                        AppState::global(cx).agent_config_service()
                                    {
                                        let service = service.clone();
                                        let proxy = view.read(cx).cached_proxy.clone();
                                        let _ = cx.spawn(async move |_cx| {
                                            if let Err(err) =
                                                service.update_proxy_config(proxy).await
                                            {
                                                log::error!(
                                                    "Failed to update proxy config: {}",
                                                    err
                                                );
                                            }
                                        });
                                    }
                                }
                            },
                        )
                        .default_value(SharedString::from("")),
                    )
                    .description(t!("settings.network.proxy.http.description").to_string()),
                    SettingItem::new(
                        t!("settings.network.proxy.https.label").to_string(),
                        SettingField::input(
                            {
                                let view = view.clone();
                                move |cx: &App| {
                                    SharedString::from(
                                        view.read(cx).cached_proxy.https_proxy_url.clone(),
                                    )
                                }
                            },
                            {
                                let view = view.clone();
                                move |val: SharedString, cx: &mut App| {
                                    view.update(cx, |this, cx| {
                                        this.cached_proxy.https_proxy_url = val.to_string();
                                        cx.notify();
                                    });

                                    if let Some(service) =
                                        AppState::global(cx).agent_config_service()
                                    {
                                        let service = service.clone();
                                        let proxy = view.read(cx).cached_proxy.clone();
                                        let _ = cx.spawn(async move |_cx| {
                                            if let Err(err) =
                                                service.update_proxy_config(proxy).await
                                            {
                                                log::error!(
                                                    "Failed to update proxy config: {}",
                                                    err
                                                );
                                            }
                                        });
                                    }
                                }
                            },
                        )
                        .default_value(SharedString::from("")),
                    )
                    .description(t!("settings.network.proxy.https.description").to_string()),
                    SettingItem::new(
                        t!("settings.network.proxy.all.label").to_string(),
                        SettingField::input(
                            {
                                let view = view.clone();
                                move |cx: &App| {
                                    SharedString::from(
                                        view.read(cx).cached_proxy.all_proxy_url.clone(),
                                    )
                                }
                            },
                            {
                                let view = view.clone();
                                move |val: SharedString, cx: &mut App| {
                                    view.update(cx, |this, cx| {
                                        this.cached_proxy.all_proxy_url = val.to_string();
                                        cx.notify();
                                    });

                                    if let Some(service) =
                                        AppState::global(cx).agent_config_service()
                                    {
                                        let service = service.clone();
                                        let proxy = view.read(cx).cached_proxy.clone();
                                        let _ = cx.spawn(async move |_cx| {
                                            if let Err(err) =
                                                service.update_proxy_config(proxy).await
                                            {
                                                log::error!(
                                                    "Failed to update proxy config: {}",
                                                    err
                                                );
                                            }
                                        });
                                    }
                                }
                            },
                        )
                        .default_value(SharedString::from("")),
                    )
                    .description(t!("settings.network.proxy.all.description").to_string()),
                ])])
    }
}
