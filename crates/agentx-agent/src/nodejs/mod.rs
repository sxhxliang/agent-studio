mod detector;
mod error;
mod installer_hint;

pub use installer_hint::{PackageManager, generate_install_hint};

use anyhow::Result;
use std::path::PathBuf;
use std::sync::OnceLock;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Result of Node.js availability check
#[derive(Debug, Clone)]
pub struct NodeJsCheckResult {
    /// Whether Node.js is available
    pub available: bool,
    /// Path to the Node.js executable (if found)
    pub path: Option<PathBuf>,
    /// Version string (e.g., "v18.16.0")
    pub version: Option<String>,
    /// Error message if Node.js is not available
    pub error_message: Option<String>,
    /// Installation hint for the user
    pub install_hint: Option<String>,
}

/// Detection strategy for Node.js discovery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeJsDetectionMode {
    /// Fast detection that avoids slow shell startup or deep scans
    Fast,
    /// Full detection that may run additional checks
    Full,
}

/// Node.js environment checker
pub struct NodeJsChecker {
    custom_path: Option<PathBuf>,
    detection_mode: NodeJsDetectionMode,
}

impl NodeJsChecker {
    /// Create a new Node.js checker
    ///
    /// # Arguments
    /// * `custom_path` - Optional custom path to Node.js executable.
    ///                   If None, will auto-detect from PATH and standard locations.
    pub fn new(custom_path: Option<PathBuf>) -> Self {
        Self {
            custom_path,
            detection_mode: NodeJsDetectionMode::Full,
        }
    }

    /// Configure detection mode (fast vs full).
    pub fn with_detection_mode(mut self, mode: NodeJsDetectionMode) -> Self {
        self.detection_mode = mode;
        self
    }

    /// Check if Node.js is available
    ///
    /// Returns a detailed result with path, version, and installation hints if needed.
    pub async fn check_nodejs_available(&self) -> Result<NodeJsCheckResult> {
        // Priority 1: Custom path from settings
        if let Some(ref custom_path) = self.custom_path {
            log::debug!("Checking custom Node.js path: {}", custom_path.display());

            match detector::verify_nodejs_executable(custom_path).await {
                Ok(version) => {
                    return Ok(NodeJsCheckResult {
                        available: true,
                        path: Some(custom_path.clone()),
                        version: Some(version),
                        error_message: None,
                        install_hint: None,
                    });
                }
                Err(e) => {
                    log::warn!("Custom Node.js path validation failed: {}", e);
                    // Fall through to auto-detection
                }
            }
        }

        // Priority 2-4: Auto-detection (PATH, standard locations, NVM)
        if let Some(detected_path) = detector::detect_system_nodejs(self.detection_mode).await {
            log::debug!("Auto-detected Node.js at: {}", detected_path.display());

            match detector::verify_nodejs_executable(&detected_path).await {
                Ok(version) => {
                    return Ok(NodeJsCheckResult {
                        available: true,
                        path: Some(detected_path),
                        version: Some(version),
                        error_message: None,
                        install_hint: None,
                    });
                }
                Err(e) => {
                    log::warn!(
                        "Detected Node.js at {} failed verification: {}",
                        detected_path.display(),
                        e
                    );
                }
            }
        }

        // Node.js not found - generate installation hint
        log::warn!("Node.js not found on system");

        let install_hint = installer_hint::generate_install_hint().await;

        Ok(NodeJsCheckResult {
            available: false,
            path: None,
            version: None,
            error_message: Some("Node.js is not installed or could not be found".to_string()),
            install_hint: Some(install_hint),
        })
    }

    /// Blocking Node.js availability check that works without a Tokio runtime.
    pub fn check_nodejs_available_blocking(&self) -> Result<NodeJsCheckResult> {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            return handle.block_on(self.check_nodejs_available());
        }

        let runtime = RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("Failed to initialize Node.js checker runtime")
        });

        runtime.block_on(self.check_nodejs_available())
    }

    /// Quick boolean check for Node.js availability
    ///
    /// Returns true if Node.js is available, false otherwise.
    /// This is a convenience method that doesn't return detailed information.
    pub async fn is_nodejs_available(&self) -> bool {
        match self.check_nodejs_available().await {
            Ok(result) => result.available,
            Err(_) => false,
        }
    }

    /// Get the actual Node.js path (for logging/debugging)
    ///
    /// Returns the path to Node.js executable if found.
    pub async fn get_nodejs_path(&self) -> Option<PathBuf> {
        match self.check_nodejs_available().await {
            Ok(result) => result.path,
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_nodejs_available() {
        let checker = NodeJsChecker::new(None);
        let result = checker.check_nodejs_available().await;

        assert!(result.is_ok());
        let result = result.unwrap();

        // Log the result for debugging
        log::debug!("Node.js check result: {:?}", result);

        // If Node.js is available, we should have path and version
        if result.available {
            assert!(result.path.is_some());
            assert!(result.version.is_some());
            assert!(result.error_message.is_none());
            assert!(result.install_hint.is_none());
        } else {
            // If not available, we should have error and install hint
            assert!(result.path.is_none());
            assert!(result.version.is_none());
            assert!(result.error_message.is_some());
            assert!(result.install_hint.is_some());
        }
    }

    #[tokio::test]
    async fn test_is_nodejs_available() {
        let checker = NodeJsChecker::new(None);
        let available = checker.is_nodejs_available().await;

        // This test will pass whether Node.js is installed or not
        log::debug!("Node.js available: {}", available);
    }

    #[tokio::test]
    async fn test_invalid_custom_path() {
        let checker = NodeJsChecker::new(Some(PathBuf::from("/nonexistent/node")));
        let result = checker.check_nodejs_available().await;

        assert!(result.is_ok());
        let result = result.unwrap();

        // With invalid custom path, should fall back to auto-detection
        // or return not available
        log::debug!("Result with invalid custom path: {:?}", result);
    }

    #[tokio::test]
    async fn test_get_nodejs_path() {
        let checker = NodeJsChecker::new(None);
        let path = checker.get_nodejs_path().await;

        log::debug!("Node.js path: {:?}", path);
    }
}
