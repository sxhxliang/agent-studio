//! Canonical on-disk locations — the single source of truth that replaces the
//! two duplicated `config_manager` modules in the legacy code.
//!
//! The repository and config-store types take their paths as constructor
//! arguments (so tests can point them at a temp directory); these helpers
//! compute the defaults the composition root uses in production.

use std::path::{Path, PathBuf};

/// The application's per-user data directory.
///
/// - Windows: `%APPDATA%\agentx`
/// - Linux: `~/.config/agentx`
/// - macOS: `~/.agentx`
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().unwrap_or_default().join(".agentx")
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs::config_dir().unwrap_or_default().join("agentx")
    }
}

/// Path to the JSON configuration file within `data_dir`.
pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

/// Directory holding per-session `.jsonl` history files within `data_dir`.
pub fn sessions_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("sessions")
}
