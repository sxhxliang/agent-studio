//! Message Service - Handles message sending and event bus interaction
//!
//! This service provides a high-level API for sending messages and subscribing
//! to session updates. It orchestrates between AgentService and SessionBus.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use agent_client_protocol::{
    AvailableCommand, ContentBlock, ContentChunk, ImageContent, PromptResponse, SessionUpdate,
    TextContent,
};
use anyhow::{Result, anyhow};

use crate::core::event_bus::session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
use crate::core::event_bus::workspace_bus::{WorkspaceUpdateBusContainer, WorkspaceUpdateEvent};
use crate::core::services::SessionStatus;

use super::agent_service::AgentService;
use super::persistence_service::{PersistedMessage, PersistenceService};

/// Message service - handles message sending and event bus interaction
pub struct MessageService {
    session_bus: SessionUpdateBusContainer,
    agent_service: Arc<AgentService>,
    persistence_service: Arc<PersistenceService>,
    workspace_bus: WorkspaceUpdateBusContainer,
}

impl MessageService {
    pub fn new(
        session_bus: SessionUpdateBusContainer,
        agent_service: Arc<AgentService>,
        persistence_service: Arc<PersistenceService>,
        workspace_bus: WorkspaceUpdateBusContainer,
    ) -> Self {
        Self {
            session_bus,
            agent_service,
            persistence_service,
            workspace_bus,
        }
    }

    /// Initialize persistence subscription
    ///
    /// This should be called after the MessageService is created.
    /// Subscribes to both session_bus and workspace_bus events.
    pub fn init_persistence(&self) {
        let persistence_service = self.persistence_service.clone();
        let session_bus = self.session_bus.clone();
        let agent_service = self.agent_service.clone();
        let load_persist_policy: Arc<Mutex<HashMap<String, bool>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Subscribe to session bus for all session updates
        session_bus.subscribe(move |event| {
            let session_id = event.session_id.clone();
            let update = (*event.update).clone();
            let agent_name = event.agent_name.clone();
            let service = persistence_service.clone();
            let agent_svc = agent_service.clone();
            let load_policy = load_persist_policy.clone();
            let is_agent_event = agent_name.is_some();

            // Handle AvailableCommandsUpdate to store in AgentService
            if let SessionUpdate::AvailableCommandsUpdate(ref commands_update) = update {
                log::debug!(
                    "Received AvailableCommandsUpdate for session {}: {} commands",
                    session_id,
                    commands_update.available_commands.len()
                );

                // Get agent name for this session (prefer event metadata if available)
                let agent_name_for_update =
                    agent_name.clone().or_else(|| agent_svc.get_agent_for_session(&session_id));

                if let Some(agent_name) = agent_name_for_update {
                    agent_svc.update_session_commands(
                        &agent_name,
                        &session_id,
                        commands_update.available_commands.clone(),
                    );
                } else {
                    log::warn!(
                        "Could not find agent for session {} when processing AvailableCommandsUpdate",
                        session_id
                    );
                }
            }

            let is_loading = is_agent_event && agent_svc.is_session_loading(&session_id);
            let should_persist = if is_loading {
                let mut policy_map = load_policy.lock().unwrap();
                let entry = policy_map.entry(session_id.clone()).or_insert_with(|| {
                    !service.session_file_exists(&session_id)
                });
                *entry
            } else {
                if is_agent_event {
                    let mut policy_map = load_policy.lock().unwrap();
                    policy_map.remove(&session_id);
                }
                true
            };

            // Spawn async task using smol to save message
            if should_persist {
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
            } else {
                log::debug!(
                    "Skipping persistence for session {} (history already loaded)",
                    session_id
                );
            }
        });

        // Subscribe to workspace bus for session status changes
        let persistence_service_ws = self.persistence_service.clone();
        let workspace_bus = self.workspace_bus.clone();

        workspace_bus.subscribe(move |event| {
            if let WorkspaceUpdateEvent::SessionStatusUpdated {
                session_id, status, ..
            } = event
            {
                // Flush accumulator when session completes or becomes idle
                if matches!(status, SessionStatus::Completed | SessionStatus::Idle) {
                    let service = persistence_service_ws.clone();
                    let session_id = session_id.clone();

                    smol::spawn(async move {
                        if let Err(e) = service.flush_session(&session_id).await {
                            log::error!(
                                "Failed to flush session {} on status change: {}",
                                session_id,
                                e
                            );
                        }
                    })
                    .detach();
                }
            }
        });

        log::info!(
            "MessageService persistence subscriptions initialized (session_bus + workspace_bus)"
        );
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
        let result = self
            .agent_service
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
            agent_name: self.agent_service.get_agent_for_session(session_id),
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
            agent_name: self.agent_service.get_agent_for_session(session_id),
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
    /// Returns a channel receiver for session updates with metadata. If session_id
    /// is provided, only updates for that session will be received.
    pub fn subscribe_session_updates(
        &self,
        session_id: Option<String>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<SessionUpdateEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        self.session_bus.subscribe(move |event| {
            // Filter by session_id if specified
            if let Some(ref filter_id) = session_id {
                if &event.session_id != filter_id {
                    return;
                }
            }

            let _ = tx.send(event.clone());
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
    pub async fn list_workspace_sessions_with_history(&self) -> Result<Vec<String>> {
        self.persistence_service.list_workspace_sessions().await
    }

    /// Get available commands for a session
    ///
    /// Returns the list of available commands (slash commands, etc.) for a given session.
    /// Returns None if the session or agent is not found.
    pub fn get_session_commands(
        &self,
        agent_name: &str,
        session_id: &str,
    ) -> Option<Vec<AvailableCommand>> {
        self.agent_service
            .get_session_commands(agent_name, session_id)
    }

    /// Get available commands for a session by session_id only
    ///
    /// Automatically looks up the agent name for the session.
    /// Returns None if the session is not found.
    pub fn get_commands_by_session_id(&self, session_id: &str) -> Option<Vec<AvailableCommand>> {
        let agent_name = self.agent_service.get_agent_for_session(session_id)?;
        self.agent_service
            .get_session_commands(&agent_name, session_id)
    }
}
