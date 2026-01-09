use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the user data directory for AgentX
/// - macOS: ~/.agentx/
/// - Windows: %APPDATA%\agentx\
/// - Linux: ~/.config/agentx/
pub fn get_user_data_dir() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?;
        Ok(home.join(".agentx"))
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get AppData directory"))?;
        Ok(appdata.join("agentx"))
    }

    #[cfg(target_os = "linux")]
    {
        let config = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?;
        Ok(config.join("agentx"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(anyhow::anyhow!("Unsupported platform"))
    }
}

/// Get the config file path in the user data directory
pub fn get_user_config_path() -> Result<PathBuf> {
    Ok(get_user_data_dir()?.join("config.json"))
}

/// Initialize user config directory and config file
/// If config file doesn't exist, create it from the embedded default config
pub fn initialize_user_config() -> Result<PathBuf> {
    let user_data_dir = get_user_data_dir()?;
    let config_path = get_user_config_path()?;

    // Create user data directory if it doesn't exist
    if !user_data_dir.exists() {
        log::info!("Creating user data directory: {:?}", user_data_dir);
        std::fs::create_dir_all(&user_data_dir)
            .with_context(|| format!("Failed to create directory: {:?}", user_data_dir))?;
    }

    // If config file doesn't exist, create it from embedded default
    if !config_path.exists() {
        log::info!("Config file not found, creating from embedded default: {:?}", config_path);

        let default_config = crate::assets::get_default_config()
            .ok_or_else(|| anyhow::anyhow!("Failed to get embedded default config"))?;

        std::fs::write(&config_path, default_config)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

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
    Ok(get_user_data_dir()?.join("themes"))
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

/// Get state file path based on build mode
/// - Debug mode: target/state.json
/// - Release mode: <user_data_dir>/state.json
pub fn get_state_file_path() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("target/state.json")
    } else {
        get_user_data_dir()
            .map(|dir| dir.join("state.json"))
            .unwrap_or_else(|_| PathBuf::from("state.json"))
    }
}

/// Get workspace config file path based on build mode
/// - Debug mode: target/workspace-config.json
/// - Release mode: <user_data_dir>/workspace-config.json
pub fn get_workspace_config_path() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("target/workspace-config.json")
    } else {
        get_user_data_dir()
            .map(|dir| dir.join("workspace-config.json"))
            .unwrap_or_else(|_| PathBuf::from("workspace-config.json"))
    }
}

/// Get docks layout file path based on build mode
/// - Debug mode: target/docks-agentx.json
/// - Release mode: <user_data_dir>/docks-agentx.json
pub fn get_docks_layout_path() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("target/docks-agentx.json")
    } else {
        get_user_data_dir()
            .map(|dir| dir.join("docks-agentx.json"))
            .unwrap_or_else(|_| PathBuf::from("docks-agentx.json"))
    }
}

/// Get sessions directory path based on build mode
/// - Debug mode: target/sessions
/// - Release mode: <user_data_dir>/sessions
pub fn get_sessions_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from("target/sessions")
    } else {
        get_user_data_dir()
            .map(|dir| dir.join("sessions"))
            .unwrap_or_else(|_| PathBuf::from("sessions"))
    }
}
