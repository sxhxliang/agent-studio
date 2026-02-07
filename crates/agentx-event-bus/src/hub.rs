use crate::core::{EventBusContainer, EventBusStats, SubscriptionId};
use agentx_types::{
    AgentConfigEvent, CodeSelectionEvent, Config, PermissionRequestEvent, SessionStatus,
    SessionUpdateEvent, WorkspaceUpdateEvent,
};

#[derive(Clone, Debug)]
pub enum AppEvent {
    AgentConfig(AgentConfigEvent),
    CodeSelection(CodeSelectionEvent),
    PermissionRequest(Box<PermissionRequestEvent>),
    SessionUpdate(SessionUpdateEvent),
    WorkspaceUpdate(WorkspaceUpdateEvent),
}

#[derive(Clone)]
pub struct EventHub {
    bus: EventBusContainer<AppEvent>,
}

impl EventHub {
    pub fn new() -> Self {
        Self {
            bus: EventBusContainer::new(),
        }
    }

    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AppEvent) -> bool + Send + Sync + 'static,
    {
        self.bus.subscribe(callback)
    }

    pub fn subscribe_with_filter<F, P>(&self, callback: F, filter: P) -> SubscriptionId
    where
        F: Fn(&AppEvent) -> bool + Send + Sync + 'static,
        P: Fn(&AppEvent) -> bool + Send + Sync + 'static,
    {
        self.bus.subscribe_with_filter(callback, filter)
    }

    pub fn subscribe_once<F>(&self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&AppEvent) + Send + Sync + 'static,
    {
        self.bus.subscribe_once(callback)
    }

    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.bus.unsubscribe(id)
    }

    pub fn publish(&self, event: AppEvent) {
        self.bus.publish(event);
    }

    pub fn stats(&self) -> EventBusStats {
        self.bus.stats()
    }

    pub fn subscriber_count(&self) -> usize {
        self.bus.subscriber_count()
    }

    pub fn clear(&self) {
        self.bus.clear();
    }

    pub fn subscribe_session_updates<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::SessionUpdate(event) = event {
                    callback(event);
                }
                true
            },
            |event| matches!(event, AppEvent::SessionUpdate(_)),
        )
    }

    pub fn subscribe_session_updates_for_session<F>(
        &self,
        session_id: String,
        callback: F,
    ) -> SubscriptionId
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::SessionUpdate(event) = event {
                    callback(event);
                }
                true
            },
            move |event| {
                matches!(
                    event,
                    AppEvent::SessionUpdate(event) if event.session_id == session_id
                )
            },
        )
    }

    pub fn subscribe_session_updates_for_agent<F>(
        &self,
        agent_name: String,
        callback: F,
    ) -> SubscriptionId
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::SessionUpdate(event) = event {
                    callback(event);
                }
                true
            },
            move |event| {
                matches!(
                    event,
                    AppEvent::SessionUpdate(event)
                        if event
                            .agent_name
                            .as_ref()
                            .map(|name| name == &agent_name)
                            .unwrap_or(false)
                )
            },
        )
    }

    pub fn subscribe_permission_requests<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::PermissionRequest(event) = event {
                    callback(event.as_ref());
                }
                true
            },
            |event| matches!(event, AppEvent::PermissionRequest(_)),
        )
    }

    pub fn subscribe_permission_requests_for_session<F>(
        &self,
        session_id: String,
        callback: F,
    ) -> SubscriptionId
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::PermissionRequest(event) = event {
                    callback(event.as_ref());
                }
                true
            },
            move |event| {
                matches!(
                    event,
                    AppEvent::PermissionRequest(event) if event.session_id == session_id
                )
            },
        )
    }

    pub fn subscribe_permission_requests_for_agent<F>(
        &self,
        agent_name: String,
        callback: F,
    ) -> SubscriptionId
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::PermissionRequest(event) = event {
                    callback(event.as_ref());
                }
                true
            },
            move |event| {
                matches!(
                    event,
                    AppEvent::PermissionRequest(event) if event.agent_name == agent_name
                )
            },
        )
    }

    pub fn subscribe_workspace_updates<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::WorkspaceUpdate(event) = event {
                    callback(event);
                }
                true
            },
            |event| matches!(event, AppEvent::WorkspaceUpdate(_)),
        )
    }

    pub fn subscribe_workspace_updates_for_workspace<F>(
        &self,
        workspace_id: String,
        callback: F,
    ) -> SubscriptionId
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::WorkspaceUpdate(event) = event {
                    callback(event);
                }
                true
            },
            move |event| {
                matches!(
                    event,
                    AppEvent::WorkspaceUpdate(
                        WorkspaceUpdateEvent::TaskCreated { workspace_id: wid, .. }
                            | WorkspaceUpdateEvent::TaskRemoved { workspace_id: wid, .. }
                            | WorkspaceUpdateEvent::WorkspaceAdded { workspace_id: wid }
                            | WorkspaceUpdateEvent::WorkspaceRemoved { workspace_id: wid }
                    ) if wid == &workspace_id
                )
            },
        )
    }

    pub fn subscribe_workspace_session_status<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&String, &SessionStatus) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::WorkspaceUpdate(WorkspaceUpdateEvent::SessionStatusUpdated {
                    session_id,
                    status,
                    ..
                }) = event
                {
                    callback(session_id, status);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::WorkspaceUpdate(WorkspaceUpdateEvent::SessionStatusUpdated { .. })
                )
            },
        )
    }

    pub fn subscribe_workspace_task_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::WorkspaceUpdate(event) = event {
                    callback(event);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::WorkspaceUpdate(
                        WorkspaceUpdateEvent::TaskCreated { .. }
                            | WorkspaceUpdateEvent::TaskUpdated { .. }
                            | WorkspaceUpdateEvent::TaskRemoved { .. }
                    )
                )
            },
        )
    }

    pub fn subscribe_agent_config_updates<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(event) = event {
                    callback(event);
                }
                true
            },
            |event| matches!(event, AppEvent::AgentConfig(_)),
        )
    }

    pub fn subscribe_agent_config_agent_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(event) = event {
                    callback(event);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::AgentConfig(
                        AgentConfigEvent::AgentAdded { .. }
                            | AgentConfigEvent::AgentUpdated { .. }
                            | AgentConfigEvent::AgentRemoved { .. }
                    )
                )
            },
        )
    }

    pub fn subscribe_agent_config_model_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(event) = event {
                    callback(event);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::AgentConfig(
                        AgentConfigEvent::ModelAdded { .. }
                            | AgentConfigEvent::ModelUpdated { .. }
                            | AgentConfigEvent::ModelRemoved { .. }
                    )
                )
            },
        )
    }

    pub fn subscribe_agent_config_mcp_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(event) = event {
                    callback(event);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::AgentConfig(
                        AgentConfigEvent::McpServerAdded { .. }
                            | AgentConfigEvent::McpServerUpdated { .. }
                            | AgentConfigEvent::McpServerRemoved { .. }
                    )
                )
            },
        )
    }

    pub fn subscribe_agent_config_command_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(event) = event {
                    callback(event);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::AgentConfig(
                        AgentConfigEvent::CommandAdded { .. }
                            | AgentConfigEvent::CommandUpdated { .. }
                            | AgentConfigEvent::CommandRemoved { .. }
                    )
                )
            },
        )
    }

    pub fn subscribe_agent_config_reloads<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&Config) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(AgentConfigEvent::ConfigReloaded { config }) = event {
                    callback(config);
                }
                true
            },
            |event| {
                matches!(
                    event,
                    AppEvent::AgentConfig(AgentConfigEvent::ConfigReloaded { .. })
                )
            },
        )
    }

    pub fn subscribe_agent_config_for_agent<F>(
        &self,
        agent_name: String,
        callback: F,
    ) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::AgentConfig(event) = event {
                    callback(event);
                }
                true
            },
            move |event| {
                matches!(
                    event,
                    AppEvent::AgentConfig(
                        AgentConfigEvent::AgentAdded { name, .. }
                            | AgentConfigEvent::AgentUpdated { name, .. }
                            | AgentConfigEvent::AgentRemoved { name }
                    ) if name == &agent_name
                )
            },
        )
    }

    pub fn subscribe_code_selections<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&CodeSelectionEvent) + Send + Sync + 'static,
    {
        self.subscribe_with_filter(
            move |event| {
                if let AppEvent::CodeSelection(event) = event {
                    callback(event);
                }
                true
            },
            |event| matches!(event, AppEvent::CodeSelection(_)),
        )
    }

    pub fn publish_session_update(&self, event: SessionUpdateEvent) {
        self.publish(AppEvent::SessionUpdate(event));
    }

    pub fn publish_permission_request(&self, event: PermissionRequestEvent) {
        self.publish(AppEvent::PermissionRequest(Box::new(event)));
    }

    pub fn publish_workspace_update(&self, event: WorkspaceUpdateEvent) {
        self.publish(AppEvent::WorkspaceUpdate(event));
    }

    pub fn publish_agent_config_update(&self, event: AgentConfigEvent) {
        self.publish(AppEvent::AgentConfig(event));
    }

    pub fn publish_code_selection(&self, event: CodeSelectionEvent) {
        self.publish(AppEvent::CodeSelection(event));
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentx_types::events::CodeSelectionData;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_publish_subscribe() {
        let hub = EventHub::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        hub.subscribe_code_selections(move |event| {
            received_clone
                .lock()
                .unwrap()
                .push(event.selection.file_path.clone());
        });

        hub.publish_code_selection(CodeSelectionEvent {
            selection: CodeSelectionData {
                file_path: "test.rs".to_string(),
                start_line: 1,
                start_column: 1,
                end_line: 10,
                end_column: 1,
                content: "test content".to_string(),
            },
        });

        assert_eq!(received.lock().unwrap().len(), 1);
    }
}
