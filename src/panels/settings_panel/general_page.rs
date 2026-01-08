use gpui::{App, Axis, Entity, ParentElement as _, SharedString, Styled};
use gpui_component::{
    ActiveTheme, IconName, Sizable, Size, Theme, ThemeMode,
    button::Button,
    group_box::GroupBoxVariant,
    h_flex,
    setting::{NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage},
};

use super::panel::SettingsPanel;
use super::types::AppSettings;

impl SettingsPanel {
    pub fn general_page(&self, view: &Entity<Self>, resettable: bool) -> SettingPage {
        let default_settings = AppSettings::default();

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
                            |cx: &App| AppSettings::global(cx).group_variant.clone(),
                            |val: SharedString, cx: &mut App| {
                                AppSettings::global_mut(cx).group_variant = val;
                            },
                        )
                        .default_value(default_settings.group_variant),
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
                            |cx: &App| AppSettings::global(cx).size.clone(),
                            |val: SharedString, cx: &mut App| {
                                AppSettings::global_mut(cx).size = val;
                            },
                        )
                        .default_value(default_settings.size),
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
                                        cx.open_url("https://github.com/sxhxliang/agent_studio");
                                    }),
                            )
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
            ])
    }
}
