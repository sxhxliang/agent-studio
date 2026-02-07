use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::process::Command;

use super::{NodeJsDetectionMode, error};

/// Create a Command with console window hidden on Windows
fn new_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let cmd = Command::new(program);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = cmd;
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }

    #[cfg(not(windows))]
    cmd
}

/// Timeout for each subprocess command (e.g., `which node`, `node --version`)
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_NVM_SEARCH_DEPTH: usize = 6;

/// Detect Node.js installation on Windows
#[cfg(target_os = "windows")]
pub async fn detect_system_nodejs(mode: NodeJsDetectionMode) -> Option<PathBuf> {
    // Try 'where node.exe' first (most reliable)
    if let Some(path) = try_which_command("node.exe").await {
        return Some(path);
    }

    // Try 'where node' as fallback
    if let Some(path) = try_which_command("node").await {
        return Some(path);
    }

    // Check standard installation directories
    let standard_paths = vec![
        "C:\\Program Files\\nodejs\\node.exe",
        "C:\\Program Files (x86)\\nodejs\\node.exe",
    ];

    for path_str in standard_paths {
        let path = PathBuf::from(path_str);
        if path.exists() {
            log::debug!("Found Node.js at standard location: {}", path.display());
            return Some(path);
        }
    }

    // Check NVM for Windows
    if let Some(path) = check_nvm_windows(mode).await {
        return Some(path);
    }

    None
}

/// Detect Node.js installation on Unix-like systems (macOS, Linux)
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub async fn detect_system_nodejs(mode: NodeJsDetectionMode) -> Option<PathBuf> {
    // Try 'which node' first
    if let Some(path) = try_which_command("node").await {
        return Some(path);
    }

    // Check standard installation directories
    for path in unix_standard_node_paths() {
        if path.exists() {
            log::debug!("Found Node.js at standard location: {}", path.display());
            return Some(path);
        }
    }

    // Check NVM for Unix
    if let Some(path) = check_nvm_unix().await {
        return Some(path);
    }

    // On macOS, GUI apps don't inherit the user's shell PATH.
    // Only try the login shell in full mode to avoid slow shell startup.
    if mode == NodeJsDetectionMode::Full {
        if let Some(path) = try_which_from_login_shell("node").await {
            return Some(path);
        }
    }

    None
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_standard_node_paths() -> Vec<PathBuf> {
    let paths = vec![
        PathBuf::from("/usr/local/bin/node"),
        PathBuf::from("/usr/bin/node"),
        PathBuf::from("/opt/homebrew/bin/node"), // macOS Apple Silicon
        PathBuf::from("/opt/node/bin/node"),     // Custom installations
        PathBuf::from("/opt/local/bin/node"),    // MacPorts
        PathBuf::from("/snap/bin/node"),         // Snap
    ];

    paths
}

/// Try to find a command by sourcing the user's login shell PATH.
/// macOS GUI apps don't inherit PATH from .zshrc/.bashrc, so this
/// runs a login shell to get the real PATH and searches for the command.
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn try_which_from_login_shell(command: &str) -> Option<PathBuf> {
    // Determine user's shell
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    // Run: $SHELL -l -c 'which node'
    let result = tokio::time::timeout(
        COMMAND_TIMEOUT,
        Command::new(&shell)
            .args(["-l", "-c", &format!("which {}", command)])
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let path = PathBuf::from(path_str.trim());
            if path.exists() {
                log::debug!(
                    "Found {} via login shell ({}): {}",
                    command,
                    shell,
                    path.display()
                );
                return Some(path);
            }
        }
        Ok(Ok(_)) => {
            log::debug!("Login shell did not find {}", command);
        }
        Ok(Err(e)) => {
            log::debug!("Failed to run login shell: {}", e);
        }
        Err(_) => {
            log::warn!(
                "Login shell timed out after {:?} searching for {}",
                COMMAND_TIMEOUT,
                command
            );
        }
    }

    None
}

/// Try to find command using 'where' (Windows) or 'which' (Unix)
async fn try_which_command(command: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let which_cmd = "where";
    #[cfg(not(target_os = "windows"))]
    let which_cmd = "which";

    match tokio::time::timeout(
        COMMAND_TIMEOUT,
        new_command(which_cmd).arg(command).output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let path_str = path_str.trim();

            // 'where' on Windows can return multiple paths, take the first one
            let first_path = path_str.lines().next()?;

            let path = PathBuf::from(first_path);
            if path.exists() {
                log::debug!("Found Node.js via {}: {}", which_cmd, path.display());
                return Some(path);
            }
        }
        Ok(Ok(_)) => {
            log::debug!("{} command did not find {}", which_cmd, command);
        }
        Ok(Err(e)) => {
            log::debug!("Failed to run {} command: {}", which_cmd, e);
        }
        Err(_) => {
            log::warn!(
                "{} command timed out after {:?} for {}",
                which_cmd,
                COMMAND_TIMEOUT,
                command
            );
        }
    }

    None
}

