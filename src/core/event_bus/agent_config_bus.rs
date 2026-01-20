//! Agent Configuration Event Bus
//!
//! Provides a publish-subscribe event bus for agent configuration changes with
//! advanced filtering and subscription management capabilities.

use std::sync::Arc;

use super::core::{EventBusContainer, SubscriptionId};
use crate::core::config::{
    AgentProcessConfig, CommandConfig, Config, McpServerConfig, ModelConfig,
};

/// Events published when agent configuration changes
#[derive(Clone, Debug)]
pub enum AgentConfigEvent {
    // ========== Agent Events ==========
    /// A new agent was added
    AgentAdded {
        name: String,
        config: AgentProcessConfig,
    },
    /// An existing agent's configuration was updated
    AgentUpdated {
        name: String,
        config: AgentProcessConfig,
    },
    /// An agent was removed
    AgentRemoved { name: String },

    // ========== Model Events ==========
    /// A new model was added
    ModelAdded { name: String, config: ModelConfig },
    /// An existing model's configuration was updated
    ModelUpdated { name: String, config: ModelConfig },
    /// A model was removed
    ModelRemoved { name: String },

    // ========== MCP Server Events ==========
    /// A new MCP server was added
    McpServerAdded {
        name: String,
        config: McpServerConfig,
    },
    /// An existing MCP server's configuration was updated
    McpServerUpdated {
        name: String,
        config: McpServerConfig,
    },
    /// An MCP server was removed
    McpServerRemoved { name: String },

    // ========== Command Events ==========
    /// A new command was added
    CommandAdded { name: String, config: CommandConfig },
    /// An existing command's configuration was updated
    CommandUpdated { name: String, config: CommandConfig },
    /// A command was removed
    CommandRemoved { name: String },

    // ========== Full Reload ==========
    /// The entire configuration was reloaded from file
    ConfigReloaded { config: Config },
}

/// Specialized container for agent config events
///
/// Provides additional convenience methods for config-specific filtering.
#[derive(Clone)]
pub struct AgentConfigBusContainer {
    inner: EventBusContainer<AgentConfigEvent>,
}

impl AgentConfigBusContainer {
    /// Create a new agent config bus
    pub fn new() -> Self {
        Self {
            inner: EventBusContainer::new(),
        }
    }

    /// Subscribe to all agent config events
    ///
    /// The callback should return `true` to keep the subscription active,
    /// or `false` to automatically unsubscribe (one-shot behavior).
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe(move |event| {
            callback(event);
            true // Keep subscription active
        })
    }

    /// Subscribe to agent-specific events only
    pub fn subscribe_agent_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            |event| {
                matches!(
                    event,
                    AgentConfigEvent::AgentAdded { .. }
                        | AgentConfigEvent::AgentUpdated { .. }
                        | AgentConfigEvent::AgentRemoved { .. }
                )
            },
        )
    }

    /// Subscribe to model-specific events only
    pub fn subscribe_model_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            |event| {
                matches!(
                    event,
                    AgentConfigEvent::ModelAdded { .. }
                        | AgentConfigEvent::ModelUpdated { .. }
                        | AgentConfigEvent::ModelRemoved { .. }
                )
            },
        )
    }

    /// Subscribe to MCP server events only
    pub fn subscribe_mcp_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            |event| {
                matches!(
                    event,
                    AgentConfigEvent::McpServerAdded { .. }
                        | AgentConfigEvent::McpServerUpdated { .. }
                        | AgentConfigEvent::McpServerRemoved { .. }
                )
            },
        )
    }

    /// Subscribe to command events only
    pub fn subscribe_command_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            |event| {
                matches!(
                    event,
                    AgentConfigEvent::CommandAdded { .. }
                        | AgentConfigEvent::CommandUpdated { .. }
                        | AgentConfigEvent::CommandRemoved { .. }
                )
            },
        )
    }

    /// Subscribe to config reload events only
    pub fn subscribe_config_reloads<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&Config) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                if let AgentConfigEvent::ConfigReloaded { config } = event {
                    callback(config);
                }
                true
            },
            |event| matches!(event, AgentConfigEvent::ConfigReloaded { .. }),
        )
    }

    /// Subscribe to events for a specific agent name
    pub fn subscribe_agent<F>(&self, agent_name: String, callback: F) -> SubscriptionId
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            move |event| match event {
                AgentConfigEvent::AgentAdded { name, .. }
                | AgentConfigEvent::AgentUpdated { name, .. }
                | AgentConfigEvent::AgentRemoved { name } => name == &agent_name,
                _ => false,
            },
        )
    }

    /// Subscribe to a single agent config event (one-shot)
    pub fn subscribe_once<F>(&self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_once(callback)
    }

    /// Unsubscribe using a subscription ID
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.inner.unsubscribe(id)
    }

    /// Publish an agent config event
    pub fn publish(&self, event: AgentConfigEvent) {
        log::trace!(
            "[AgentConfigBus] Publishing event to {} subscribers: {:?}",
            self.subscriber_count(),
            event
        );
        self.inner.publish(event);
    }

    /// Get the number of active subscriptions
    pub fn subscriber_count(&self) -> usize {
        self.inner.subscriber_count()
    }

    /// Get event bus statistics
    pub fn stats(&self) -> super::core::EventBusStats {
        self.inner.stats()
    }

    /// Clear all subscriptions
    pub fn clear(&self) {
        self.inner.clear();
    }
}

impl Default for AgentConfigBusContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_subscribe_and_publish() {
        let bus = AgentConfigBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe(move |event| match event {
            AgentConfigEvent::AgentAdded { name, .. } => {
                assert_eq!(name, "test-agent");
                count_clone.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        });

        let config = AgentProcessConfig {
            command: "test-command".to_string(),
            args: vec![],
            env: HashMap::new(),
            nodejs_path: None,
        };

        bus.publish(AgentConfigEvent::AgentAdded {
            name: "test-agent".to_string(),
            config,
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_subscribe_agent_events() {
        let bus = AgentConfigBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_agent_events(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should pass filter
        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "test".to_string(),
        });

        // Should be filtered out
        bus.publish(AgentConfigEvent::ModelRemoved {
            name: "test".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_specific_agent() {
        let bus = AgentConfigBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_agent("agent-1".to_string(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "agent-2".to_string(),
        });

        // Should pass filter
        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "agent-1".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_model_events() {
        let bus = AgentConfigBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_model_events(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "test".to_string(),
        });

        // Should pass filter
        bus.publish(AgentConfigEvent::ModelRemoved {
            name: "test".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = AgentConfigBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        let sub_id = bus.subscribe(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "test".to_string(),
        });

        assert!(bus.unsubscribe(sub_id));

        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "test2".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }
}
