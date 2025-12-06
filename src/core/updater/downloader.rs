use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Progress callback for download operations
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Update downloader
pub struct UpdateDownloader {
    /// Directory to download updates to
    download_dir: PathBuf,
}

impl UpdateDownloader {
    /// Create a new downloader with default download directory
    pub fn new() -> Result<Self> {
        let download_dir = std::env::temp_dir().join("agentx_updates");
        std::fs::create_dir_all(&download_dir)?;

        Ok(Self { download_dir })
    }

    /// Create downloader with custom download directory
    pub fn with_dir(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self { download_dir: dir })
    }

    /// Download update from URL to local file
    ///
    /// # Arguments
    /// * `url` - URL to download from
    /// * `filename` - Optional filename (will be extracted from URL if not provided)
    /// * `progress` - Optional progress callback (current_bytes, total_bytes)
    ///
    /// # Returns
    /// Path to the downloaded file
    pub async fn download(
        &self,
        url: &str,
        filename: Option<&str>,
        _progress: Option<ProgressCallback>,
    ) -> Result<PathBuf> {
        let filename = filename
            .map(|s| s.to_string())
            .or_else(|| Self::extract_filename_from_url(url))
            .ok_or_else(|| anyhow!("Could not determine filename"))?;

        let file_path = self.download_dir.join(&filename);

        // TODO: Implement real HTTP download
        // Example implementation:
        // let client = reqwest::Client::new();
        // let mut response = client.get(url).send().await?;
        // let total_size = response.content_length().unwrap_or(0);
        //
        // let mut file = tokio::fs::File::create(&file_path).await?;
        // let mut downloaded = 0u64;
        //
        // while let Some(chunk) = response.chunk().await? {
        //     file.write_all(&chunk).await?;
        //     downloaded += chunk.len() as u64;
        //     if let Some(ref callback) = progress {
        //         callback(downloaded, total_size);
        //     }
        // }
        //
        // file.flush().await?;

        log::info!("Download would save to: {:?}", file_path);

        Err(anyhow!("Download not yet implemented. Please add reqwest dependency and implement HTTP client."))
    }

    /// Extract filename from URL
    fn extract_filename_from_url(url: &str) -> Option<String> {
        url.split('/')
            .next_back()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    /// Get the download directory
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    /// Clean up old downloads
    pub async fn cleanup(&self) -> Result<()> {
        if self.download_dir.exists() {
            tokio::fs::remove_dir_all(&self.download_dir).await?;
            tokio::fs::create_dir_all(&self.download_dir).await?;
        }
        Ok(())
    }
}

impl Default for UpdateDownloader {
    fn default() -> Self {
        Self::new().expect("Failed to create default downloader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_filename() {
        assert_eq!(
            UpdateDownloader::extract_filename_from_url("https://example.com/app-v1.0.0.dmg"),
            Some("app-v1.0.0.dmg".to_string())
        );

        assert_eq!(
            UpdateDownloader::extract_filename_from_url("https://example.com/"),
            None
        );
    }
}
