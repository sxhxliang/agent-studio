use std::path::PathBuf;

use gpui::{App, SharedString, px};
use gpui_component::{ActiveTheme, Theme, ThemeRegistry, scroll::ScrollbarShow};
use serde::{Deserialize, Serialize};

use crate::app::actions::{SwitchTheme, SwitchThemeMode};
use crate::panels::AppSettings;

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
    // Get state file path based on build mode
    let state_file = crate::core::config_manager::get_state_file_path();

    // Load last theme state and app settings
    let json = std::fs::read_to_string(&state_file).unwrap_or(String::default());
    tracing::info!("Load themes and app settings from: {:?}", state_file);
    let state = serde_json::from_str::<State>(&json).unwrap_or_default();

    // Initialize AppSettings globally (before it was only initialized in SettingsPanel::new)
    let app_settings = state.app_settings.unwrap_or_else(AppSettings::default);
    tracing::info!(
        "Loaded app_settings with font_size: {}",
        app_settings.font_size
    );
    cx.set_global::<AppSettings>(app_settings.clone());

    // Get themes directory based on build mode
    let themes_dir = if cfg!(debug_assertions) {
        // Debug mode: use local ./themes for development
        let dir = PathBuf::from("./themes");
        tracing::info!("Debug mode: using local themes directory: {:?}", dir);
        dir
    } else {
        // Release mode: use user data directory, fallback to ./themes
        match crate::core::config_manager::initialize_themes_dir() {
            Ok(dir) => {
                tracing::info!(
                    "Release mode: using themes from user data directory: {:?}",
                    dir
                );
                dir
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize user themes directory: {}, falling back to ./themes",
                    e
                );
                PathBuf::from("./themes")
            }
        }
    };

    if let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&state.theme)
            .cloned()
        {
            Theme::global_mut(cx).apply_config(&theme);

            // Re-sync font_size from AppSettings after applying theme config
            // to ensure user settings take precedence over theme defaults
            let font_size = AppSettings::global(cx).font_size;
            tracing::info!(
                "Re-syncing font_size from AppSettings after theme load: {}",
                font_size
            );
            Theme::global_mut(cx).font_size = px(font_size as f32);
            cx.refresh_windows();
        }
    }) {
        tracing::error!("Failed to watch themes directory: {}", err);
    }

    if let Some(scrollbar_show) = state.scrollbar_show {
        Theme::global_mut(cx).scrollbar_show = scrollbar_show;
    }

    // Sync font_size from AppSettings to Theme
    tracing::info!(
        "Initial font_size sync from AppSettings: {} -> Theme",
        app_settings.font_size
    );
    Theme::global_mut(cx).font_size = px(app_settings.font_size as f32);

    cx.refresh_windows();

    // Save initial state to ensure all fields are persisted
    save_state(cx);

    // Save state when theme changes
    cx.observe_global::<Theme>(|cx| {
        save_state(cx);
    })
    .detach();

    // Save state when app settings change, and sync font_size to Theme
    cx.observe_global::<AppSettings>(|cx| {
        // Auto-sync font_size from AppSettings to Theme
        let font_size = AppSettings::global(cx).font_size;
        tracing::info!(
            "AppSettings changed, syncing font_size: {} -> Theme",
            font_size
        );
        Theme::global_mut(cx).font_size = px(font_size as f32);

        save_state(cx);
    })
    .detach();

    cx.on_action(|switch: &SwitchTheme, cx| {
        let theme_name = switch.0.clone();
        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme_config);

            // Re-sync font_size from AppSettings after applying theme config
            // to ensure user settings take precedence over theme defaults
            let font_size = AppSettings::global(cx).font_size;
            tracing::info!(
                "Re-syncing font_size from AppSettings after theme switch: {}",
                font_size
            );
            Theme::global_mut(cx).font_size = px(font_size as f32);
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
pub(crate) fn save_state(cx: &mut App) {
    let state = State {
        theme: cx.theme().theme_name().clone(),
        scrollbar_show: Some(cx.theme().scrollbar_show),
        app_settings: Some(AppSettings::global(cx).clone()),
    };

    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let state_file = crate::core::config_manager::get_state_file_path();
        println!("Save layout...");
        // Ignore write errors - if state file doesn't exist or can't be written, do nothing
        let _ = std::fs::write(state_file, json);
    }
}
