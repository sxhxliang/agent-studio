//! Persistence Service - Handles message persistence to JSONL files
//!
//! This service saves session updates to disk in JSONL format (one JSON object per line)
//! and loads historical messages when needed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_client_protocol::{
    ContentBlock, ContentChunk, SessionUpdate, TextContent, ToolCallStatus, ToolCallUpdate,
};
use anyhow::{Context, Result};
use chrono::Utc;
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

/// Type of chunk being accumulated
#[derive(Debug, Clone, PartialEq)]
enum AccumulatedChunkType {
    AgentMessage,
    AgentThought,
    UserMessage,
    Empty, // Initial state, no chunks yet
}

/// Accumulates chunks for a session before flushing to disk
struct ChunkAccumulator {
    /// Timestamp of first chunk in the sequence
    first_timestamp: String,
    /// Type of chunks being accumulated
    chunk_type: AccumulatedChunkType,
    /// Accumulated chunks for AgentMessageChunk
    agent_message_chunks: Vec<ContentChunk>,
    /// Accumulated text for AgentThoughtChunk
    agent_thought_text: String,
    /// Accumulated chunks for UserMessageChunk
    user_message_chunks: Vec<ContentChunk>,
    /// Tool call updates: toolCallId -> (first_timestamp, latest_update)
    /// Only keeps the latest update for each tool call
    tool_call_updates: HashMap<String, (String, ToolCallUpdate)>,
}

impl ChunkAccumulator {
    /// Create a new empty accumulator
    fn new() -> Self {
        Self {
            first_timestamp: String::new(),
            chunk_type: AccumulatedChunkType::Empty,
            agent_message_chunks: Vec::new(),
            agent_thought_text: String::new(),
            user_message_chunks: Vec::new(),
            tool_call_updates: HashMap::new(),
        }
    }

    /// Try to append an AgentMessageChunk
    /// Returns Some(FlushData) if type change requires flush, None if accumulated
    fn try_append_agent_message_chunk(&mut self, chunk: ContentChunk) -> Option<FlushData> {
        match self.chunk_type {
            AccumulatedChunkType::Empty => {
                // First chunk - initialize
                self.chunk_type = AccumulatedChunkType::AgentMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_message_chunks.push(chunk);
                None // No flush needed
            }
            AccumulatedChunkType::AgentMessage => {
                // Same type - accumulate
                self.agent_message_chunks.push(chunk);
                None
            }
            _ => {
                // Type changed - flush old, start new
                let flushed = self.flush();
                self.chunk_type = AccumulatedChunkType::AgentMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_message_chunks.push(chunk);
                Some(FlushData::Accumulated(Box::new(flushed)))
            }
        }
    }

    /// Try to append an AgentThoughtChunk
    /// Returns Some(FlushData) if type change requires flush, None if accumulated
    fn try_append_agent_thought_chunk(&mut self, chunk: ContentChunk) -> Option<FlushData> {
        let text = extract_text_from_content_chunk(&chunk);

        match self.chunk_type {
            AccumulatedChunkType::Empty => {
                self.chunk_type = AccumulatedChunkType::AgentThought;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_thought_text = text;
                None
            }
            AccumulatedChunkType::AgentThought => {
                // Append text (same as ConversationPanel logic)
                self.agent_thought_text.push_str(&text);
                None
            }
            _ => {
                let flushed = self.flush();
                self.chunk_type = AccumulatedChunkType::AgentThought;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.agent_thought_text = text;
                Some(FlushData::Accumulated(Box::new(flushed)))
            }
        }
    }

    /// Try to append a UserMessageChunk
    /// Returns Some(FlushData) if type change requires flush, None if accumulated
    fn try_append_user_message_chunk(&mut self, chunk: ContentChunk) -> Option<FlushData> {
        match self.chunk_type {
            AccumulatedChunkType::Empty => {
                self.chunk_type = AccumulatedChunkType::UserMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.user_message_chunks.push(chunk);
                None
            }
            AccumulatedChunkType::UserMessage => {
                self.user_message_chunks.push(chunk);
                None
            }
            _ => {
                let flushed = self.flush();
                self.chunk_type = AccumulatedChunkType::UserMessage;
                self.first_timestamp = Utc::now().to_rfc3339();
                self.user_message_chunks.push(chunk);
                Some(FlushData::Accumulated(Box::new(flushed)))
            }
        }
    }

