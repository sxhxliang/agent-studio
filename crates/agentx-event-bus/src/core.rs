//! Core Event Bus Implementation
//!
//! Provides a unified, type-safe event bus with advanced features:
//! - Subscription lifecycle management (subscribe/unsubscribe)
//! - Filtering capabilities
//! - Performance metrics
//! - Automatic cleanup

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// Unique identifier for event subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(usize);

impl SubscriptionId {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Statistics for event bus performance monitoring
#[derive(Debug, Clone, Default)]
pub struct EventBusStats {
    /// Total number of events published
    pub events_published: usize,
    /// Total number of events delivered to subscribers
    pub events_delivered: usize,
    /// Current number of active subscriptions
    pub active_subscriptions: usize,
    /// Total number of subscriptions created
    pub total_subscriptions: usize,
}

/// Subscriber callback with filtering support
struct Subscriber<T> {
    id: SubscriptionId,
    callback: Box<dyn Fn(&T) -> bool + Send + Sync>,
    filter: Option<Box<dyn Fn(&T) -> bool + Send + Sync>>,
}

impl<T> Subscriber<T> {
    fn should_notify(&self, event: &T) -> bool {
        match &self.filter {
            Some(filter) => filter(event),
            None => true,
        }
    }

    fn notify(&self, event: &T) -> bool {
        (self.callback)(event)
    }
}

/// Core event bus implementation with advanced features
pub struct EventBus<T> {
    subscribers: Vec<Subscriber<T>>,
    stats: EventBusStats,
}

impl<T> EventBus<T>
where
    T: Clone,
{
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            stats: EventBusStats::default(),
        }
    }

    /// Subscribe to events with an optional filter
    ///
    /// The callback receives an event and returns `true` to keep the subscription
    /// or `false` to automatically unsubscribe (one-shot behavior).
    ///
    /// The filter, if provided, determines whether the callback should be invoked.
    ///
    /// Returns a SubscriptionId that can be used to unsubscribe later.
    pub fn subscribe<F>(&mut self, callback: F) -> SubscriptionId
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let id = SubscriptionId::new();
        let subscriber = Subscriber {
            id,
            callback: Box::new(callback),
            filter: None,
        };

        self.subscribers.push(subscriber);
        self.stats.active_subscriptions += 1;
        self.stats.total_subscriptions += 1;

        log::trace!("[EventBus] New subscription: {:?}", id);
        id
    }

    /// Subscribe to events with a filter predicate
    ///
    /// Only events matching the filter will be delivered to the callback.
    pub fn subscribe_with_filter<F, P>(&mut self, callback: F, filter: Option<P>) -> SubscriptionId
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let id = SubscriptionId::new();
        let subscriber = Subscriber {
            id,
            callback: Box::new(callback),
            filter: filter.map(|f| Box::new(f) as Box<dyn Fn(&T) -> bool + Send + Sync>),
        };

        self.subscribers.push(subscriber);
        self.stats.active_subscriptions += 1;
        self.stats.total_subscriptions += 1;

        log::trace!("[EventBus] New subscription: {:?}", id);
        id
    }

    /// Subscribe to a single event (one-shot subscription)
    ///
    /// The subscription will be automatically removed after the first matching event.
    pub fn subscribe_once<F>(&mut self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&T) + Send + Sync + 'static,
    {
        let callback_cell = Arc::new(Mutex::new(Some(callback)));
        self.subscribe(move |event| {
            if let Some(cb) = callback_cell.lock().unwrap().take() {
                cb(event);
                false // Unsubscribe after first invocation
            } else {
                false
            }
        })
    }

    /// Unsubscribe using a subscription ID
    ///
    /// Returns true if the subscription was found and removed.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        if let Some(pos) = self.subscribers.iter().position(|s| s.id == id) {
            self.subscribers.remove(pos);
            self.stats.active_subscriptions = self.stats.active_subscriptions.saturating_sub(1);
            log::trace!("[EventBus] Unsubscribed: {:?}", id);
            true
        } else {
            log::warn!("[EventBus] Subscription not found: {:?}", id);
            false
        }
    }

    /// Publish an event to all subscribers
    ///
    /// Automatically removes one-shot subscribers that return false.
    pub fn publish(&mut self, event: T) {
        self.stats.events_published += 1;

        let mut to_remove = Vec::new();

        for subscriber in &self.subscribers {
            if subscriber.should_notify(&event) {
                self.stats.events_delivered += 1;

                // If callback returns false, mark for removal
                if !subscriber.notify(&event) {
                    to_remove.push(subscriber.id);
                }
            }
        }

        // Remove one-shot subscribers
        for id in to_remove {
            self.unsubscribe(id);
        }

        log::trace!(
            "[EventBus] Published event to {} subscribers",
            self.subscribers.len()
        );
    }

    /// Get current statistics
    pub fn stats(&self) -> EventBusStats {
        self.stats.clone()
    }

    /// Get the number of active subscriptions
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// Clear all subscriptions
    pub fn clear(&mut self) {
        let count = self.subscribers.len();
        self.subscribers.clear();
        self.stats.active_subscriptions = 0;
        log::info!("[EventBus] Cleared {} subscriptions", count);
    }
}

