use gpui::{App, Context, Entity, ParentElement as _, Styled, Window};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::Button,
    h_flex,
    label::Label,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage},
    v_flex,
};
use rust_i18n::t;

use super::panel::SettingsPanel;
use super::types::{AppSettings, UpdateStatus};
use crate::core::updater::{UpdateCheckResult, Version};

fn os_display_name() -> String {
    match std::env::consts::OS {
        "macos" => "macOS".into(),
        "windows" => "Windows".into(),
        "linux" => "Linux".into(),
        other => other.into(),
    }
}

fn arch_display_name() -> String {
    match std::env::consts::ARCH {
        "x86_64" => "x64".into(),
        "aarch64" => "ARM64".into(),
        other => other.into(),
    }
}

fn os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Unknown".into())
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| {
                s.find('[')
                    .and_then(|start| s.find(']').map(|end| (start, end)))
                    .map(|(s_i, e_i)| s[s_i + 1..e_i].replace("Version ", "").trim().to_string())
                    .unwrap_or_else(|| s.trim().to_string())
            })
            .unwrap_or_else(|| "Unknown".into())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|c| {
                c.lines().find(|l| l.starts_with("VERSION_ID=")).map(|l| {
                    l.trim_start_matches("VERSION_ID=")
                        .trim_matches('"')
                        .to_string()
                })
            })
            .unwrap_or_else(|| "Unknown".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "Unknown".into()
    }
}

impl SettingsPanel {
    pub fn update_page(&self, view: &Entity<Self>, resettable: bool) -> SettingPage {
        let default_settings = AppSettings::default();

        SettingPage::new(t!("settings.update.title").to_string())
            .resettable(resettable)
            .groups(vec![
                SettingGroup::new()
                    .title(t!("settings.update.group.version").to_string())
                    .items(vec![
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
                                            .child(
                                                Label::new(
                                                    t!("settings.update.current_version.label")
                                                        .to_string(),
                                                )
                                                .text_sm(),
                                            )
                                            .child(
                                                Label::new(&current_version)
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground),
                                            ),
                                    )
                                    .child(match &update_status {
                                        UpdateStatus::Idle | UpdateStatus::NoUpdate => h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::Check).size_4())
                                            .child(
                                                Label::new(
                                                    t!("settings.update.status.up_to_date")
                                                        .to_string(),
                                                )
                                                .text_xs()
                                                .text_color(cx.theme().success_foreground),
                                            ),
                                        UpdateStatus::Checking => h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::LoaderCircle).size_4())
                                            .child(
                                                Label::new(
                                                    t!("settings.update.status.checking")
                                                        .to_string(),
                                                )
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground),
                                            ),
                                        UpdateStatus::Available { version, notes } => {
                                            let notes_elem = if notes.is_empty() {
                                                None
                                            } else {
                                                Some(
                                                    Label::new(notes)
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground),
                                                )
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
                                                        Label::new(
                                                            t!(
                                                                "settings.update.status.available",
                                                                version = version
                                                            )
                                                            .to_string(),
                                                        )
                                                        .text_xs()
                                                        .text_color(cx.theme().accent_foreground),
                                                    ),
                                            )
                                            .children(notes_elem)
                                        }
                                        UpdateStatus::Error(err) => h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Icon::new(IconName::CircleX).size_4())
                                            .child(
                                                Label::new(
                                                    t!("settings.update.status.error", error = err)
                                                        .to_string(),
                                                )
                                                .text_xs()
                                                .text_color(cx.theme().colors.danger_foreground),
                                            ),
                                    })
                            }
                        }),
                        SettingItem::new(
                            t!("settings.update.check.label").to_string(),
                            SettingField::render({
                                let view = view.clone();
                                move |options, _window, _cx| {
                                    Button::new("check-updates")
                                        .icon(IconName::LoaderCircle)
                                        .label(t!("settings.update.check.button").to_string())
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
                        .description(t!("settings.update.check.description").to_string()),
                    ]),
                // System Information
                SettingGroup::new()
                    .title(t!("settings.update.group.system").to_string())
                    .items(vec![SettingItem::render({
                        let os = format!("{} {}", os_display_name(), os_version());
                        let arch = arch_display_name();
                        move |_options, _window, cx| {
                            v_flex()
                                .gap_2()
                                .w_full()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::Settings).size_4())
                                        .child(
                                            Label::new(t!("settings.update.system.os").to_string())
                                                .text_sm(),
                                        )
                                        .child(
                                            Label::new(&os)
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::Folder).size_4())
                                        .child(
                                            Label::new(
                                                t!("settings.update.system.arch").to_string(),
                                            )
                                            .text_sm(),
                                        )
                                        .child(
                                            Label::new(&arch)
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                )
                        }
                    })]),
                // Update Settings
                SettingGroup::new()
                    .title(t!("settings.update.group.settings").to_string())
                    .items(vec![
                        SettingItem::new(
                            t!("settings.update.auto_check.label").to_string(),
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).auto_check_on_startup,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).auto_check_on_startup = val;
                                },
                            )
                            .default_value(default_settings.auto_check_on_startup),
                        )
                        .description(t!("settings.update.auto_check.description").to_string()),
                        SettingItem::new(
                            t!("settings.update.notifications.label").to_string(),
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).notifications_enabled,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).notifications_enabled = val;
                                },
                            )
                            .default_value(default_settings.notifications_enabled),
                        )
                        .description(t!("settings.update.notifications.description").to_string()),
                        SettingItem::new(
                            t!("settings.update.auto_update.label").to_string(),
                            SettingField::switch(
                                |cx: &App| AppSettings::global(cx).auto_update,
                                |val: bool, cx: &mut App| {
                                    AppSettings::global_mut(cx).auto_update = val;
                                },
                            )
                            .default_value(default_settings.auto_update),
                        )
                        .description(t!("settings.update.auto_update.description").to_string()),
                        SettingItem::new(
                            t!("settings.update.frequency.label").to_string(),
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
                        .description(t!("settings.update.frequency.description").to_string()),
                    ]),
            ])
    }

    pub fn check_for_updates(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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
}
