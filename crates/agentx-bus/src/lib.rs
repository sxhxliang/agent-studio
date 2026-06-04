//! # agentx-bus — typed asynchronous event bus (infrastructure)
//!
//! A generic publish/subscribe bus: `publish::<E>(event)` and
//! `subscribe::<E>() -> Receiver<E>`, routed by [`std::any::TypeId`] so each
//! event type gets its own channel.
//!
//! It replaces the legacy hub's ~20 hand-written `subscribe_*` methods and its
//! synchronous-while-locked dispatch (a deadlock hazard): publishing hands the
//! event to a channel and returns immediately; subscribers receive on their own
//! tasks.
//!
//! ## Dependency rule
//! Standalone infrastructure depending only on `tokio` for channels. It is
//! generic over the event type and knows nothing about the domain. Event
//! *types* live in `agentx-domain`; the bus only moves them around.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

pub use tokio::sync::broadcast::Receiver;
pub use tokio::sync::broadcast::error::{RecvError, TryRecvError};

/// Marker for anything that can travel on the bus. Blanket-implemented, so any
/// `Clone + Send + Sync + 'static` value qualifies — usually
/// `agentx_domain::DomainEvent`.
pub trait Event: Clone + Send + Sync + 'static {}
impl<T: Clone + Send + Sync + 'static> Event for T {}

/// A cheap-to-clone handle to the event bus.
///
/// Each event type `E` gets its own [`broadcast`] channel, looked up by
/// `TypeId`. Publishing never blocks on subscriber work: it pushes onto the
/// channel and returns, so a subscriber that publishes while handling an event
/// cannot deadlock the publisher (the central flaw in the legacy hub).
#[derive(Clone)]
pub struct EventBus {
    inner: Arc<Inner>,
}

struct Inner {
    capacity: usize,
    channels: Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl EventBus {
    /// Create a bus with the default per-type channel capacity (1024).
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    /// Create a bus where each event type buffers up to `capacity` events for
    /// slow subscribers before they begin observing [`RecvError::Lagged`].
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                capacity,
                channels: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Publish an event to every current subscriber of type `E`. With no
    /// subscribers the event is simply dropped.
    pub fn publish<E: Event>(&self, event: E) {
        let _ = self.sender::<E>().send(event);
    }

    /// Subscribe to events of type `E`. Only events published *after* this call
    /// are delivered.
    pub fn subscribe<E: Event>(&self) -> Receiver<E> {
        self.sender::<E>().subscribe()
    }

    fn sender<E: Event>(&self) -> broadcast::Sender<E> {
        let mut channels = self.inner.channels.lock().expect("event bus mutex poisoned");
        let entry = channels
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(broadcast::channel::<E>(self.inner.capacity).0));
        entry
            .downcast_ref::<broadcast::Sender<E>>()
            .expect("a TypeId maps to exactly one sender type")
            .clone()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Ping(u32);
    #[derive(Clone, Debug, PartialEq)]
    struct Pong(&'static str);

    #[tokio::test]
    async fn subscriber_receives_published_event() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe::<Ping>();
        bus.publish(Ping(7));
        assert_eq!(rx.recv().await.unwrap(), Ping(7));
    }

    #[tokio::test]
    async fn event_types_are_isolated() {
        let bus = EventBus::new();
        let mut pings = bus.subscribe::<Ping>();
        let mut pongs = bus.subscribe::<Pong>();
        bus.publish(Pong("hi"));
        assert_eq!(pongs.recv().await.unwrap(), Pong("hi"));
        // The Ping channel saw nothing.
        assert!(matches!(pings.try_recv(), Err(TryRecvError::Empty)));
    }

    #[tokio::test]
    async fn every_subscriber_receives_each_event() {
        let bus = EventBus::new();
        let mut a = bus.subscribe::<Ping>();
        let mut b = bus.subscribe::<Ping>();
        bus.publish(Ping(1));
        assert_eq!(a.recv().await.unwrap(), Ping(1));
        assert_eq!(b.recv().await.unwrap(), Ping(1));
    }

    #[test]
    fn publishing_without_subscribers_does_not_panic() {
        EventBus::new().publish(Ping(99));
    }
}