impl<T> Default for EventBus<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe container for EventBus
#[derive(Clone)]
pub struct EventBusContainer<T> {
    inner: Arc<Mutex<EventBus<T>>>,
}

impl<T> EventBusContainer<T>
where
    T: Clone,
{
    /// Create a new event bus container
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventBus::new())),
        }
    }

    /// Subscribe to events
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let mut bus = self.inner.lock().unwrap();
        bus.subscribe(callback)
    }

    /// Subscribe with a filter predicate
    pub fn subscribe_with_filter<F, P>(&self, callback: F, filter: P) -> SubscriptionId
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
        P: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let mut bus = self.inner.lock().unwrap();
        bus.subscribe_with_filter(callback, Some(filter))
    }

    /// Subscribe to a single event (one-shot)
    pub fn subscribe_once<F>(&self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&T) + Send + Sync + 'static,
    {
        let mut bus = self.inner.lock().unwrap();
        bus.subscribe_once(callback)
    }

    /// Unsubscribe using a subscription ID
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        let mut bus = self.inner.lock().unwrap();
        bus.unsubscribe(id)
    }

    /// Publish an event
    pub fn publish(&self, event: T) {
        let mut bus = self.inner.lock().unwrap();
        bus.publish(event);
    }

    /// Get current statistics
    pub fn stats(&self) -> EventBusStats {
        let bus = self.inner.lock().unwrap();
        bus.stats()
    }

    /// Get subscriber count
    pub fn subscriber_count(&self) -> usize {
        let bus = self.inner.lock().unwrap();
        bus.subscriber_count()
    }

    /// Clear all subscriptions
    pub fn clear(&self) {
        let mut bus = self.inner.lock().unwrap();
        bus.clear();
    }
}

impl<T> Default for EventBusContainer<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Debug, PartialEq)]
    struct TestEvent {
        id: usize,
        message: String,
    }

    #[test]
    fn test_subscribe_and_publish() {
        let bus = EventBusContainer::new();
        let received = Arc::new(Mutex::new(Vec::new()));

        let received_clone = received.clone();
        bus.subscribe(move |event: &TestEvent| {
            received_clone.lock().unwrap().push(event.clone());
            true // Keep subscription
        });

        bus.publish(TestEvent {
            id: 1,
            message: "test".to_string(),
        });

        assert_eq!(received.lock().unwrap().len(), 1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_subscribe_with_filter() {
        let bus = EventBusContainer::new();
        let received = Arc::new(AtomicUsize::new(0));

        let received_clone = received.clone();
        bus.subscribe_with_filter(
            move |_: &TestEvent| {
                received_clone.fetch_add(1, Ordering::SeqCst);
                true
            },
            |event: &TestEvent| event.id > 5,
        );

        // Should be filtered out
        bus.publish(TestEvent {
            id: 3,
            message: "filtered".to_string(),
        });

        // Should pass filter
        bus.publish(TestEvent {
            id: 10,
            message: "passed".to_string(),
        });

        assert_eq!(received.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_once() {
        let bus = EventBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_once(move |_: &TestEvent| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.publish(TestEvent {
            id: 1,
            message: "first".to_string(),
        });
        bus.publish(TestEvent {
            id: 2,
            message: "second".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = EventBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        let sub_id = bus.subscribe(move |_: &TestEvent| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            true
        });

        bus.publish(TestEvent {
            id: 1,
            message: "first".to_string(),
        });

        assert!(bus.unsubscribe(sub_id));

        bus.publish(TestEvent {
            id: 2,
            message: "second".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_one_shot_callback() {
        let bus = EventBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe(move |_: &TestEvent| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            false // Unsubscribe after first call
        });

        bus.publish(TestEvent {
            id: 1,
            message: "first".to_string(),
        });
        bus.publish(TestEvent {
            id: 2,
            message: "second".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_stats() {
        let bus = EventBusContainer::new();

        bus.subscribe(|_: &TestEvent| true);
        bus.subscribe(|_: &TestEvent| true);

        bus.publish(TestEvent {
            id: 1,
            message: "test".to_string(),
        });

        let stats = bus.stats();
        assert_eq!(stats.events_published, 1);
        assert_eq!(stats.events_delivered, 2);
        assert_eq!(stats.active_subscriptions, 2);
    }

    #[test]
    fn test_clear() {
        let bus = EventBusContainer::new();

        bus.subscribe(|_: &TestEvent| true);
        bus.subscribe(|_: &TestEvent| true);

        assert_eq!(bus.subscriber_count(), 2);

        bus.clear();

        assert_eq!(bus.subscriber_count(), 0);
    }
}