/// Check NVM (Node Version Manager) on Windows
#[cfg(target_os = "windows")]
async fn check_nvm_windows(mode: NodeJsDetectionMode) -> Option<PathBuf> {
    // NVM for Windows typically installs to %APPDATA%\nvm
    let appdata = std::env::var("APPDATA").ok()?;
    let nvm_dir = PathBuf::from(appdata).join("nvm");

    if !nvm_dir.exists() {
        return None;
    }

    log::debug!("Checking NVM directory: {}", nvm_dir.display());

    // Try to find the current version from settings.txt
    let settings_file = nvm_dir.join("settings.txt");
    if let Ok(settings) = fs::read_to_string(&settings_file).await {
        for line in settings.lines() {
            if line.starts_with("root:") {
                let root = line.trim_start_matches("root:").trim();
                let node_exe = PathBuf::from(root).join("node.exe");
                if node_exe.exists() {
                    log::debug!("Found Node.js via NVM: {}", node_exe.display());
                    return Some(node_exe);
                }
            }
        }
    }

    // Fallback: scan for any node.exe in nvm directory (full mode only)
    if mode == NodeJsDetectionMode::Full {
        return search_for_node_in_directory(&nvm_dir);
    }

    None
}

/// Check NVM (Node Version Manager) on Unix-like systems
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn check_nvm_unix() -> Option<PathBuf> {
    // NVM typically installs to ~/.nvm
    let home = std::env::var("HOME").ok()?;
    let nvm_dir = PathBuf::from(home)
        .join(".nvm")
        .join("versions")
        .join("node");

    if !nvm_dir.exists() {
        return None;
    }

    log::debug!("Checking NVM directory: {}", nvm_dir.display());

    // Find the latest or default version
    let mut versions = vec![];

    if let Ok(mut entries) = fs::read_dir(&nvm_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().is_dir() {
                versions.push(entry.path());
            }
        }
    }

    // Sort versions (simple lexicographic sort, good enough for most cases)
    versions.sort();

    // Try the latest version first
    for version_dir in versions.iter().rev() {
        let node_path = version_dir.join("bin").join("node");
        if node_path.exists() {
            log::debug!("Found Node.js via NVM: {}", node_path.display());
            return Some(node_path);
        }
    }

    None
}

/// Recursively search for node.exe (Windows) or node (Unix) in a directory
fn search_for_node_in_directory(dir: &Path) -> Option<PathBuf> {
    search_for_node_in_directory_inner(dir, 0)
}

fn search_for_node_in_directory_inner(dir: &Path, depth: usize) -> Option<PathBuf> {
    if depth > MAX_NVM_SEARCH_DEPTH {
        return None;
    }

    #[cfg(target_os = "windows")]
    let node_name = "node.exe";
    #[cfg(not(target_os = "windows"))]
    let node_name = "node";

    // Use std::fs for simplicity and avoid async recursion
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(node_name) {
            return Some(path);
        }

        if path.is_dir() {
            // Recursively search subdirectories (limit depth to avoid long scans)
            if let Some(found) = search_for_node_in_directory_inner(&path, depth + 1) {
                return Some(found);
            }
        }
    }

    None
}

/// Verify a path is actually Node.js by running --version
pub async fn verify_nodejs_executable(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(error::invalid_custom_path_error(
            path.display().to_string(),
            "File does not exist".to_string(),
        ));
    }

    // Check if it's executable (Unix-like systems)
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(path).await {
            Ok(metadata) => {
                let permissions = metadata.permissions();
                if permissions.mode() & 0o111 == 0 {
                    return Err(error::invalid_custom_path_error(
                        path.display().to_string(),
                        "File is not executable".to_string(),
                    ));
                }
            }
            Err(e) => {
                return Err(error::invalid_custom_path_error(
                    path.display().to_string(),
                    format!("Cannot read file metadata: {}", e),
                ));
            }
        }
    }

    // Run 'node --version' to verify it's actually Node.js
    match tokio::time::timeout(COMMAND_TIMEOUT, new_command(path).arg("--version").output()).await {
        Ok(Ok(output)) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            let version = version.trim().to_string();

            if version.starts_with('v')
                || version.chars().next().map_or(false, |c| c.is_ascii_digit())
            {
                log::info!("Verified Node.js at {}: {}", path.display(), version);
                Ok(version)
            } else {
                Err(error::validation_failed_error(
                    path.display().to_string(),
                    format!("Unexpected version output: {}", version),
                ))
            }
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(error::validation_failed_error(
                path.display().to_string(),
                format!("Command failed with: {}", stderr.trim()),
            ))
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(error::permission_denied_error(path.display().to_string()))
        }
        Ok(Err(e)) => Err(anyhow::Error::from(e))
            .with_context(|| format!("Failed to execute {}", path.display())),
        Err(_) => Err(anyhow::anyhow!(
            "Timed out running '{} --version' after {:?}",
            path.display(),
            COMMAND_TIMEOUT
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_system_nodejs() {
        // This test will return different results based on the system
        let result = detect_system_nodejs(NodeJsDetectionMode::Full).await;
        // Just ensure it doesn't panic
        log::debug!("Detected Node.js: {:?}", result);
    }

    #[tokio::test]
    async fn test_verify_invalid_path() {
        let invalid_path = PathBuf::from("/nonexistent/node");
        let result = verify_nodejs_executable(&invalid_path).await;
        assert!(result.is_err());
    }
}