    /// Accumulate a tool call update
    /// Only keeps the latest update for each tool call ID
    /// When status is Completed/Failed, returns FlushData to write immediately
    fn accumulate_tool_call_update(&mut self, update: ToolCallUpdate) -> Option<FlushData> {
        let tool_call_id = update.tool_call_id.to_string();
        let timestamp = Utc::now().to_rfc3339();

        // Check if this is a terminal state (Completed/Failed)
        let is_terminal = update
            .fields
            .status
            .as_ref()
            .map(|s| matches!(s, ToolCallStatus::Completed | ToolCallStatus::Failed))
            .unwrap_or(false);

        if is_terminal {
            // Terminal state: write immediately
            // Check if we have accumulated earlier updates for this tool call
            let final_update = if let Some((first_timestamp, _existing)) =
                self.tool_call_updates.remove(&tool_call_id)
            {
                // Use the first timestamp for the final update
                log::info!(
                    "Tool call {} reached terminal state {:?}, flushing immediately",
                    tool_call_id,
                    update.fields.status
                );
                (first_timestamp, SessionUpdate::ToolCallUpdate(update))
            } else {
                // First time seeing this tool call and it's already complete
                log::info!(
                    "Tool call {} received in terminal state {:?}, writing immediately",
                    tool_call_id,
                    update.fields.status
                );
                (timestamp, SessionUpdate::ToolCallUpdate(update))
            };

            Some(FlushData::ToolCallCompleted(Box::new(final_update)))
        } else {
            // Non-terminal state: accumulate
            self.tool_call_updates
                .entry(tool_call_id.clone())
                .and_modify(|(_ts, existing_update)| {
                    // Keep the first timestamp, update the content
                    *existing_update = update.clone();
                    log::debug!(
                        "Updated tool_call_update for toolCallId: {} (status: {:?})",
                        tool_call_id,
                        update.fields.status
                    );
                })
                .or_insert_with(|| {
                    log::debug!(
                        "First tool_call_update for toolCallId: {} (status: {:?})",
                        tool_call_id,
                        update.fields.status
                    );
                    (timestamp, update)
                });

            None // Continue accumulating
        }
    }

    /// Flush accumulated chunks into a SessionUpdate
    /// Returns None if nothing accumulated, Some((timestamp, update)) otherwise
    fn flush(&mut self) -> Option<(String, SessionUpdate)> {
        if matches!(self.chunk_type, AccumulatedChunkType::Empty) {
            return None; // Nothing to flush
        }

        let timestamp = self.first_timestamp.clone();
        let update = match self.chunk_type {
            AccumulatedChunkType::AgentMessage => {
                let merged_chunk = merge_text_chunks(&self.agent_message_chunks);
                SessionUpdate::AgentMessageChunk(merged_chunk)
            }
            AccumulatedChunkType::AgentThought => {
                let text_block =
                    ContentBlock::Text(TextContent::new(self.agent_thought_text.clone()));
                SessionUpdate::AgentThoughtChunk(ContentChunk::new(text_block))
            }
            AccumulatedChunkType::UserMessage => {
                let merged_chunk = merge_text_chunks(&self.user_message_chunks);
                SessionUpdate::UserMessageChunk(merged_chunk)
            }
            AccumulatedChunkType::Empty => unreachable!(),
        };

        // Reset chunk state (but keep tool_call_updates)
        self.chunk_type = AccumulatedChunkType::Empty;
        self.first_timestamp = String::new();
        self.agent_message_chunks.clear();
        self.agent_thought_text.clear();
        self.user_message_chunks.clear();

        Some((timestamp, update))
    }

    /// Flush all tool call updates
    /// Returns a vector of (timestamp, update) pairs
    fn flush_tool_call_updates(&mut self) -> Vec<(String, SessionUpdate)> {
        if self.tool_call_updates.is_empty() {
            return Vec::new();
        }

        let updates: Vec<(String, SessionUpdate)> = self
            .tool_call_updates
            .drain()
            .map(|(_tool_call_id, (timestamp, update))| {
                (timestamp, SessionUpdate::ToolCallUpdate(update))
            })
            .collect();

        log::info!("Flushed {} tool_call_updates", updates.len());
        updates
    }
}

