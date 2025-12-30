use gpui::{App, Global, SharedString};
use gpui_component::{
    Sizable,
    button::Button,
    setting::{RenderOptions, SettingFieldElement},
};

pub struct AppSettings {
    pub auto_switch_theme: bool,
    pub cli_path: SharedString,
    pub font_family: SharedString,
    pub font_size: f64,
    pub line_height: f64,
    pub notifications_enabled: bool,
    pub auto_update: bool,
    pub auto_check_on_startup: bool,
    pub check_frequency_days: f64,
    pub resettable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
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
    pub fn global(cx: &App) -> &AppSettings {
        cx.global::<AppSettings>()
    }

    pub fn global_mut(cx: &mut App) -> &mut AppSettings {
        cx.global_mut::<AppSettings>()
    }
}

pub struct OpenURLSettingField {
    pub label: SharedString,
    pub url: SharedString,
}

impl OpenURLSettingField {
    pub fn new(label: impl Into<SharedString>, url: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
        }
    }
}

impl SettingFieldElement for OpenURLSettingField {
    type Element = Button;
    fn render_field(
        &self,
        options: &RenderOptions,
        _: &mut gpui::Window,
        _: &mut App,
    ) -> Self::Element {
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
