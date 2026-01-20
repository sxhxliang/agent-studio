use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Get the user data directory for AgentX
/// - macOS: ~/.agentx/
/// - Windows: %APPDATA%\agentx\
/// - Linux: ~/.config/agentx/
pub fn get_user_data_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
        Ok(home.join(".agentx"))
    }

    #[cfg(target_os = "windows")]
    {
        let appdata =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Failed to get AppData directory"))?;
        Ok(appdata.join("agentx"))
    }

    #[cfg(target_os = "linux")]
    {
        let config =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;
        Ok(config.join("agentx"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("Unsupported platform"))
    }
}

/// Ensure the user data directory exists
pub fn ensure_user_data_dir() -> Result<PathBuf> {
    let dir = get_user_data_dir()?;
    if !dir.exists() {
        log::info!("Creating user data directory: {:?}", dir);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create directory: {:?}", dir))?;
    }
    Ok(dir)
}

fn fallback_data_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("agentx");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("Failed to create fallback data directory {:?}: {}", dir, e);
    }
    dir
}

pub fn user_data_dir_or_temp() -> PathBuf {
    match ensure_user_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!("Failed to resolve user data directory: {}", e);
            fallback_data_dir()
        }
    }
}

pub fn ensure_default_config_at(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
    }
    let default_config = crate::assets::get_default_config()
        .ok_or_else(|| anyhow::anyhow!("Failed to get embedded default config"))?;
    std::fs::write(path, default_config)
        .with_context(|| format!("Failed to write config file: {:?}", path))?;
    Ok(())
}

/// Get the config file path in the user data directory
pub fn get_user_config_path() -> Result<PathBuf> {
    Ok(get_user_data_dir()?.join("config.json"))
}

pub fn get_user_config_path_or_temp() -> PathBuf {
    user_data_dir_or_temp().join("config.json")
}

/// Initialize user config directory and config file
/// If config file doesn't exist, create it from the embedded default config
pub fn initialize_user_config() -> Result<PathBuf> {
    let user_data_dir = ensure_user_data_dir()?;
    let config_path = user_data_dir.join("config.json");

    if !config_path.exists() {
        log::info!(
            "Config file not found, creating from embedded default: {:?}",
            config_path
        );
        ensure_default_config_at(&config_path)?;
        log::info!("Created default config file at: {:?}", config_path);
    } else {
        log::info!("Using existing config file: {:?}", config_path);
    }

    Ok(config_path)
}

/// Load config from user data directory
/// Falls back to embedded default if file doesn't exist or is invalid
pub fn load_user_config() -> Result<crate::core::config::Config> {
    let config_path = initialize_user_config()?;

    let config_content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

    let config: crate::core::config::Config = serde_json::from_str(&config_content)
        .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;

    Ok(config)
}

/// Get the themes directory path in the user data directory
pub fn get_themes_dir() -> Result<PathBuf> {
    Ok(user_data_dir_or_temp().join("themes"))
}

/// Initialize themes directory and theme files
/// If themes directory doesn't exist, create it and populate with embedded themes
pub fn initialize_themes_dir() -> Result<PathBuf> {
    let themes_dir = get_themes_dir()?;

    // Create themes directory if it doesn't exist
    if !themes_dir.exists() {
        log::info!("Creating themes directory: {:?}", themes_dir);
        std::fs::create_dir_all(&themes_dir)
            .with_context(|| format!("Failed to create themes directory: {:?}", themes_dir))?;
    }

    // Get all embedded theme files
    let embedded_themes = crate::assets::get_embedded_themes();

    if embedded_themes.is_empty() {
        log::warn!("No embedded themes found");
        return Ok(themes_dir);
    }

    // Write each theme file if it doesn't exist
    for (filename, content) in embedded_themes {
        let theme_path = themes_dir.join(&filename);

        if !theme_path.exists() {
            log::info!("Creating theme file: {:?}", theme_path);
            std::fs::write(&theme_path, content)
                .with_context(|| format!("Failed to write theme file: {:?}", theme_path))?;
        }
    }

    log::info!("Themes directory initialized: {:?}", themes_dir);
    Ok(themes_dir)
}

/// Get state file path
/// Always uses user data directory: <user_data_dir>/state.json
pub fn get_state_file_path() -> PathBuf {
    user_data_dir_or_temp().join("state.json")
}

/// Get workspace config file path
/// Always uses user data directory: <user_data_dir>/workspace-config.json
pub fn get_workspace_config_path() -> PathBuf {
    user_data_dir_or_temp().join("workspace-config.json")
}

/// Get docks layout file path
/// Always uses user data directory: <user_data_dir>/docks-layout.json
pub fn get_docks_layout_path() -> PathBuf {
    user_data_dir_or_temp().join("docks-layout.json")
}

/// Get sessions directory path
/// Always uses user data directory: <user_data_dir>/sessions
pub fn get_sessions_dir() -> PathBuf {
    user_data_dir_or_temp().join("sessions")
}
