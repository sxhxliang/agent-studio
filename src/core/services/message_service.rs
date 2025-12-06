//! Message Service - Handles message sending and event bus interaction
//!
//! This service provides a high-level API for sending messages and subscribing
//! to session updates. It orchestrates between AgentService and SessionBus.

use std::sync::Arc;

use agent_client_protocol::{ContentBlock, ContentChunk, ImageContent, PromptResponse, SessionUpdate, TextContent};
use anyhow::{anyhow, Result};

use crate::core::event_bus::session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};

use super::agent_service::AgentService;
use super::persistence_service::{PersistedMessage, PersistenceService};

/// Message service - handles message sending and event bus interaction
pub struct MessageService {
    session_bus: SessionUpdateBusContainer,
    agent_service: Arc<AgentService>,
    persistence_service: Arc<PersistenceService>,
}

impl MessageService {
    pub fn new(
        session_bus: SessionUpdateBusContainer,
        agent_service: Arc<AgentService>,
        persistence_service: Arc<PersistenceService>,
    ) -> Self {
        Self {
            session_bus,
            agent_service,
            persistence_service,
        }
    }

    /// Initialize persistence subscription
    ///
    /// This should be called after the MessageService is created
    pub fn init_persistence(&self) {
        let persistence_service = self.persistence_service.clone();
        let session_bus = self.session_bus.clone();

        session_bus.subscribe(move |event| {
            let session_id = event.session_id.clone();
            let update = (*event.update).clone();
            let service = persistence_service.clone();

            // Spawn async task using smol to save message
            smol::spawn(async move {
                if let Err(e) = service.save_update(&session_id, update).await {
                    log::error!(
                        "Failed to persist message for session {}: {}",
                        session_id,
                        e
                    );
                }
            })
            .detach();
        });

        log::info!("MessageService persistence subscription initialized");
    }

    /// Send a user message to an existing session
    ///
    /// This method performs the following steps:
    /// 1. Verify the session exists
    /// 2. Publish the user message to the event bus (immediate UI feedback)
    /// 3. Send the prompt to the agent
    ///
    /// Use this when you already have a session ID and want to ensure
    /// the UI panel has subscribed before the message is sent.
    pub async fn send_message_to_session(
        &self,
        agent_name: &str,
        session_id: &str,
        content_blocks: Vec<ContentBlock>,
    ) -> Result<PromptResponse> {
        // 1. Verify session exists
        if self
            .agent_service
            .get_session_info(agent_name, session_id)
            .is_none()
        {
            return Err(anyhow!("Session not found: {}", session_id));
        }

        // 2. Publish user message blocks to event bus (immediate UI feedback)
        for block in &content_blocks {
            self.publish_user_content_block(session_id, block);
        }

        // 3. Send prompt to agent
        let result =  self.agent_service
            .send_prompt(agent_name, session_id, content_blocks)
            .await
            .map_err(|e| anyhow!("Failed to send message: {}", e))?;

        Ok(result)
    }

    /// Publish a user message to the event bus (immediate UI feedback)
    pub fn publish_user_message(&self, session_id: &str, message: &str) {
        let content_block = ContentBlock::from(message.to_string());
        let content_chunk = ContentChunk::new(content_block);

        let user_event = SessionUpdateEvent {
            session_id: session_id.to_string(),
            update: Arc::new(SessionUpdate::UserMessageChunk(content_chunk)),
        };

        self.session_bus.publish(user_event);
        log::debug!("Published user message to session bus: {}", session_id);
    }

    /// Publish a user content block to the event bus
    ///
    /// Converts protocol ContentBlock to schema ContentBlock and publishes to the event bus
    pub fn publish_user_content_block(&self, session_id: &str, block: &ContentBlock) {
        // Convert protocol ContentBlock to schema ContentBlock
        let schema_block = match block {
            ContentBlock::Text(text) => {
                // Create TextContent using new() or default methods to handle non-exhaustive struct
                let text_content = TextContent::new(text.text.clone());
                // Note: annotations field might not be directly settable due to version mismatch
                ContentBlock::Text(text_content)
            }
            ContentBlock::Image(img) => {
                // Create ImageContent using new() method
                let image_content = ImageContent::new(img.data.clone(), img.mime_type.clone());
                ContentBlock::Image(image_content)
            }
            // Handle other block types if needed
            _ => return,
        };

        let content_chunk = ContentChunk::new(schema_block);
        let user_event = SessionUpdateEvent {
            session_id: session_id.to_string(),
            update: Arc::new(SessionUpdate::UserMessageChunk(content_chunk)),
        };

        self.session_bus.publish(user_event);
        log::debug!(
            "Published user content block to session bus: {}",
            session_id
        );
    }

    /// Subscribe to session updates
    ///
    /// Returns a channel receiver for session updates. If session_id is provided,
    /// only updates for that session will be received.
    pub fn subscribe_session_updates(
        &self,
        session_id: Option<String>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<SessionUpdate> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        self.session_bus.subscribe(move |event| {
            // Filter by session_id if specified
            if let Some(ref filter_id) = session_id {
                if &event.session_id != filter_id {
                    return;
                }
            }

            let _ = tx.send((*event.update).clone());
        });

        rx
    }

    /// Load historical messages for a session
    ///
    /// Returns all persisted messages in chronological order
    pub async fn load_history(&self, session_id: &str) -> Result<Vec<PersistedMessage>> {
        self.persistence_service.load_messages(session_id).await
    }

    /// Delete a session's history
    pub async fn delete_history(&self, session_id: &str) -> Result<()> {
        self.persistence_service.delete_session(session_id).await
    }

    /// List all available sessions with history
    pub async fn list_sessions_with_history(&self) -> Result<Vec<String>> {
        self.persistence_service.list_sessions().await
    }
}
