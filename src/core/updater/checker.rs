use super::version::Version;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn tokio_handle() -> tokio::runtime::Handle {
    tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
        RUNTIME
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("Failed to create tokio runtime for update checker")
            })
            .handle()
            .clone()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
    pub release_notes: String,
    pub published_at: String,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    NoUpdate,
    UpdateAvailable(UpdateInfo),
    Error(String),
}

#[derive(Clone)]
pub struct UpdateChecker {
    check_url: String,
    timeout: Duration,
}

impl UpdateChecker {
    pub fn new() -> Self {
        Self {
            check_url: "https://api.github.com/repos/sxhxliang/agent-studio/releases/latest"
                .to_string(),
            timeout: Duration::from_secs(10),
        }
    }

    /// Safe to call from any async executor (GPUI, tokio, etc.).
    pub async fn check_for_updates(&self) -> UpdateCheckResult {
        let check_url = self.check_url.clone();
        let timeout = self.timeout;

        let fetch_result = tokio_handle()
            .spawn(async move { fetch_latest_release(&check_url, timeout).await })
            .await;

        let info = match fetch_result {
            Ok(Ok(info)) => info,
            Ok(Err(e)) => {
                log::error!("Failed to check for updates: {}", e);
                return UpdateCheckResult::Error(e.to_string());
            }
            Err(e) => {
                log::error!("Update check task failed: {}", e);
                return UpdateCheckResult::Error(e.to_string());
            }
        };

        let current = Version::current();
        match Version::parse(&info.version) {
            Ok(latest) if latest.is_newer_than(&current) => {
                log::info!("Update available: {} -> {}", current, latest);
                UpdateCheckResult::UpdateAvailable(info)
            }
            Ok(_) => {
                log::info!("No update available (current: {})", current);
                UpdateCheckResult::NoUpdate
            }
            Err(e) => {
                log::error!("Failed to parse remote version: {}", e);
                UpdateCheckResult::Error(format!("Invalid version format: {}", e))
            }
        }
    }
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

async fn fetch_latest_release(check_url: &str, timeout: Duration) -> Result<UpdateInfo> {
    log::info!("Fetching latest release from: {}", check_url);

    let client = reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(format!("AgentStudio/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let response = client
        .get(check_url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "GitHub API returned status: {}",
            response.status()
        ));
    }

    let body = response.text().await?;
    let release: GitHubRelease = serde_json::from_str(&body)?;
    let download_url = find_platform_asset(&release.assets);

    Ok(UpdateInfo {
        version: release.tag_name,
        download_url,
        release_notes: release.body.unwrap_or_default(),
        published_at: release.published_at,
        file_size: release.assets.first().map(|a| a.size),
    })
}

fn find_platform_asset(assets: &[GitHubAsset]) -> String {
    let patterns: &[&str] = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => &["aarch64-apple-darwin", "arm64-macos", "darwin-arm64"],
        ("macos", "x86_64") => &["x86_64-apple-darwin", "x64-macos", "darwin-x64"],
        ("windows", "x86_64") => &["x86_64-pc-windows", "win64", "windows-x64"],
        ("windows", "aarch64") => &["aarch64-pc-windows", "win-arm64", "windows-arm64"],
        ("linux", "x86_64") => &["x86_64-unknown-linux", "linux-x64", "linux64"],
        ("linux", "aarch64") => &["aarch64-unknown-linux", "linux-arm64"],
        _ => &[],
    };

    for pattern in patterns {
        if let Some(asset) = assets.iter().find(|a| {
            a.name.to_lowercase().contains(pattern)
                || a.browser_download_url.to_lowercase().contains(pattern)
        }) {
            return asset.browser_download_url.clone();
        }
    }

    assets
        .first()
        .map(|a| a.browser_download_url.clone())
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    published_at: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}