/// Data to be flushed to disk
enum FlushData {
    /// Only accumulated data to write (boxed for size)
    Accumulated(Box<Option<(String, SessionUpdate)>>),
    /// Both accumulated data and a new non-chunk update (boxed for size)
    Both(Box<(Option<(String, SessionUpdate)>, SessionUpdate)>),
    /// A completed tool call update (status=Completed/Failed) (boxed for size)
    ToolCallCompleted(Box<(String, SessionUpdate)>),
}

/// Merge multiple ContentChunks into a single ContentChunk
/// For text chunks, concatenates all text into one chunk
fn merge_text_chunks(chunks: &[ContentChunk]) -> ContentChunk {
    if chunks.is_empty() {
        return ContentChunk::new(ContentBlock::Text(TextContent::new(String::new())));
    }

    if chunks.len() == 1 {
        return chunks[0].clone();
    }

    // Check if all chunks are text
    let all_text = chunks
        .iter()
        .all(|chunk| matches!(chunk.content, ContentBlock::Text(_)));

    if all_text {
        // Concatenate all text
        let merged_text = chunks
            .iter()
            .filter_map(|chunk| {
                if let ContentBlock::Text(text) = &chunk.content {
                    Some(text.text.as_str())
                } else {
                    None
                }
            })
            .collect::<String>();

        let text_block = ContentBlock::Text(TextContent::new(merged_text));
        let mut merged_chunk = ContentChunk::new(text_block);

        // Preserve meta from first chunk if any
        if let Some(meta) = &chunks[0].meta {
            merged_chunk.meta = Some(meta.clone());
        }

        return merged_chunk;
    }

    // Mixed content (text + images) - should not happen in practice
    // but handle defensively by returning first chunk
    log::warn!("Attempted to merge heterogeneous chunks, returning first chunk only");
    chunks[0].clone()
}

/// Extract text from ContentChunk (for AgentThoughtChunk)
fn extract_text_from_content_chunk(chunk: &ContentChunk) -> String {
    match &chunk.content {
        ContentBlock::Text(text) => text.text.clone(),
        ContentBlock::Image(img) => format!("[Image: {}]", img.mime_type),
        _ => "[Non-text content]".to_string(),
    }
}

/// Message persistence service
pub struct PersistenceService {
    /// Base directory for session files
    base_dir: PathBuf,
    /// Thread-safe storage for chunk accumulators per session
    accumulators: Arc<Mutex<HashMap<String, ChunkAccumulator>>>,
}

