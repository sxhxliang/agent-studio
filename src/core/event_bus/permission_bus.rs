//! Permission Request Event Bus
//!
//! Provides a specialized event bus for permission requests with filtering capabilities.

use agent_client_protocol as acp;
use std::sync::Arc;

use super::core::{EventBusContainer, SubscriptionId};

/// Permission request event that can be broadcast to subscribers
#[derive(Clone, Debug)]
pub struct PermissionRequestEvent {
    /// Unique permission request ID from PermissionStore
    pub permission_id: String,
    /// Session ID for this permission request
    pub session_id: String,
    /// Agent name requesting permission
    pub agent_name: String,
    /// Tool call details
    pub tool_call: acp::ToolCallUpdate,
    /// Available permission options
    pub options: Vec<acp::PermissionOption>,
}

/// Specialized container for permission request events
///
/// Provides additional convenience methods for permission-specific filtering.
#[derive(Clone)]
pub struct PermissionBusContainer {
    inner: EventBusContainer<PermissionRequestEvent>,
}

impl PermissionBusContainer {
    /// Create a new permission bus
    pub fn new() -> Self {
        Self {
            inner: EventBusContainer::new(),
        }
    }

    /// Subscribe to all permission requests
    ///
    /// The callback should return `true` to keep the subscription active,
    /// or `false` to automatically unsubscribe (one-shot behavior).
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe(move |event| {
            callback(event);
            true // Keep subscription active
        })
    }

    /// Subscribe to permission requests for a specific session only
    ///
    /// Automatically filters events to only include the specified session_id.
    pub fn subscribe_session<F>(&self, session_id: String, callback: F) -> SubscriptionId
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            move |event| event.session_id == session_id,
        )
    }

    /// Subscribe to permission requests for a specific agent only
    ///
    /// Automatically filters events to only include the specified agent_name.
    pub fn subscribe_agent<F>(&self, agent_name: String, callback: F) -> SubscriptionId
    where
        F: Fn(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            move |event| event.agent_name == agent_name,
        )
    }

    /// Subscribe to a single permission request (one-shot)
    ///
    /// The subscription will be automatically removed after the first event.
    pub fn subscribe_once<F>(&self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&PermissionRequestEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_once(callback)
    }

    /// Unsubscribe using a subscription ID
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.inner.unsubscribe(id)
    }

    /// Publish a permission request to all subscribers
    pub fn publish(&self, event: PermissionRequestEvent) {
        log::trace!(
            "[PermissionBus] Publishing permission request: {} for session: {}",
            event.permission_id,
            event.session_id
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

impl Default for PermissionBusContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn create_test_event(session_id: &str, agent_name: &str) -> PermissionRequestEvent {
        // Create a minimal ToolCallUpdate using the new method
        let tool_call = acp::ToolCallUpdate::new(
            acp::ToolCallId::from("tool-1".to_string()),
            acp::ToolCallUpdateFields::default(),
        );

        PermissionRequestEvent {
            permission_id: "perm-1".to_string(),
            session_id: session_id.to_string(),
            agent_name: agent_name.to_string(),
            tool_call,
            options: vec![],
        }
    }

    #[test]
    fn test_subscribe_and_publish() {
        let bus = PermissionBusContainer::new();
        let received = Arc::new(Mutex::new(Vec::new()));

        let received_clone = received.clone();
        bus.subscribe(move |event| {
            received_clone
                .lock()
                .unwrap()
                .push(event.permission_id.clone());
        });

        bus.publish(create_test_event("session-1", "agent-1"));

        assert_eq!(received.lock().unwrap().len(), 1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_subscribe_session_filter() {
        let bus = PermissionBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_session("session-1".to_string(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(create_test_event("session-2", "agent-1"));

        // Should pass filter
        bus.publish(create_test_event("session-1", "agent-1"));

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_agent_filter() {
        let bus = PermissionBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_agent("agent-1".to_string(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(create_test_event("session-1", "agent-2"));

        // Should pass filter
        bus.publish(create_test_event("session-1", "agent-1"));

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = PermissionBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        let sub_id = bus.subscribe(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.publish(create_test_event("session-1", "agent-1"));

        assert!(bus.unsubscribe(sub_id));

        bus.publish(create_test_event("session-2", "agent-1"));

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }
}
