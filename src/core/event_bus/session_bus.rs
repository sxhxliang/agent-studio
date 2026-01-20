//! Session Update Event Bus
//!
//! Provides a specialized event bus for session updates with filtering capabilities.

use agent_client_protocol::SessionUpdate;
use std::sync::Arc;

use super::core::{EventBusContainer, SubscriptionId};

/// Session update event that can be broadcast to subscribers
#[derive(Clone, Debug)]
pub struct SessionUpdateEvent {
    pub session_id: String,
    pub agent_name: Option<String>,
    pub update: Arc<SessionUpdate>,
}

/// Specialized container for session update events
///
/// Provides additional convenience methods for session-specific filtering.
#[derive(Clone)]
pub struct SessionUpdateBusContainer {
    inner: EventBusContainer<SessionUpdateEvent>,
}

impl SessionUpdateBusContainer {
    /// Create a new session update bus
    pub fn new() -> Self {
        Self {
            inner: EventBusContainer::new(),
        }
    }

    /// Subscribe to all session updates
    ///
    /// The callback should return `true` to keep the subscription active,
    /// or `false` to automatically unsubscribe (one-shot behavior).
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe(move |event| {
            callback(event);
            true // Keep subscription active
        })
    }

    /// Subscribe to updates for a specific session only
    ///
    /// Automatically filters events to only include the specified session_id.
    pub fn subscribe_session<F>(&self, session_id: String, callback: F) -> SubscriptionId
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            move |event| event.session_id == session_id,
        )
    }

    /// Subscribe to updates for a specific agent only
    ///
    /// Automatically filters events to only include the specified agent_name.
    pub fn subscribe_agent<F>(&self, agent_name: String, callback: F) -> SubscriptionId
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            move |event| {
                event
                    .agent_name
                    .as_ref()
                    .map(|name| name == &agent_name)
                    .unwrap_or(false)
            },
        )
    }

    /// Subscribe to a single session update (one-shot)
    ///
    /// The subscription will be automatically removed after the first event.
    pub fn subscribe_once<F>(&self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_once(callback)
    }

    /// Unsubscribe using a subscription ID
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.inner.unsubscribe(id)
    }

    /// Publish a session update to all subscribers
    pub fn publish(&self, event: SessionUpdateEvent) {
        log::trace!(
            "[SessionUpdateBus] Publishing event for session: {}",
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

    /// Get access to the underlying event bus (for advanced use cases)
    pub fn inner(&self) -> &EventBusContainer<SessionUpdateEvent> {
        &self.inner
    }
}

impl Default for SessionUpdateBusContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::ContentBlock;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_subscribe_and_publish() {
        let bus = SessionUpdateBusContainer::new();
        let received = Arc::new(Mutex::new(Vec::new()));

        let received_clone = received.clone();
        bus.subscribe(move |event| {
            received_clone
                .lock()
                .unwrap()
                .push(event.session_id.clone());
        });

        bus.publish(SessionUpdateEvent {
            session_id: "session-1".to_string(),
            agent_name: Some("agent-1".to_string()),
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        assert_eq!(received.lock().unwrap().len(), 1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_subscribe_session_filter() {
        let bus = SessionUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_session("session-1".to_string(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(SessionUpdateEvent {
            session_id: "session-2".to_string(),
            agent_name: None,
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        // Should pass filter
        bus.publish(SessionUpdateEvent {
            session_id: "session-1".to_string(),
            agent_name: None,
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_agent_filter() {
        let bus = SessionUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_agent("agent-1".to_string(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out (different agent)
        bus.publish(SessionUpdateEvent {
            session_id: "session-1".to_string(),
            agent_name: Some("agent-2".to_string()),
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        // Should be filtered out (no agent)
        bus.publish(SessionUpdateEvent {
            session_id: "session-2".to_string(),
            agent_name: None,
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        // Should pass filter
        bus.publish(SessionUpdateEvent {
            session_id: "session-3".to_string(),
            agent_name: Some("agent-1".to_string()),
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = SessionUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        let sub_id = bus.subscribe(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.publish(SessionUpdateEvent {
            session_id: "session-1".to_string(),
            agent_name: None,
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        assert!(bus.unsubscribe(sub_id));

        bus.publish(SessionUpdateEvent {
            session_id: "session-2".to_string(),
            agent_name: None,
            update: Arc::new(SessionUpdate::UserMessageChunk(
                agent_client_protocol::ContentChunk::new(ContentBlock::from("test".to_string())),
            )),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }
}
