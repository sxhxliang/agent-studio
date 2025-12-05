mod checker;
mod downloader;
mod version;

pub use checker::{UpdateCheckResult, UpdateChecker, UpdateInfo};
pub use downloader::{ProgressCallback, UpdateDownloader};
pub use version::Version;

/// Update manager that coordinates checking, downloading, and installing updates
#[derive(Clone)]
pub struct UpdateManager {
    checker: UpdateChecker,
}

impl UpdateManager {
    /// Create a new update manager with default configuration
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            checker: UpdateChecker::new(),
        })
    }

    /// Check for available updates
    pub async fn check_for_updates(&self) -> UpdateCheckResult {
        self.checker.check_for_updates().await
    }

    /// Download an update
    pub async fn download_update(
        &self,
        info: &UpdateInfo,
        progress: Option<ProgressCallback>,
    ) -> anyhow::Result<std::path::PathBuf> {
        let downloader = UpdateDownloader::new()?;
        downloader
            .download(&info.download_url, None, progress)
            .await
    }

    /// Get current application version
    pub fn current_version() -> Version {
        Version::current()
    }
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default UpdateManager")
    }
}
