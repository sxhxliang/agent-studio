use anyhow::anyhow;

/// Helper functions for creating user-friendly Node.js error messages
/// Create error for Node.js not found with installation hint
pub fn nodejs_not_found_error(install_hint: String) -> anyhow::Error {
    anyhow!(
        "Node.js is not installed or could not be found.\n\n{}",
        install_hint
    )
}

/// Create error for invalid custom path
pub fn invalid_custom_path_error(path: String, reason: String) -> anyhow::Error {
    anyhow!(
        "The custom Node.js path '{}' is invalid:\n{}\n\n\
         Please check your Settings > General > Node.js Path configuration.",
        path,
        reason
    )
}

/// Create error for validation failure
pub fn validation_failed_error(path: String, reason: String) -> anyhow::Error {
    anyhow!(
        "Node.js at '{}' failed validation:\n{}\n\n\
         The file may not be a valid Node.js executable.",
        path,
        reason
    )
}

/// Create error for permission denied
pub fn permission_denied_error(path: String) -> anyhow::Error {
    anyhow!(
        "Permission denied when trying to access Node.js at '{}'.\n\n\
         Please check file permissions.",
        path
    )
}
