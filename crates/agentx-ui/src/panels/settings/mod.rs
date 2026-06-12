//! The settings panel — pages of config-backed fields built on gpui-component's
//! `Settings` DSL.
//!
//! Holds a cached [`Config`] snapshot the field getters read; setters mutate the
//! snapshot and persist it through an optional [`ConfigService`] (absent in the
//! preview gallery, where edits simply don't save). Agent process management
//! (restart) and add/remove dialogs are deferred — v1 edits existing entries.

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Root,
    dock::{Panel, PanelEvent},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};

use agentx_app::ConfigService;
use agentx_domain::Config;

/// Open the settings window, loading the current config through the service so
/// edits persist back to disk.
pub fn open_settings_window(config_service: Arc<ConfigService>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let config = config_service.load().await.unwrap_or_default();
        let _ = cx.update(|cx| {
            let bounds = Bounds::centered(None, size(px(820.0), px(640.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            let _ = cx.open_window(options, |window, cx| {
                let panel = cx.new(|cx| SettingsPanel::new(config, Some(config_service), cx));
                cx.new(|cx| Root::new(panel, window, cx).bg(cx.theme().background))
            });
        });
    })
    .detach();
}

pub struct SettingsPanel {
    config: Config,
    config_service: Option<Arc<ConfigService>>,
    focus_handle: FocusHandle,
}

impl SettingsPanel {
    pub fn new(
        config: Config,
        config_service: Option<Arc<ConfigService>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            config,
            config_service,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Persist the current snapshot (no-op when no service is wired, e.g. preview).
    fn persist(&self, cx: &mut Context<Self>) {
        if let Some(service) = self.config_service.clone() {
            let config = self.config.clone();
            cx.spawn(async move |_, _| {
                let _ = service.save(config).await;
            })
            .detach();
        }
    }

    fn general_page(&self, view: &Entity<Self>) -> SettingPage {
        SettingPage::new("General").groups(vec![SettingGroup::new().title("Workspace").items(
            vec![
                SettingItem::new(
                    "Upload directory",
                    SettingField::input(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                SharedString::from(
                                    view.read(cx).config.upload_dir.to_string_lossy().to_string(),
                                )
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: SharedString, cx: &mut App| {
                                view.update(cx, |this, cx| {
                                    this.config.upload_dir = PathBuf::from(val.to_string());
                                    this.persist(cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
                .description("Where pasted images and uploads are stored."),
                SettingItem::new(
                    "Tool-call preview lines",
                    SettingField::input(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                SharedString::from(
                                    view.read(cx).config.tool_call_preview_max_lines.to_string(),
                                )
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: SharedString, cx: &mut App| {
                                if let Ok(lines) = val.trim().parse::<usize>() {
                                    view.update(cx, |this, cx| {
                                        this.config.tool_call_preview_max_lines = lines;
                                        this.persist(cx);
                                        cx.notify();
                                    });
                                }
                            }
                        },
                    ),
                )
                .description("Max lines shown in a collapsed tool-call preview."),
            ],
        )])
    }

    fn network_page(&self, view: &Entity<Self>) -> SettingPage {
        let proxy_input = |getter: fn(&Config) -> String,
                           setter: fn(&mut Config, String),
                           view: &Entity<Self>| {
            let g_view = view.clone();
            let s_view = view.clone();
            SettingField::input(
                move |cx: &App| SharedString::from(getter(&g_view.read(cx).config)),
                move |val: SharedString, cx: &mut App| {
                    s_view.update(cx, |this, cx| {
                        setter(&mut this.config, val.to_string());
                        this.persist(cx);
                        cx.notify();
                    });
                },
            )
        };

        SettingPage::new("Network").groups(vec![SettingGroup::new().title("Proxy").items(vec![
            SettingItem::new(
                "Enable proxy",
                SettingField::switch(
                    {
                        let view = view.clone();
                        move |cx: &App| view.read(cx).config.proxy.enabled
                    },
                    {
                        let view = view.clone();
                        move |val: bool, cx: &mut App| {
                            view.update(cx, |this, cx| {
                                this.config.proxy.enabled = val;
                                this.persist(cx);
                                cx.notify();
                            });
                        }
                    },
                ),
            )
            .description("Route agent subprocesses through an HTTP proxy."),
            SettingItem::new(
                "HTTP proxy",
                proxy_input(
                    |c| c.proxy.http_proxy_url.clone(),
                    |c, v| c.proxy.http_proxy_url = v,
                    view,
                ),
            ),
            SettingItem::new(
                "HTTPS proxy",
                proxy_input(
                    |c| c.proxy.https_proxy_url.clone(),
                    |c, v| c.proxy.https_proxy_url = v,
                    view,
                ),
            ),
            SettingItem::new(
                "All proxy",
                proxy_input(
                    |c| c.proxy.all_proxy_url.clone(),
                    |c, v| c.proxy.all_proxy_url = v,
                    view,
                ),
            ),
        ])])
    }

    fn agent_page(&self, view: &Entity<Self>) -> SettingPage {
        let mut names: Vec<String> = self.config.agents.keys().cloned().collect();
        names.sort();
        let items = names
            .into_iter()
            .map(|name| {
                let getter_name = name.clone();
                let setter_name = name.clone();
                SettingItem::new(
                    name.clone(),
                    SettingField::input(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                SharedString::from(
                                    view.read(cx)
                                        .config
                                        .agents
                                        .get(&getter_name)
                                        .map(|agent| agent.command.clone())
                                        .unwrap_or_default(),
                                )
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: SharedString, cx: &mut App| {
                                view.update(cx, |this, cx| {
                                    if let Some(agent) = this.config.agents.get_mut(&setter_name) {
                                        agent.command = val.to_string();
                                    }
                                    this.persist(cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
                .description("Launch command")
            })
            .collect::<Vec<_>>();
        SettingPage::new("Agents").groups(vec![
            SettingGroup::new().title("Configured agents").items(items),
        ])
    }

    fn model_page(&self, view: &Entity<Self>) -> SettingPage {
        let mut names: Vec<String> = self.config.models.keys().cloned().collect();
        names.sort();
        let items = names
            .into_iter()
            .map(|name| {
                let getter_name = name.clone();
                let setter_name = name.clone();
                SettingItem::new(
                    name.clone(),
                    SettingField::switch(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                view.read(cx)
                                    .config
                                    .models
                                    .get(&getter_name)
                                    .map(|model| model.enabled)
                                    .unwrap_or(false)
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: bool, cx: &mut App| {
                                view.update(cx, |this, cx| {
                                    if let Some(model) = this.config.models.get_mut(&setter_name) {
                                        model.enabled = val;
                                    }
                                    this.persist(cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
                .description("Enabled")
            })
            .collect::<Vec<_>>();
        SettingPage::new("Models").groups(vec![SettingGroup::new().title("Models").items(items)])
    }

    fn mcp_page(&self, view: &Entity<Self>) -> SettingPage {
        let mut names: Vec<String> = self.config.mcp_servers.keys().cloned().collect();
        names.sort();
        let items = names
            .into_iter()
            .map(|name| {
                let getter_name = name.clone();
                let setter_name = name.clone();
                SettingItem::new(
                    name.clone(),
                    SettingField::checkbox(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                view.read(cx)
                                    .config
                                    .mcp_servers
                                    .get(&getter_name)
                                    .map(|server| server.enabled)
                                    .unwrap_or(false)
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: bool, cx: &mut App| {
                                view.update(cx, |this, cx| {
                                    if let Some(server) =
                                        this.config.mcp_servers.get_mut(&setter_name)
                                    {
                                        server.enabled = val;
                                    }
                                    this.persist(cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
                .description("Enabled")
            })
            .collect::<Vec<_>>();
        SettingPage::new("MCP").groups(vec![SettingGroup::new().title("MCP servers").items(items)])
    }

    fn command_page(&self, view: &Entity<Self>) -> SettingPage {
        let mut names: Vec<String> = self.config.commands.keys().cloned().collect();
        names.sort();
        let items = names
            .into_iter()
            .map(|name| {
                let getter_name = name.clone();
                let setter_name = name.clone();
                SettingItem::new(
                    name.clone(),
                    SettingField::input(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                SharedString::from(
                                    view.read(cx)
                                        .config
                                        .commands
                                        .get(&getter_name)
                                        .map(|command| command.template.clone())
                                        .unwrap_or_default(),
                                )
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: SharedString, cx: &mut App| {
                                view.update(cx, |this, cx| {
                                    if let Some(command) =
                                        this.config.commands.get_mut(&setter_name)
                                    {
                                        command.template = val.to_string();
                                    }
                                    this.persist(cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
                .description("Template")
            })
            .collect::<Vec<_>>();
        SettingPage::new("Commands")
            .groups(vec![SettingGroup::new().title("Commands").items(items)])
    }

    fn prompt_page(&self, view: &Entity<Self>) -> SettingPage {
        let mut names: Vec<String> = self.config.system_prompts.keys().cloned().collect();
        names.sort();
        let items = names
            .into_iter()
            .map(|name| {
                let getter_name = name.clone();
                let setter_name = name.clone();
                SettingItem::new(
                    name.clone(),
                    SettingField::input(
                        {
                            let view = view.clone();
                            move |cx: &App| {
                                SharedString::from(
                                    view.read(cx)
                                        .config
                                        .system_prompts
                                        .get(&getter_name)
                                        .cloned()
                                        .unwrap_or_default(),
                                )
                            }
                        },
                        {
                            let view = view.clone();
                            move |val: SharedString, cx: &mut App| {
                                view.update(cx, |this, cx| {
                                    this.config
                                        .system_prompts
                                        .insert(setter_name.clone(), val.to_string());
                                    this.persist(cx);
                                    cx.notify();
                                });
                            }
                        },
                    ),
                )
            })
            .collect::<Vec<_>>();
        SettingPage::new("Prompts").groups(vec![
            SettingGroup::new().title("System prompts").items(items),
        ])
    }

    fn update_page(&self, _view: &Entity<Self>) -> SettingPage {
        SettingPage::new("Update").groups(vec![SettingGroup::new().title("Updates").items(vec![
            SettingItem::new(
                "Status",
                SettingField::render(|_options, _window, _cx| {
                    div().text_sm().child("Up to date")
                }),
            )
            .description("Automatic updates are not configured in this build."),
        ])])
    }

    fn about_page(&self, _view: &Entity<Self>) -> SettingPage {
        SettingPage::new("About").groups(vec![SettingGroup::new().title("About").items(vec![
            SettingItem::new(
                "AgentX",
                SettingField::render(|_options, _window, _cx| {
                    div()
                        .text_sm()
                        .child(format!("AgentX UI v{}", env!("CARGO_PKG_VERSION")))
                }),
            )
            .description("A GPU-accelerated desktop AI agent studio."),
        ])])
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().background;
        let view = cx.entity();
        v_flex()
            .size_full()
            .bg(background)
            .child(Settings::new("agentx-settings").pages(vec![
                self.general_page(&view),
                self.network_page(&view),
                self.agent_page(&view),
                self.model_page(&view),
                self.mcp_page(&view),
                self.command_page(&view),
                self.prompt_page(&view),
                self.update_page(&view),
                self.about_page(&view),
            ]))
    }
}

impl EventEmitter<PanelEvent> for SettingsPanel {}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for SettingsPanel {
    fn panel_name(&self) -> &'static str {
        "SettingsPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Settings"
    }
}
