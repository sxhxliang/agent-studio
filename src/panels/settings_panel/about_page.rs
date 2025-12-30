use gpui::{ParentElement as _, Styled};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    label::Label,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage},
    text::TextView,
    v_flex,
};

use super::types::OpenURLSettingField;

pub fn about_page(resettable: bool) -> SettingPage {
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
                )),
                SettingItem::new(
                    "Website",
                    SettingField::render(|options, _window, _cx| {
                        gpui_component::button::Button::new("open-url")
                            .outline()
                            .label("Website...")
                            .with_size(options.size)
                            .on_click(|_, _window, cx| {
                                cx.open_url("https://github.com/sxhxliang/agent_studio");
                            })
                    }),
                )
                .description("Official website and documentation for the Agent Studio."),
            ]))
}
