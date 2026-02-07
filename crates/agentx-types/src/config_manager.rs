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

pub fn ensure_default_config_at(path: &Path, default_config_content: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
    }
    std::fs::write(path, default_config_content)
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

/// Get the themes directory path in the user data directory
pub fn get_themes_dir() -> Result<PathBuf> {
    Ok(user_data_dir_or_temp().join("themes"))
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
