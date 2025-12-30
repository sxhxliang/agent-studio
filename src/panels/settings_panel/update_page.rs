use gpui::{App, Context, Entity, ParentElement as _, Styled, Window};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::Button,
    h_flex,
    label::Label,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage},
    v_flex,
};

use super::panel::SettingsPanel;
use super::types::{AppSettings, UpdateStatus};
use crate::core::updater::{UpdateCheckResult, Version};

impl SettingsPanel {
    pub fn update_page(&self, view: &Entity<Self>, resettable: bool) -> SettingPage {
        let default_settings = AppSettings::default();

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
                                    UpdateStatus::Idle => h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::Check).size_4())
                                        .child(
                                            Label::new("You're up to date!")
                                                .text_xs()
                                                .text_color(cx.theme().success_foreground),
                                        ),
                                    UpdateStatus::Checking => h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::LoaderCircle).size_4())
                                        .child(
                                            Label::new("Checking for updates...")
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                    UpdateStatus::NoUpdate => h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Icon::new(IconName::Check).size_4())
                                        .child(
                                            Label::new("You're up to date!")
                                                .text_xs()
                                                .text_color(cx.theme().success_foreground),
                                        ),
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
                                                    .child(Icon::new(IconName::ArrowDown).size_4())
                                                    .child(
                                                        Label::new(format!(
                                                            "Update available: v{}",
                                                            version
                                                        ))
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
                                            Label::new(format!("Error: {}", err))
                                                .text_xs()
                                                .text_color(cx.theme().colors.danger_foreground),
                                        ),
                                })
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
