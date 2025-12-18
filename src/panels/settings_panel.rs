use gpui::{
    App, AppContext, Axis, Context, Element, Entity, FocusHandle, Focusable, Global, IntoElement,
    ParentElement as _, Render, SharedString, Styled, Window, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, Size, Theme, ThemeMode, WindowExt as _,
    button::Button,
    dialog::DialogButtonProps,
    group_box::GroupBoxVariant,
    h_flex,
    input::{Input, InputState},
    label::Label,
    setting::{
        NumberFieldOptions, RenderOptions, SettingField, SettingFieldElement, SettingGroup,
        SettingItem, SettingPage, Settings,
    },
    text::TextView,
    v_flex,
};

use crate::{
    AppState,
    app::actions::{
        AddAgent, ChangeConfigPath, ReloadAgentConfig, RemoveAgent, RestartAgent, UpdateAgent,
    },
    core::{
        config::AgentProcessConfig,
        updater::{UpdateCheckResult, UpdateManager, Version},
    },
};
use std::{collections::HashMap, path::PathBuf};

struct AppSettings {
    auto_switch_theme: bool,
    cli_path: SharedString,
    font_family: SharedString,
    font_size: f64,
    line_height: f64,
    notifications_enabled: bool,
    auto_update: bool,
    auto_check_on_startup: bool,
    check_frequency_days: f64,
    resettable: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum UpdateStatus {
    Idle,
    Checking,
    Available { version: String, notes: String },
    NoUpdate,
    Error(String),
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_switch_theme: false,
            cli_path: "/usr/local/bin/bash".into(),
            font_family: "Arial".into(),
            font_size: 14.0,
            line_height: 12.0,
            notifications_enabled: true,
            auto_update: true,
            auto_check_on_startup: true,
            check_frequency_days: 7.0,
            resettable: true,
        }
    }
}

impl Global for AppSettings {}

impl AppSettings {
    fn global(cx: &App) -> &AppSettings {
        cx.global::<AppSettings>()
    }

    pub fn global_mut(cx: &mut App) -> &mut AppSettings {
        cx.global_mut::<AppSettings>()
    }
}

pub struct SettingsPanel {
    focus_handle: FocusHandle,
    group_variant: GroupBoxVariant,
    size: Size,
    update_status: UpdateStatus,
    update_manager: UpdateManager,
    // Agent configuration state
    agent_configs: HashMap<String, AgentProcessConfig>,
    upload_dir: String,
    config_path: String,
}

struct OpenURLSettingField {
    label: SharedString,
    url: SharedString,
}

impl OpenURLSettingField {
    fn new(label: impl Into<SharedString>, url: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
        }
    }
}

impl SettingFieldElement for OpenURLSettingField {
    type Element = Button;
    fn render_field(&self, options: &RenderOptions, _: &mut Window, _: &mut App) -> Self::Element {
        let url = self.url.clone();
        Button::new("open-url")
            .outline()
            .label(self.label.clone())
            .with_size(options.size)
            .on_click(move |_, _window, cx| {
                cx.open_url(url.as_str());
            })
    }
}

impl crate::panels::dock_panel::DockPanel for SettingsPanel {
    fn title() -> &'static str {
        "Settings"
    }

    fn description() -> &'static str {
        "A collection of settings groups and items for the application."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> gpui::Pixels {
        px(0.)
    }
}

