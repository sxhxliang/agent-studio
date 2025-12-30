//! Agent Configuration Event Bus
//!
//! This module provides a publish-subscribe event bus for agent configuration changes.
//! It allows components to subscribe to configuration updates and react to changes
//! such as agent additions, updates, removals, or full config reloads.

use std::sync::{Arc, Mutex};

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

/// Callback function type for agent config events
type AgentConfigCallback = Arc<dyn Fn(&AgentConfigEvent) + Send + Sync>;

/// Event bus for agent configuration changes
pub struct AgentConfigBus {
    subscribers: Vec<AgentConfigCallback>,
}

impl AgentConfigBus {
    /// Create a new agent config bus
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribe to agent config events
    ///
    /// The callback will be invoked whenever an agent config event is published.
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        self.subscribers.push(Arc::new(callback));
    }

    /// Publish an agent config event to all subscribers
    pub fn publish(&self, event: AgentConfigEvent) {
        log::debug!(
            "[AgentConfigBus] Publishing event to {} subscribers: {:?}",
            self.subscribers.len(),
            event
        );

        for callback in &self.subscribers {
            callback(&event);
        }
    }
}

/// Thread-safe container for AgentConfigBus
#[derive(Clone)]
pub struct AgentConfigBusContainer {
    inner: Arc<Mutex<AgentConfigBus>>,
}

impl AgentConfigBusContainer {
    /// Create a new agent config bus container
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AgentConfigBus::new())),
        }
    }

    /// Subscribe to agent config events
    pub fn subscribe<F>(&self, callback: F)
    where
        F: Fn(&AgentConfigEvent) + Send + Sync + 'static,
    {
        let mut bus = self.inner.lock().unwrap();
        bus.subscribe(callback);
    }

    /// Publish an agent config event
    pub fn publish(&self, event: AgentConfigEvent) {
        let bus = self.inner.lock().unwrap();
        bus.publish(event);
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_agent_config_bus_subscribe_and_publish() {
        let bus = AgentConfigBusContainer::new();
        let call_count = Arc::new(AtomicUsize::new(0));

        let count_clone = call_count.clone();
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
        };

        bus.publish(AgentConfigEvent::AgentAdded {
            name: "test-agent".to_string(),
            config,
        });

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = AgentConfigBusContainer::new();
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));

        let c1 = count1.clone();
        bus.subscribe(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        });

        let c2 = count2.clone();
        bus.subscribe(move |_| {
            c2.fetch_add(1, Ordering::SeqCst);
        });

        let config = AgentProcessConfig {
            command: "test".to_string(),
            args: vec![],
            env: HashMap::new(),
        };

        bus.publish(AgentConfigEvent::AgentRemoved {
            name: "test".to_string(),
        });

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }
}
