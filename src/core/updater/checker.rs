use super::version::Version;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Information about an available update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
    pub release_notes: String,
    pub published_at: String,
    pub file_size: Option<u64>,
}

impl UpdateInfo {
    pub fn parse_version(&self) -> Result<Version> {
        Version::parse(&self.version).map_err(|e| anyhow!("{}", e))
    }
}

/// Result of checking for updates
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// No update available (current version is latest)
    NoUpdate,
    /// Update available with info
    UpdateAvailable(UpdateInfo),
    /// Error occurred while checking
    Error(String),
}

/// Update checker that queries remote API for available updates
#[derive(Clone)]
pub struct UpdateChecker {
    /// API endpoint to check for updates
    check_url: String,
    /// Timeout for HTTP requests
    timeout: Duration,
}

impl UpdateChecker {
    /// Create a new update checker with default GitHub API endpoint
    pub fn new() -> Self {
        Self {
            check_url: "https://api.github.com/repos/sxhxliang/agent_studio/releases/latest"
                .to_string(),
            timeout: Duration::from_secs(10),
        }
    }

    /// Create update checker with custom endpoint
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            check_url: url.into(),
            timeout: Duration::from_secs(10),
        }
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check for available updates
    ///
    /// This is an async function that needs to be called from a tokio runtime.
    /// Returns UpdateCheckResult indicating whether an update is available.
    pub async fn check_for_updates(&self) -> UpdateCheckResult {
        match self.fetch_latest_release().await {
            Ok(info) => {
                let current = Version::current();
                match info.parse_version() {
                    Ok(latest) => {
                        if latest.is_newer_than(&current) {
                            log::info!("Update available: {} -> {}", current, latest);
                            UpdateCheckResult::UpdateAvailable(info)
                        } else {
                            log::info!("No update available (current: {})", current);
                            UpdateCheckResult::NoUpdate
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to parse remote version: {}", e);
                        UpdateCheckResult::Error(format!("Invalid version format: {}", e))
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to check for updates: {}", e);
                UpdateCheckResult::Error(e.to_string())
            }
        }
    }

    /// Fetch latest release information from GitHub API
    async fn fetch_latest_release(&self) -> Result<UpdateInfo> {
        // For now, return a mock response
        // In production, this would use reqwest to fetch from the API

        // TODO: Implement real HTTP client
        // Example implementation:
        // let client = reqwest::Client::builder()
        //     .timeout(self.timeout)
        //     .user_agent("AgentStudio")
        //     .build()?;
        //
        // let response = client.get(&self.check_url).send().await?;
        // let release: GitHubRelease = response.json().await?;
        //
        // Ok(UpdateInfo {
        //     version: release.tag_name,
        //     download_url: release.assets[0].browser_download_url.clone(),
        //     release_notes: release.body,
        //     published_at: release.published_at,
        //     file_size: Some(release.assets[0].size),
        // })

        // Mock implementation for demonstration
        Err(anyhow!(
            "Update checking not yet implemented. Please add reqwest dependency and implement HTTP client."
        ))
    }
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// GitHub release API response structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    body: String,
    published_at: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}