impl SettingsPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.set_global::<AppSettings>(AppSettings::default());

        // Initialize with empty state - will be loaded async
        let agent_configs = HashMap::new();
        let upload_dir = String::new();
        let config_path = String::new();

        let panel = Self {
            focus_handle: cx.focus_handle(),
            group_variant: GroupBoxVariant::Outline,
            size: Size::default(),
            update_status: UpdateStatus::Idle,
            update_manager: UpdateManager::default(),
            agent_configs,
            upload_dir,
            config_path,
        };

        // Load agent configs asynchronously
        let weak_entity = cx.entity().downgrade();
        if let Some(service) = AppState::global(cx).agent_config_service() {
            let service = service.clone();
            cx.spawn_in(window, async move |_this, window| {
                let agents = service.list_agents().await;
                let upload_dir = service.get_upload_dir().await;

                _ = window.update(|_window, cx| {
                    if let Some(entity) = weak_entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.agent_configs = agents.into_iter().collect();
                            this.upload_dir = upload_dir.to_string_lossy().to_string();
                            cx.notify();
                        });
                    }
                });
            })
            .detach();
        }

        // Load config path from AppState
        let config_path = AppState::global(cx)
            .agent_config_service()
            .and_then(|_| {
                // Get config path from Settings or default
                Some(
                    std::env::current_dir()
                        .ok()?
                        .join("config.json")
                        .to_string_lossy()
                        .to_string(),
                )
            })
            .unwrap_or_default();

        let weak_entity_path = cx.entity().downgrade();
        cx.spawn_in(window, async move |_this, window| {
            _ = window.update(|_window, cx| {
                if let Some(entity) = weak_entity_path.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.config_path = config_path;
                        cx.notify();
                    });
                }
            });
        })
        .detach();

        // Subscribe to AgentConfigBus for dynamic updates
        let agent_config_bus = AppState::global(cx).agent_config_bus.clone();
        let weak_entity = cx.entity().downgrade();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        agent_config_bus.subscribe(move |event| {
            let _ = tx.send(event.clone());
        });

        cx.spawn_in(window, async move |_this, window| {
            while let Some(event) = rx.recv().await {
                if let Some(entity) = weak_entity.upgrade() {
                    _ = window.update(|_window, cx| {
                        entity.update(cx, |this, cx| {
                            this.on_agent_config_event(&event, cx);
                        });
                    });
                } else {
                    break;
                }
            }
        })
        .detach();

        panel
    }

    /// Show dialog to add or edit an agent
    fn show_add_edit_agent_dialog(
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
            .and_then(|name| self.agent_configs.get(name).cloned());

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
                        let name = name_input.read(cx).text().to_string().trim().to_string();
                        let command = command_input.read(cx).text().to_string().trim().to_string();
                        let args_text = args_input.read(cx).text().to_string();
                        let env_text = env_input.read(cx).text().to_string();

                        // Validate inputs
                        if name.is_empty() {
                            log::warn!("Agent name cannot be empty");
                            return false; // Don't close dialog
                        }

                        if command.is_empty() {
                            log::warn!("Command cannot be empty");
                            return false;
                        }

                        // Parse args (split by whitespace, ignore empty strings)
                        let args: Vec<String> = args_text
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect();

                        // Parse env (KEY=VALUE format, one per line)
                        let mut env = HashMap::new();
                        for line in env_text.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }
                            if let Some((key, value)) = line.split_once('=') {
                                env.insert(key.trim().to_string(), value.trim().to_string());
                            } else {
                                log::warn!("Invalid env format (should be KEY=VALUE): {}", line);
                                return false;
                            }
                        }

                        // Dispatch appropriate action
                        if is_edit {
                            window.dispatch_action(
                                Box::new(UpdateAgent {
                                    name,
                                    command,
                                    args,
                                    env,
                                }),
                                cx,
                            );
                        } else {
                            window.dispatch_action(
                                Box::new(AddAgent {
                                    name,
                                    command,
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
    fn show_delete_confirm_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        agent_name: String,
    ) {
        window.open_dialog(cx, move |dialog, window, cx| {
            dialog
                .title("Confirm Delete")
                .confirm()
                .button_props(
                    DialogButtonProps::default()
                        .ok_text("Delete")
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .cancel_text("Cancel")
                )
                .on_ok({
                    let agent_name = agent_name.clone();
                    move |_, window, cx| {
                        log::info!("Deleting agent: {}", agent_name);
                        window.dispatch_action(
                            Box::new(RemoveAgent {
                                name: agent_name.clone(),
                            }),
                            cx
                        );
                        true  // Close dialog
                    }
                })
                .child(
                    v_flex()
                        .w_full()
                        .gap_3()
                        .p_4()
                        .child(
                            Label::new(format!("Are you sure you want to delete the agent \"{}\"?", agent_name))
                                .text_sm()
                        )
                        .child(
                            Label::new("This action cannot be undone. The agent process will be terminated.")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                        )
                )
        });
    }

    /// Show file picker to select config file
    fn show_config_file_picker(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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

    /// Handle agent configuration events
    fn on_agent_config_event(
        &mut self,
        event: &crate::core::event_bus::agent_config_bus::AgentConfigEvent,
        cx: &mut Context<Self>,
    ) {
        use crate::core::event_bus::agent_config_bus::AgentConfigEvent;

        log::info!("[SettingsPanel] Received agent config event: {:?}", event);

        // Reload all configs from service asynchronously
        if let Some(service) = AppState::global(cx).agent_config_service() {
            let service = service.clone();
            let weak_entity = cx.entity().downgrade();

            cx.spawn(async move |_entity, cx| {
                let agents = service.list_agents().await;
                let upload_dir = service.get_upload_dir().await;

                _ = cx.update(|cx| {
                    if let Some(entity) = weak_entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.agent_configs = agents.into_iter().collect();
                            this.upload_dir = upload_dir.to_string_lossy().to_string();
                            cx.notify();
                        });
                    }
                });
            })
            .detach();
        }
    }

    fn check_for_updates(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.update_status = UpdateStatus::Checking;
        cx.notify();

        let update_manager = self.update_manager.clone();
        let entity = cx.entity().downgrade();

        cx.spawn(async move |_this, cx| {
            let result = update_manager.check_for_updates().await;

            let _ = cx.update(|cx| {
                let _ = entity.update(cx, |this, cx| {
                    this.update_status = match result {
                        UpdateCheckResult::NoUpdate => UpdateStatus::NoUpdate,
                        UpdateCheckResult::UpdateAvailable(info) => UpdateStatus::Available {
                            version: info.version,
                            notes: info.release_notes,
                        },
                        UpdateCheckResult::Error(err) => UpdateStatus::Error(err),
                    };
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn setting_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<SettingPage> {
        let view = cx.entity();
        let default_settings = AppSettings::default();
        let resettable = AppSettings::global(cx).resettable;

        vec![
            SettingPage::new("General")
                .resettable(resettable)
                .default_open(true)
                .groups(vec![
                    SettingGroup::new().title("Appearance").items(vec![
                        SettingItem::new(
                            "Dark Mode",
                            SettingField::switch(
                                |cx: &App| cx.theme().mode.is_dark(),
                                |val: bool, cx: &mut App| {
                                    let mode = if val {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    };
                                    Theme::global_mut(cx).mode = mode;
                                    Theme::change(mode, None, cx);
                                },
                            )
                            .default_value(false),
                        )
                        .description("Switch between light and dark themes."),
                        SettingItem::new(
                            "Auto Switch Theme",
                            SettingField::checkbox(
                                |cx: &App| AppSettings::global(cx).auto_switch_theme,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).auto_switch_theme = val;
                                },
                            )
                            .default_value(default_settings.auto_switch_theme),
                        )
                        .description("Automatically switch theme based on system settings."),
                        SettingItem::new(
                            "resettable",
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).resettable,
                                |checked: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).resettable = checked
                                },
                            ),
                        )
                        .description("Enable/Disable reset button for settings."),
                        SettingItem::new(
                            "Group Variant",
                            SettingField::dropdown(
                                vec![
                                    (GroupBoxVariant::Normal.as_str().into(), "Normal".into()),
                                    (GroupBoxVariant::Outline.as_str().into(), "Outline".into()),
                                    (GroupBoxVariant::Fill.as_str().into(), "Fill".into()),
                                ],
                                {
                                    let view = view.clone();
                                    move |cx: &App| {
                                        SharedString::from(
                                            view.read(cx).group_variant.as_str().to_string(),
                                        )
                                    }
                                },
                                {
                                    let view = view.clone();
                                    move |val: SharedString, cx: &mut App| {
                                        view.update(cx, |view, cx| {
                                            view.group_variant =
                                                GroupBoxVariant::from_str(val.as_str());
                                            cx.notify();
                                        });
                                    }
                                },
                            )
                            .default_value(GroupBoxVariant::Outline.as_str().to_string()),
                        )
                        .description("Select the variant for setting groups."),
                        SettingItem::new(
                            "Group Size",
                            SettingField::dropdown(
                                vec![
                                    (Size::Medium.as_str().into(), "Medium".into()),
                                    (Size::Small.as_str().into(), "Small".into()),
                                    (Size::XSmall.as_str().into(), "XSmall".into()),
                                ],
                                {
                                    let view = view.clone();
                                    move |cx: &App| {
                                        SharedString::from(view.read(cx).size.as_str().to_string())
                                    }
                                },
                                {
                                    let view = view.clone();
                                    move |val: SharedString, cx: &mut App| {
                                        view.update(cx, |view, cx| {
                                            view.size = Size::from_str(val.as_str());
                                            cx.notify();
                                        });
                                    }
                                },
                            )
                            .default_value(Size::default().as_str().to_string()),
                        )
                        .description("Select the size for the setting group."),
                    ]),
                    SettingGroup::new()
                        .title("Font")
                        .item(
                            SettingItem::new(
                                "Font Family",
                                SettingField::dropdown(
                                    vec![
                                        ("Arial".into(), "Arial".into()),
                                        ("Helvetica".into(), "Helvetica".into()),
                                        ("Times New Roman".into(), "Times New Roman".into()),
                                        ("Courier New".into(), "Courier New".into()),
                                    ],
                                    |cx: &App| AppSettings::global(cx).font_family.clone(),
                                    |val: SharedString, cx: &mut App| {
                                        AppSettings::global_mut(cx).font_family = val;
                                    },
                                )
                                .default_value(default_settings.font_family),
                            )
                            .description("Select the font family for the story."),
                        )
                        .item(
                            SettingItem::new(
                                "Font Size",
                                SettingField::number_input(
                                    NumberFieldOptions {
                                        min: 8.0,
                                        max: 72.0,
                                        ..Default::default()
                                    },
                                    |cx: &App| AppSettings::global(cx).font_size,
                                    |val: f64, cx: &mut App| {
                                        AppSettings::global_mut(cx).font_size = val;
                                    },
                                )
                                .default_value(default_settings.font_size),
                            )
                            .description("Adjust the font size for better readability."),
                        )
                        .item(
                            SettingItem::new(
                                "Line Height",
                                SettingField::number_input(
                                    NumberFieldOptions {
                                        min: 8.0,
                                        max: 32.0,
                                        ..Default::default()
                                    },
                                    |cx: &App| AppSettings::global(cx).line_height,
                                    |val: f64, cx: &mut App| {
                                        AppSettings::global_mut(cx).line_height = val;
                                    },
                                )
                                .default_value(default_settings.line_height),
                            )
                            .description("Adjust the line height for better readability."),
                        ),
                    SettingGroup::new().title("Other").items(vec![
                        SettingItem::render(|options, _, _| {
                            h_flex()
                                .w_full()
                                .justify_between()
                                .flex_wrap()
                                .gap_3()
                                .child("This is a custom element item by use SettingItem::element.")
                                .child(
                                    Button::new("action")
                                        .icon(IconName::Globe)
                                        .label("Repository...")
                                        .outline()
                                        .with_size(options.size)
                                        .on_click(|_, _, cx| {
                                            cx.open_url(
                                                "https://github.com/sxhxliang/agent_studio",
                                            );
                                        }),
                                )
                                .into_any_element()
                        }),
                        SettingItem::new(
                            "CLI Path",
                            SettingField::input(
                                |cx: &App| AppSettings::global(cx).cli_path.clone(),
                                |val: SharedString, cx: &mut App| {
                                    println!("cli-path set value: {}", val);
                                    AppSettings::global_mut(cx).cli_path = val;
                                },
                            )
                            .default_value(default_settings.cli_path),
                        )
                        .layout(Axis::Vertical)
                        .description(
                            "Path to the CLI executable. \n\
                        This item uses Vertical layout. The title,\
                        description, and field are all aligned vertically with width 100%.",
                        ),
                    ]),
                ]),
            SettingPage::new("Software Update")
                .resettable(resettable)
                .groups(vec![
                    SettingGroup::new().title("Version").items(vec![
                        SettingItem::render({
                            let current_version = Version::current().to_string();
                            let update_status = self.update_status.clone();
                            move |_options, _window, cx| {
                                v_flex()
                                    .gap_2()
                                    .w_full()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Label::new("Current Version:").text_sm())
                                            .child(
                                                Label::new(&current_version)
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground),
                                            ),
                                    )
                                    .child(match &update_status {
                                        UpdateStatus::Idle => Label::new(
                                            "Click 'Check for Updates' to check for new versions",
                                        )
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .into_any_element(),
                                        UpdateStatus::Checking => h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::LoaderCircle).size_4())
                                            .child(
                                                Label::new("Checking for updates...")
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                            .into_any_element(),
                                        UpdateStatus::NoUpdate => h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::Check).size_4())
                                            .child(
                                                Label::new("You're up to date!")
                                                    .text_xs()
                                                    .text_color(cx.theme().success_foreground),
                                            )
                                            .into_any_element(),
                                        UpdateStatus::Available { version, notes } => {
                                            let has_notes = !notes.is_empty();
                                            let notes_elem = if has_notes {
                                                Some(
                                                    Label::new(notes)
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground),
                                                )
                                            } else {
                                                None
                                            };

                                            v_flex()
                                                .gap_2()
                                                .w_full()
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(
                                                            Icon::new(IconName::ArrowDown).size_4(),
                                                        )
                                                        .child(
                                                            Label::new(format!(
                                                                "Update available: v{}",
                                                                version
                                                            ))
                                                            .text_xs()
                                                            .text_color(
                                                                cx.theme().accent_foreground,
                                                            ),
                                                        ),
                                                )
                                                .children(notes_elem)
                                                .into_any_element()
                                        }
                                        UpdateStatus::Error(err) => h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::CircleX).size_4())
                                            .child(
                                                Label::new(format!("Error: {}", err))
                                                    .text_xs()
                                                    .text_color(
                                                        cx.theme().colors.danger_foreground,
                                                    ),
                                            )
                                            .into_any_element(),
                                    })
                                    .into_any()
                            }
                        }),
                        SettingItem::new(
                            "Check for Updates",
                            SettingField::render({
                                let view = view.clone();
                                move |options, _window, _cx| {
                                    Button::new("check-updates")
                                        .icon(IconName::LoaderCircle)
                                        .label("Check Now")
                                        .outline()
                                        .with_size(options.size)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.check_for_updates(window, cx);
                                                });
                                            }
                                        })
                                }
                            }),
                        )
                        .description("Manually check for available updates."),
                    ]),
                    SettingGroup::new().title("Update Settings").items(vec![
                        SettingItem::new(
                            "Auto Check on Startup",
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).auto_check_on_startup,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).auto_check_on_startup = val;
                                },
                            )
                            .default_value(default_settings.auto_check_on_startup),
                        )
                        .description(
                            "Automatically check for updates when the application starts.",
                        ),
                        SettingItem::new(
                            "Enable Notifications",
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).notifications_enabled,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).notifications_enabled = val;
                                },
                            )
                            .default_value(default_settings.notifications_enabled),
                        )
                        .description("Receive notifications about available updates."),
                        SettingItem::new(
                            "Auto Update",
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).auto_update,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).auto_update = val;
                                },
                            )
                            .default_value(default_settings.auto_update),
                        )
                        .description("Automatically download and install updates."),
                        SettingItem::new(
                            "Check Frequency (days)",
                            SettingField::number_input(
                                NumberFieldOptions {
                                    min: 1.0,
                                    max: 30.0,
                                    step: 1.0,
                                    ..Default::default()
                                },
                                |cx: &App| AppSettings::global(cx).check_frequency_days,
                                |val: f64, cx: &mut App| {
                                    AppSettings::global_mut(cx).check_frequency_days = val;
                                },
                            )
                            .default_value(default_settings.check_frequency_days),
                        )
                        .description("How often to automatically check for updates (in days)."),
                    ]),
                ]),
            // Agent Servers Page
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
                                        let config_path = view.read(cx).config_path.clone();
                                        let display = if config_path.is_empty() {
                                            "Not configured".to_string()
                                        } else {
                                            config_path
                                        };

                                        v_flex()
                                            .w_full()
                                            .gap_2()
                                            .child(
                                                gpui::div()
                                                    .w_full()
                                                    .overflow_x_hidden()
                                                    .child(
                                                        Label::new(display)
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
                                            .into_any()
                                    }
                                }),
                            )
                            .description("Path to agent configuration file (config.json)"),
                            SettingItem::new(
                                "Upload Directory",
                                SettingField::render({
                                    let view = view.clone();
                                    move |_options, _window, cx| {
                                        let upload_dir = view.read(cx).upload_dir.clone();
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
                                            .into_any()
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
                                let agent_configs = view.read(cx).agent_configs.clone();

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
                                        let name_clone = name.clone();
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

                                content.into_any()
                            }
                        })),
                ]),
            SettingPage::new("About")
                .resettable(resettable)
                .group(
                    SettingGroup::new().item(SettingItem::render(|_options, _, cx| {
                        v_flex()
                            .gap_3()
                            .w_full()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::GalleryVerticalEnd).size_16())
                            .child("Agent Studio")
                            .child(
                                Label::new(
                                    "Rust GUI components for building fantastic cross-platform \
                                    desktop application by using GPUI.",
                                )
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                            )
                            .into_any()
                    })),
                )
                .group(SettingGroup::new().title("Links").items(vec![
                        SettingItem::new(
                            "GitHub Repository",
                            SettingField::element(OpenURLSettingField::new(
                                "Repository...",
                                "https://github.com/sxhxliang/agent_studio",
                            )),
                        )
                        .description("Open the GitHub repository in your default browser."),
                        SettingItem::new(
                            "Documentation",
                            SettingField::element(OpenURLSettingField::new(
                                "Rust Docs...",
                                "https://docs.rs/gpui-component"
                            )),
                        )
                        .description(TextView::markdown(
                            "desc",
                            "Rust doc for the `gpui-component` crate.",
                            window,
                            cx,
                        )),
                        SettingItem::new(
                            "Website",
                            SettingField::render(|options, _window, _cx| {
                                Button::new("open-url")
                                    .outline()
                                    .label("Website...")
                                    .with_size(options.size)
                                    .on_click(|_, _window, cx| {
                                        cx.open_url("https://github.com/sxhxliang/agent_studio");
                                    })
                            }),
                        )
                        .description("Official website and documentation for the Agent Studio."),
                    ])),
        ]
    }
}

impl Focusable for SettingsPanel {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Settings::new("app-settings")
            .with_size(self.size)
            .with_group_variant(self.group_variant)
            .pages(self.setting_pages(window, cx))
    }
}
