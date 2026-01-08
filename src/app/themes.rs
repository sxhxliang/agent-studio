use std::path::PathBuf;

use gpui::{App, SharedString};
use gpui_component::{ActiveTheme, Theme, ThemeRegistry, scroll::ScrollbarShow};
use serde::{Deserialize, Serialize};

use crate::app::actions::{SwitchTheme, SwitchThemeMode};
use crate::panels::AppSettings;

const STATE_FILE: &str = "target/state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    theme: SharedString,
    scrollbar_show: Option<ScrollbarShow>,
    #[serde(default)]
    app_settings: Option<AppSettings>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            theme: "Default Light".into(),
            scrollbar_show: None,
            app_settings: None,
        }
    }
}

pub fn init(cx: &mut App) {
    // Load last theme state and app settings
    let json = std::fs::read_to_string(STATE_FILE).unwrap_or(String::default());
    tracing::info!("Load themes and app settings...");
    let state = serde_json::from_str::<State>(&json).unwrap_or_default();

    // Initialize AppSettings globally (before it was only initialized in SettingsPanel::new)
    let app_settings = state.app_settings.unwrap_or_else(AppSettings::default);
    cx.set_global::<AppSettings>(app_settings);

    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&state.theme)
            .cloned()
        {
            Theme::global_mut(cx).apply_config(&theme);
        }
    }) {
        tracing::error!("Failed to watch themes directory: {}", err);
    }

    if let Some(scrollbar_show) = state.scrollbar_show {
        Theme::global_mut(cx).scrollbar_show = scrollbar_show;
    }
    cx.refresh_windows();

    // Save initial state to ensure all fields are persisted
    save_state(cx);

    // Save state when theme changes
    cx.observe_global::<Theme>(|cx| {
        save_state(cx);
    })
    .detach();

    // Save state when app settings change
    cx.observe_global::<AppSettings>(|cx| {
        save_state(cx);
    })
    .detach();

    cx.on_action(|switch: &SwitchTheme, cx| {
        let theme_name = switch.0.clone();
        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme_config);
        }
        cx.refresh_windows();
    });
    cx.on_action(|switch: &SwitchThemeMode, cx| {
        let mode = switch.0;
        Theme::change(mode, None, cx);
        cx.refresh_windows();
    });
}

/// Helper function to save current state to file
fn save_state(cx: &mut App) {
    let state = State {
        theme: cx.theme().theme_name().clone(),
        scrollbar_show: Some(cx.theme().scrollbar_show),
        app_settings: Some(AppSettings::global(cx).clone()),
    };

    if let Ok(json) = serde_json::to_string_pretty(&state) {
        // Ignore write errors - if STATE_FILE doesn't exist or can't be written, do nothing
        let _ = std::fs::write(STATE_FILE, json);
    }
}
