use gpui::{
    App, AppContext, Axis, Context, Element, Entity, FocusHandle, Focusable, Global, IntoElement,
    ParentElement as _, Render, SharedString, Styled, Window, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, Size, Theme, ThemeMode,
    button::Button,
    group_box::GroupBoxVariant,
    h_flex,
    label::Label,
    setting::{
        NumberFieldOptions, RenderOptions, SettingField, SettingFieldElement, SettingGroup,
        SettingItem, SettingPage, Settings,
    },
    text::TextView,
    v_flex,
};

use crate::core::updater::{UpdateCheckResult, UpdateManager, Version};

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

    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.set_global::<AppSettings>(AppSettings::default());

        Self {
            focus_handle: cx.focus_handle(),
            group_variant: GroupBoxVariant::Outline,
            size: Size::default(),
            update_status: UpdateStatus::Idle,
            update_manager: UpdateManager::default(),
        }
    }

    fn check_for_updates(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.update_status = UpdateStatus::Checking;
        cx.notify();

        let update_manager = self.update_manager.clone();
        let entity = cx.entity().downgrade();

        cx.spawn(async move |_this, mut cx| {
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
                                                    .text_color(cx.theme().muted_foreground)
                                            )
                                    )
                                    .child(
                                        match &update_status {
                                            UpdateStatus::Idle => {
                                                Label::new("Click 'Check for Updates' to check for new versions")
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .into_any_element()
                                            }
                                            UpdateStatus::Checking => {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(Icon::new(IconName::LoaderCircle).size_4())
                                                    .child(
                                                        Label::new("Checking for updates...")
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                    )
                                                    .into_any_element()
                                            }
                                            UpdateStatus::NoUpdate => {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(Icon::new(IconName::Check).size_4())
                                                    .child(
                                                        Label::new("You're up to date!")
                                                            .text_xs()
                                                            .text_color(cx.theme().success_foreground)
                                                    )
                                                    .into_any_element()
                                            }
                                            UpdateStatus::Available { version, notes } => {
                                                let has_notes = !notes.is_empty();
                                                let notes_elem = if has_notes {
                                                    Some(
                                                        Label::new(notes)
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
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
                                                            .child(Icon::new(IconName::ArrowDown).size_4())
                                                            .child(
                                                                Label::new(format!("Update available: v{}", version))
                                                                    .text_xs()
                                                                    .text_color(cx.theme().accent_foreground)
                                                            )
                                                    )
                                                    .children(notes_elem)
                                                    .into_any_element()
                                            }
                                            UpdateStatus::Error(err) => {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(Icon::new(IconName::CircleX).size_4())
                                                    .child(
                                                        Label::new(format!("Error: {}", err))
                                                            .text_xs()
                                                            .text_color(cx.theme().colors.danger_foreground)
                                                    )
                                                    .into_any_element()
                                            }
                                        }
                                    )
                                    .into_any()
                            }
                        }),
                        SettingItem::new(
                            "Check for Updates",
                            SettingField::render({
                                let view = view.clone();
                                move |options, window, _cx| {
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
                        .description("Automatically check for updates when the application starts."),
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
                    ])
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
