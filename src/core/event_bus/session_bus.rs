use agent_client_protocol::SessionUpdate;
use std::sync::Arc;

/// Session update event that can be broadcast to subscribers
#[derive(Clone, Debug)]
pub struct SessionUpdateEvent {
    pub session_id: String,
    pub update: Arc<SessionUpdate>,
}

/// Global event bus for session updates
pub struct SessionUpdateBus {
    subscribers: Vec<Box<dyn Fn(&SessionUpdateEvent) + Send + Sync>>,
}

impl SessionUpdateBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribe to session updates
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    /// Publish a session update to all subscribers
    pub fn publish(&self, event: SessionUpdateEvent) {
        for subscriber in &self.subscribers {
            subscriber(&event);
        }
    }
}

impl Default for SessionUpdateBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Global container for the session update bus
pub struct SessionUpdateBusContainer {
    inner: Arc<std::sync::Mutex<SessionUpdateBus>>,
}

impl SessionUpdateBusContainer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(SessionUpdateBus::new())),
        }
    }

    pub fn subscribe<F>(&self, callback: F)
    where
        F: Fn(&SessionUpdateEvent) + Send + Sync + 'static,
    {
        if let Ok(mut bus) = self.inner.lock() {
            bus.subscribe(callback);
        }
    }

    pub fn publish(&self, event: SessionUpdateEvent) {
        if let Ok(bus) = self.inner.lock() {
            bus.publish(event);
        }
    }

    pub fn clone_inner(&self) -> Arc<std::sync::Mutex<SessionUpdateBus>> {
        Arc::clone(&self.inner)
    }
}

impl Default for SessionUpdateBusContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SessionUpdateBusContainer {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
