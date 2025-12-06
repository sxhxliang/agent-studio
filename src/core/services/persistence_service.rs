//! Persistence Service - Handles message persistence to JSONL files
//!
//! This service saves session updates to disk in JSONL format (one JSON object per line)
//! and loads historical messages when needed.

use std::path::PathBuf;

use agent_client_protocol::SessionUpdate;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persisted message entry with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMessage {
    /// Timestamp in ISO 8601 format
    pub timestamp: String,
    /// The session update
    pub update: SessionUpdate,
}

impl PersistedMessage {
    /// Create a new persisted message with current timestamp
    pub fn new(update: SessionUpdate) -> Self {
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self { timestamp, update }
    }

    /// Create from existing timestamp
    pub fn with_timestamp(timestamp: String, update: SessionUpdate) -> Self {
        Self { timestamp, update }
    }
}

/// Message persistence service
pub struct PersistenceService {
    /// Base directory for session files
    base_dir: PathBuf,
}

impl PersistenceService {
    /// Create a new persistence service
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for storing session files (e.g., "target/sessions")
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the file path for a session
    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(format!("{}.jsonl", session_id))
    }

    /// Ensure the base directory exists
    fn ensure_base_dir_sync(&self) -> Result<()> {
        if !self.base_dir.exists() {
            std::fs::create_dir_all(&self.base_dir).context("Failed to create base directory")?;
        }
        Ok(())
    }

    /// Save a session update to disk
    ///
    /// Appends the update to the session's JSONL file
    pub async fn save_update(&self, session_id: &str, update: SessionUpdate) -> Result<()> {
        let file_path = self.session_file_path(session_id);
        let base_dir = self.base_dir.clone();
        let message = PersistedMessage::new(update);

        // Use smol::unblock to run blocking I/O in a thread pool
        smol::unblock(move || {
            // Ensure directory exists
            if !base_dir.exists() {
                std::fs::create_dir_all(&base_dir).context("Failed to create base directory")?;
            }

            // Serialize to JSON and append newline
            let json = serde_json::to_string(&message).context("Failed to serialize message")?;

            // Open file in append mode
            use std::fs::OpenOptions;
            use std::io::Write;

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&file_path)
                .context("Failed to open session file")?;

            // Write JSON line
            write!(file, "{}\n", json).context("Failed to write message")?;

            log::debug!("Saved message to session file: {}", file_path.display());
            Ok(())
        })
        .await
    }

    /// Load all messages for a session
    ///
    /// Returns messages in chronological order
    pub async fn load_messages(&self, session_id: &str) -> Result<Vec<PersistedMessage>> {
        let file_path = self.session_file_path(session_id);
        let session_id = session_id.to_string(); // Clone for the closure

        // Use smol::unblock to run blocking I/O
        smol::unblock(move || {
            // Check if file exists
            if !file_path.exists() {
                log::debug!("No history file found for session: {}", session_id);
                return Ok(Vec::new());
            }

            use std::fs::File;
            use std::io::{BufRead, BufReader};

            let file = File::open(&file_path).context("Failed to open session file")?;

            let reader = BufReader::new(file);
            let mut messages = Vec::new();

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<PersistedMessage>(&line) {
                    Ok(message) => messages.push(message),
                    Err(e) => {
                        log::warn!("Failed to parse line in session file: {}", e);
                        // Continue reading other lines
                    }
                }
            }

            log::info!(
                "Loaded {} messages from session file: {}",
                messages.len(),
                file_path.display()
            );
            Ok(messages)
        })
        .await
    }

    /// Delete a session's history file
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let file_path = self.session_file_path(session_id);

        smol::unblock(move || {
            if file_path.exists() {
                std::fs::remove_file(&file_path).context("Failed to delete session file")?;
                log::info!("Deleted session file: {}", file_path.display());
            }
            Ok(())
        })
        .await
    }

    /// List all available sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        let base_dir = self.base_dir.clone();

        smol::unblock(move || {
            if !base_dir.exists() {
                return Ok(Vec::new());
            }

            let mut sessions = Vec::new();

            for entry in
                std::fs::read_dir(&base_dir).context("Failed to read sessions directory")?
            {
                let entry = entry?;
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if extension == "jsonl" {
                        if let Some(stem) = path.file_stem() {
                            if let Some(session_id) = stem.to_str() {
                                sessions.push(session_id.to_string());
                            }
                        }
                    }
                }
            }

            Ok(sessions)
        })
        .await
    }
}

// Tests commented out - can be enabled by adding tempfile dependency
/*
#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::ContentBlock;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_save_and_load_messages() {
        let temp_dir = TempDir::new().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        let session_id = "test-session";
        let update = SessionUpdate::AgentMessageChunk(
            agent_client_protocol::ContentChunk::new(
                ContentBlock::from("Hello, world!".to_string())
            )
        );

        // Save message
        service.save_update(session_id, update.clone()).await.unwrap();

        // Load messages
        let messages = service.load_messages(session_id).await.unwrap();
        assert_eq!(messages.len(), 1);

        // Verify content
        match &messages[0].update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                match &chunk.content_block.content {
                    ContentBlock::Text(text) => {
                        assert_eq!(text.text, "Hello, world!");
                    }
                    _ => panic!("Expected text content"),
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        // Create some session files
        service.save_update("session-1", SessionUpdate::AgentMessageChunk(
            agent_client_protocol::ContentChunk::new(
                ContentBlock::from("Message 1".to_string())
            )
        )).await.unwrap();

        service.save_update("session-2", SessionUpdate::AgentMessageChunk(
            agent_client_protocol::ContentChunk::new(
                ContentBlock::from("Message 2".to_string())
            )
        )).await.unwrap();

        // List sessions
        let sessions = service.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&"session-1".to_string()));
        assert!(sessions.contains(&"session-2".to_string()));
    }
}
*/
