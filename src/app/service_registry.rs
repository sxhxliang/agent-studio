use std::sync::Arc;

use crate::core::{
    event_bus::EventHub,
    services::{
        AgentConfigService, AgentService, AiService, MessageService, PersistenceService,
        WorkspaceService,
    },
};

/// A clonable container for all application services.
///
/// `ServiceRegistry` can be cheaply cloned and captured in async closures,
/// removing the need to access the GPUI global `AppState` from background tasks.
#[derive(Clone)]
pub struct ServiceRegistry {
    pub event_hub: EventHub,
    agent_service: Option<Arc<AgentService>>,
    message_service: Option<Arc<MessageService>>,
    persistence_service: Option<Arc<PersistenceService>>,
    workspace_service: Option<Arc<WorkspaceService>>,
    agent_config_service: Option<Arc<AgentConfigService>>,
    ai_service: Option<Arc<AiService>>,
}

impl ServiceRegistry {
    pub fn new(event_hub: EventHub) -> Self {
        Self {
            event_hub,
            agent_service: None,
            message_service: None,
            persistence_service: None,
            workspace_service: None,
            agent_config_service: None,
            ai_service: None,
        }
    }

    pub fn agent_service(&self) -> anyhow::Result<&Arc<AgentService>> {
        self.agent_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AgentService not initialized"))
    }

    pub fn message_service(&self) -> anyhow::Result<&Arc<MessageService>> {
        self.message_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MessageService not initialized"))
    }

    pub fn persistence_service(&self) -> anyhow::Result<&Arc<PersistenceService>> {
        self.persistence_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("PersistenceService not initialized"))
    }

    pub fn workspace_service(&self) -> anyhow::Result<&Arc<WorkspaceService>> {
        self.workspace_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("WorkspaceService not initialized"))
    }

    pub fn agent_config_service(&self) -> anyhow::Result<&Arc<AgentConfigService>> {
        self.agent_config_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AgentConfigService not initialized"))
    }

    pub fn ai_service(&self) -> anyhow::Result<&Arc<AiService>> {
        self.ai_service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("AiService not initialized"))
    }

    // --- Setters (used by AppState during initialization) ---

    pub(crate) fn set_agent_service(&mut self, service: Arc<AgentService>) {
        self.agent_service = Some(service);
    }

    pub(crate) fn set_message_service(&mut self, service: Arc<MessageService>) {
        self.message_service = Some(service);
    }

    pub(crate) fn set_persistence_service(&mut self, service: Arc<PersistenceService>) {
        self.persistence_service = Some(service);
    }

    pub(crate) fn set_workspace_service(&mut self, service: Arc<WorkspaceService>) {
        self.workspace_service = Some(service);
    }

    pub(crate) fn set_agent_config_service(&mut self, service: Arc<AgentConfigService>) {
        self.agent_config_service = Some(service);
    }

    pub(crate) fn set_ai_service(&mut self, service: Arc<AiService>) {
        self.ai_service = Some(service);
    }
}