impl PersistenceService {
    /// Create a new persistence service
    ///
    /// # Arguments
    /// * `base_dir` - Base directory for storing session files (e.g., "target/sessions")
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            accumulators: Arc::new(Mutex::new(HashMap::new())),
        }
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
    /// Accumulates chunk updates and tool_call_updates in memory and flushes when needed.
    /// Non-chunk updates trigger immediate flush and write.
    pub async fn save_update(&self, session_id: &str, update: SessionUpdate) -> Result<()> {
        let flush_data = {
            let mut accumulators = self.accumulators.lock().unwrap();
            let accumulator = accumulators
                .entry(session_id.to_string())
                .or_insert_with(ChunkAccumulator::new);

            match update {
                SessionUpdate::AgentMessageChunk(chunk) => {
                    log::debug!("Accumulating AgentMessageChunk for session: {}", session_id);
                    accumulator.try_append_agent_message_chunk(chunk)
                }
                SessionUpdate::AgentThoughtChunk(chunk) => {
                    log::debug!("Accumulating AgentThoughtChunk for session: {}", session_id);
                    accumulator.try_append_agent_thought_chunk(chunk)
                }
                SessionUpdate::UserMessageChunk(chunk) => {
                    log::debug!("Accumulating UserMessageChunk for session: {}", session_id);
                    accumulator.try_append_user_message_chunk(chunk)
                }
                SessionUpdate::ToolCallUpdate(update) => {
                    log::debug!(
                        "Accumulating ToolCallUpdate for session: {}, toolCallId: {}",
                        session_id,
                        update.tool_call_id
                    );
                    accumulator.accumulate_tool_call_update(update)
                }
                _ => {
                    // Non-chunk update: flush accumulator, then write both
                    log::debug!(
                        "Non-chunk update received, flushing accumulator for session: {}",
                        session_id
                    );
                    let flushed = accumulator.flush();
                    Some(FlushData::Both(Box::new((flushed, update))))
                }
            }
        }; // Lock released here

        // Write outside lock to avoid blocking
        if let Some(data) = flush_data {
            self.write_flush_data(session_id, data).await?;
        }

        Ok(())
    }

    /// Write flush data to disk
    async fn write_flush_data(&self, session_id: &str, data: FlushData) -> Result<()> {
        match data {
            FlushData::Accumulated(boxed_data) => {
                if let Some((timestamp, update)) = *boxed_data {
                    // Write only accumulated data
                    self.write_with_timestamp(session_id, update, timestamp)
                        .await?;
                }
            }
            FlushData::Both(boxed_data) => {
                let (accumulated, non_chunk) = *boxed_data;
                // Write accumulated data first (if any)
                if let Some((timestamp, update)) = accumulated {
                    self.write_with_timestamp(session_id, update, timestamp)
                        .await?;
                }
                // Then write non-chunk update with current timestamp
                self.write_with_current_timestamp(session_id, non_chunk)
                    .await?;
            }
            FlushData::ToolCallCompleted(boxed_data) => {
                let (timestamp, update) = *boxed_data;
                // Write completed tool call update immediately
                log::debug!(
                    "Writing completed tool_call_update for session: {}",
                    session_id
                );
                self.write_with_timestamp(session_id, update, timestamp)
                    .await?;
            }
        }
        Ok(())
    }

    /// Write update with specific timestamp
    async fn write_with_timestamp(
        &self,
        session_id: &str,
        update: SessionUpdate,
        timestamp: String,
    ) -> Result<()> {
        let file_path = self.session_file_path(session_id);
        let base_dir = self.base_dir.clone();
        let message = PersistedMessage::with_timestamp(timestamp, update);

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

            log::debug!(
                "Wrote merged message to session file: {}",
                file_path.display()
            );
            Ok(())
        })
        .await
    }

    /// Write update with current timestamp
    async fn write_with_current_timestamp(
        &self,
        session_id: &str,
        update: SessionUpdate,
    ) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        self.write_with_timestamp(session_id, update, timestamp)
            .await
    }

    /// Flush accumulated chunks and tool_call_updates for a specific session
    ///
    /// This should be called when a session completes or becomes idle
    pub async fn flush_session(&self, session_id: &str) -> Result<()> {
        let (chunk_flush_data, tool_call_updates) = {
            let mut accumulators = self.accumulators.lock().unwrap();
            if let Some(acc) = accumulators.get_mut(session_id) {
                let chunks = acc.flush();
                let tool_calls = acc.flush_tool_call_updates();
                (chunks, tool_calls)
            } else {
                (None, Vec::new())
            }
        };

        let has_chunks = chunk_flush_data.is_some();
        let has_tool_calls = !tool_call_updates.is_empty();

        // Write accumulated chunks first (if any)
        if let Some((timestamp, update)) = chunk_flush_data {
            log::info!("Flushing accumulated chunks for session: {}", session_id);
            self.write_with_timestamp(session_id, update, timestamp)
                .await?;
        }

        // Write all accumulated tool_call_updates
        if has_tool_calls {
            log::info!(
                "Flushing {} tool_call_updates for session: {}",
                tool_call_updates.len(),
                session_id
            );
            for (timestamp, update) in tool_call_updates {
                self.write_with_timestamp(session_id, update, timestamp)
                    .await?;
            }
        }

        if !has_chunks && !has_tool_calls {
            log::debug!("No accumulated data to flush for session: {}", session_id);
        }

        Ok(())
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
    ///
    /// Flushes any pending chunks before deleting
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        // Flush pending chunks first
        self.flush_session(session_id).await?;

        // Remove accumulator
        {
            let mut accumulators = self.accumulators.lock().unwrap();
            accumulators.remove(session_id);
        }

        // Delete file
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

#[cfg(test)]
mod tests {
    use super::*;

    // ============== PersistedMessage tests ==============

    #[test]
    fn test_persisted_message_new() {
        let content = ContentBlock::Text(TextContent::new("test".to_string()));
        let chunk = ContentChunk::new(content);
        let update = SessionUpdate::UserMessageChunk(chunk);

        let msg = PersistedMessage::new(update);

        // Should have a valid RFC3339 timestamp
        assert!(!msg.timestamp.is_empty());
        assert!(chrono::DateTime::parse_from_rfc3339(&msg.timestamp).is_ok());
    }

    #[test]
    fn test_persisted_message_with_timestamp() {
        let content = ContentBlock::Text(TextContent::new("test".to_string()));
        let chunk = ContentChunk::new(content);
        let update = SessionUpdate::AgentMessageChunk(chunk);

        let timestamp = "2025-01-01T00:00:00+00:00".to_string();
        let msg = PersistedMessage::with_timestamp(timestamp.clone(), update);

        assert_eq!(msg.timestamp, timestamp);
    }

    #[test]
    fn test_persisted_message_serialization_roundtrip() {
        let content = ContentBlock::Text(TextContent::new("Hello world".to_string()));
        let chunk = ContentChunk::new(content);
        let update = SessionUpdate::UserMessageChunk(chunk);
        let msg = PersistedMessage::new(update);

        let json = serde_json::to_string(&msg).unwrap();
        let restored: PersistedMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.timestamp, restored.timestamp);
        // Verify the update type matches
        assert!(matches!(
            restored.update,
            SessionUpdate::UserMessageChunk(_)
        ));
    }

    // ============== merge_text_chunks tests ==============

    #[test]
    fn test_merge_text_chunks_empty() {
        let chunks: Vec<ContentChunk> = vec![];
        let merged = merge_text_chunks(&chunks);

        if let ContentBlock::Text(text) = &merged.content {
            assert!(text.text.is_empty());
        } else {
            panic!("Expected Text content block");
        }
    }

    #[test]
    fn test_merge_text_chunks_single() {
        let content = ContentBlock::Text(TextContent::new("single chunk".to_string()));
        let chunk = ContentChunk::new(content);
        let chunks = vec![chunk.clone()];

        let merged = merge_text_chunks(&chunks);

        if let ContentBlock::Text(text) = &merged.content {
            assert_eq!(text.text, "single chunk");
        } else {
            panic!("Expected Text content block");
        }
    }

    #[test]
    fn test_merge_text_chunks_multiple() {
        let chunks = vec![
            ContentChunk::new(ContentBlock::Text(TextContent::new("Hello ".to_string()))),
            ContentChunk::new(ContentBlock::Text(TextContent::new("World".to_string()))),
            ContentChunk::new(ContentBlock::Text(TextContent::new("!".to_string()))),
        ];

        let merged = merge_text_chunks(&chunks);

        if let ContentBlock::Text(text) = &merged.content {
            assert_eq!(text.text, "Hello World!");
        } else {
            panic!("Expected Text content block");
        }
    }

    #[test]
    fn test_merge_text_chunks_preserves_meta() {
        let mut first_chunk =
            ContentChunk::new(ContentBlock::Text(TextContent::new("first".to_string())));
        // Set meta as a Map
        let mut meta_map = serde_json::Map::new();
        meta_map.insert("key".to_string(), serde_json::json!("value"));
        first_chunk.meta = Some(meta_map);

        let second_chunk =
            ContentChunk::new(ContentBlock::Text(TextContent::new(" second".to_string())));

        let chunks = vec![first_chunk, second_chunk];
        let merged = merge_text_chunks(&chunks);

        // Meta from first chunk should be preserved
        assert!(merged.meta.is_some());
        assert_eq!(merged.meta.unwrap()["key"], "value");
    }

    // ============== extract_text_from_content_chunk tests ==============

    #[test]
    fn test_extract_text_from_text_chunk() {
        let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(
            "extracted text".to_string(),
        )));
        let text = extract_text_from_content_chunk(&chunk);
        assert_eq!(text, "extracted text");
    }

    #[test]
    fn test_extract_text_from_image_chunk() {
        use agent_client_protocol::ImageContent;

        // ImageContent::new(data, mime_type)
        let image_content = ImageContent::new("base64data".to_string(), "image/png".to_string());
        let chunk = ContentChunk::new(ContentBlock::Image(image_content));
        let text = extract_text_from_content_chunk(&chunk);

        assert!(text.contains("Image"));
        assert!(text.contains("image/png"));
    }

    // ============== ChunkAccumulator tests ==============

    #[test]
    fn test_chunk_accumulator_agent_message_same_type() {
        let mut acc = ChunkAccumulator::new();

        let chunk1 = ContentChunk::new(ContentBlock::Text(TextContent::new("chunk1".to_string())));
        let chunk2 = ContentChunk::new(ContentBlock::Text(TextContent::new("chunk2".to_string())));

        // First chunk should not trigger flush
        let result1 = acc.try_append_agent_message_chunk(chunk1);
        assert!(result1.is_none());

        // Second chunk of same type should not trigger flush
        let result2 = acc.try_append_agent_message_chunk(chunk2);
        assert!(result2.is_none());

        assert_eq!(acc.agent_message_chunks.len(), 2);
    }

    #[test]
    fn test_chunk_accumulator_type_change_triggers_flush() {
        let mut acc = ChunkAccumulator::new();

        let agent_chunk =
            ContentChunk::new(ContentBlock::Text(TextContent::new("agent".to_string())));
        let thought_chunk =
            ContentChunk::new(ContentBlock::Text(TextContent::new("thinking".to_string())));

        // First chunk
        acc.try_append_agent_message_chunk(agent_chunk);
        assert!(matches!(acc.chunk_type, AccumulatedChunkType::AgentMessage));

        // Type change should trigger flush
        let result = acc.try_append_agent_thought_chunk(thought_chunk);
        assert!(result.is_some());

        // Now should be AgentThought
        assert!(matches!(acc.chunk_type, AccumulatedChunkType::AgentThought));
    }

    #[test]
    fn test_chunk_accumulator_thought_text_concatenation() {
        let mut acc = ChunkAccumulator::new();

        let chunk1 = ContentChunk::new(ContentBlock::Text(TextContent::new("First ".to_string())));
        let chunk2 = ContentChunk::new(ContentBlock::Text(TextContent::new("thought".to_string())));

        acc.try_append_agent_thought_chunk(chunk1);
        acc.try_append_agent_thought_chunk(chunk2);

        assert_eq!(acc.agent_thought_text, "First thought");
    }

    #[test]
    fn test_chunk_accumulator_user_message_chunks() {
        let mut acc = ChunkAccumulator::new();

        let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new("user msg".to_string())));
        let result = acc.try_append_user_message_chunk(chunk);

        assert!(result.is_none());
        assert!(matches!(acc.chunk_type, AccumulatedChunkType::UserMessage));
        assert_eq!(acc.user_message_chunks.len(), 1);
    }

    #[test]
    fn test_chunk_accumulator_flush_empty() {
        let mut acc = ChunkAccumulator::new();
        let result = acc.flush();
        assert!(result.is_none());
    }

    #[test]
    fn test_chunk_accumulator_flush_resets_state() {
        let mut acc = ChunkAccumulator::new();

        let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new("to flush".to_string())));
        acc.try_append_agent_message_chunk(chunk);

        // Flush
        let result = acc.flush();
        assert!(result.is_some());

        // State should be reset
        assert!(matches!(acc.chunk_type, AccumulatedChunkType::Empty));
        assert!(acc.agent_message_chunks.is_empty());
        assert!(acc.first_timestamp.is_empty());
    }

    // Note: Tool call accumulator tests are skipped because ToolCallUpdate
    // uses #[non_exhaustive] and cannot be constructed directly in tests.
    // The tool call accumulator logic is implicitly tested through integration tests.

    // ============== PersistenceService file I/O tests ==============

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        let session_id = "test-session-1";

        // Save a user message chunk
        let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(
            "Hello from test".to_string(),
        )));
        let update = SessionUpdate::UserMessageChunk(chunk);
        service.save_update(session_id, update).await.unwrap();

        // Flush to ensure it's written
        service.flush_session(session_id).await.unwrap();

        // Load back
        let messages = service.load_messages(session_id).await.unwrap();

        assert_eq!(messages.len(), 1);
        assert!(matches!(
            messages[0].update,
            SessionUpdate::UserMessageChunk(_)
        ));
    }

    #[tokio::test]
    async fn test_load_nonexistent_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        let messages = service.load_messages("nonexistent-session").await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_load_malformed_jsonl_skips_bad_lines() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("bad-session.jsonl");

        // Write a file with one valid and one invalid line
        let valid_msg = PersistedMessage::new(SessionUpdate::UserMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new("valid".to_string())),
        )));
        let valid_json = serde_json::to_string(&valid_msg).unwrap();

        std::fs::write(&file_path, format!("{}\n{{invalid json\n", valid_json)).unwrap();

        let service = PersistenceService::new(temp_dir.path().to_path_buf());
        let messages = service.load_messages("bad-session").await.unwrap();

        // Should have parsed the valid line
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        let session_id = "to-delete";

        // Create a session file
        let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new("data".to_string())));
        service
            .save_update(session_id, SessionUpdate::UserMessageChunk(chunk))
            .await
            .unwrap();
        service.flush_session(session_id).await.unwrap();

        // Verify file exists
        let file_path = temp_dir.path().join(format!("{}.jsonl", session_id));
        assert!(file_path.exists());

        // Delete
        service.delete_session(session_id).await.unwrap();

        // File should be gone
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        // Create some session files
        for session_id in ["session-a", "session-b", "session-c"] {
            let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new("x".to_string())));
            service
                .save_update(session_id, SessionUpdate::UserMessageChunk(chunk))
                .await
                .unwrap();
            service.flush_session(session_id).await.unwrap();
        }

        let sessions = service.list_sessions().await.unwrap();

        assert_eq!(sessions.len(), 3);
        assert!(sessions.contains(&"session-a".to_string()));
        assert!(sessions.contains(&"session-b".to_string()));
        assert!(sessions.contains(&"session-c".to_string()));
    }

    #[tokio::test]
    async fn test_list_sessions_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        let sessions = service.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_flush_session_writes_remaining() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        let session_id = "flush-test";

        // Save multiple chunks (they get accumulated)
        for text in ["chunk1", "chunk2", "chunk3"] {
            let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(text.to_string())));
            service
                .save_update(session_id, SessionUpdate::AgentMessageChunk(chunk))
                .await
                .unwrap();
        }

        // Flush
        service.flush_session(session_id).await.unwrap();

        // Should have 1 merged message
        let messages = service.load_messages(session_id).await.unwrap();
        assert_eq!(messages.len(), 1);

        // Check merged content
        if let SessionUpdate::AgentMessageChunk(chunk) = &messages[0].update {
            if let ContentBlock::Text(text) = &chunk.content {
                assert_eq!(text.text, "chunk1chunk2chunk3");
            } else {
                panic!("Expected text content");
            }
        } else {
            panic!("Expected AgentMessageChunk");
        }
    }

    #[tokio::test]
    async fn test_save_multiple_sessions_isolation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = PersistenceService::new(temp_dir.path().to_path_buf());

        // Save to different sessions
        let chunk1 = ContentChunk::new(ContentBlock::Text(TextContent::new(
            "session1 data".to_string(),
        )));
        service
            .save_update("session-1", SessionUpdate::UserMessageChunk(chunk1))
            .await
            .unwrap();
        service.flush_session("session-1").await.unwrap();

        let chunk2 = ContentChunk::new(ContentBlock::Text(TextContent::new(
            "session2 data".to_string(),
        )));
        service
            .save_update("session-2", SessionUpdate::UserMessageChunk(chunk2))
            .await
            .unwrap();
        service.flush_session("session-2").await.unwrap();

        // Verify separate files
        let msg1 = service.load_messages("session-1").await.unwrap();
        let msg2 = service.load_messages("session-2").await.unwrap();

        assert_eq!(msg1.len(), 1);
        assert_eq!(msg2.len(), 1);

        // Verify content isolation
        if let SessionUpdate::UserMessageChunk(chunk) = &msg1[0].update {
            if let ContentBlock::Text(text) = &chunk.content {
                assert!(text.text.contains("session1"));
            }
        }

        if let SessionUpdate::UserMessageChunk(chunk) = &msg2[0].update {
            if let ContentBlock::Text(text) = &chunk.content {
                assert!(text.text.contains("session2"));
            }
        }
    }
}
