//! Workspace Update Event Bus
//!
//! Provides a specialized event bus for workspace and task updates.

use chrono::{DateTime, Utc};
use std::sync::Arc;

use super::core::{EventBusContainer, SubscriptionId};
use crate::core::services::SessionStatus;

/// Workspace update events
#[derive(Clone, Debug)]
pub enum WorkspaceUpdateEvent {
    /// A new task was created
    TaskCreated {
        workspace_id: String,
        task_id: String,
    },
    /// A task was updated
    TaskUpdated { task_id: String },
    /// A task was removed
    TaskRemoved {
        workspace_id: String,
        task_id: String,
    },
    /// A new workspace was added
    WorkspaceAdded { workspace_id: String },
    /// A workspace was removed
    WorkspaceRemoved { workspace_id: String },
    /// A session status was updated
    SessionStatusUpdated {
        session_id: String,
        agent_name: String,
        status: SessionStatus,
        last_active: DateTime<Utc>,
        message_count: usize,
    },
}

/// Specialized container for workspace update events
///
/// Provides additional convenience methods for workspace-specific filtering.
#[derive(Clone)]
pub struct WorkspaceUpdateBusContainer {
    inner: EventBusContainer<WorkspaceUpdateEvent>,
}

impl WorkspaceUpdateBusContainer {
    /// Create a new workspace update bus
    pub fn new() -> Self {
        Self {
            inner: EventBusContainer::new(),
        }
    }

    /// Subscribe to all workspace updates
    ///
    /// The callback should return `true` to keep the subscription active,
    /// or `false` to automatically unsubscribe (one-shot behavior).
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe(move |event| {
            callback(event);
            true // Keep subscription active
        })
    }

    /// Subscribe to updates for a specific workspace only
    pub fn subscribe_workspace<F>(&self, workspace_id: String, callback: F) -> SubscriptionId
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            move |event| match event {
                WorkspaceUpdateEvent::TaskCreated {
                    workspace_id: wid, ..
                }
                | WorkspaceUpdateEvent::TaskRemoved {
                    workspace_id: wid, ..
                }
                | WorkspaceUpdateEvent::WorkspaceAdded { workspace_id: wid }
                | WorkspaceUpdateEvent::WorkspaceRemoved { workspace_id: wid } => {
                    wid == &workspace_id
                }
                _ => false,
            },
        )
    }

    /// Subscribe to session status updates only
    pub fn subscribe_session_status<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&String, &SessionStatus) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                if let WorkspaceUpdateEvent::SessionStatusUpdated {
                    session_id, status, ..
                } = event
                {
                    callback(session_id, status);
                }
                true
            },
            |event| matches!(event, WorkspaceUpdateEvent::SessionStatusUpdated { .. }),
        )
    }

    /// Subscribe to task events only
    pub fn subscribe_task_events<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_with_filter(
            move |event| {
                callback(event);
                true
            },
            |event| {
                matches!(
                    event,
                    WorkspaceUpdateEvent::TaskCreated { .. }
                        | WorkspaceUpdateEvent::TaskUpdated { .. }
                        | WorkspaceUpdateEvent::TaskRemoved { .. }
                )
            },
        )
    }

    /// Subscribe to a single workspace update (one-shot)
    pub fn subscribe_once<F>(&self, callback: F) -> SubscriptionId
    where
        F: FnOnce(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.inner.subscribe_once(callback)
    }

    /// Unsubscribe using a subscription ID
    pub fn unsubscribe(&self, id: SubscriptionId) -> bool {
        self.inner.unsubscribe(id)
    }

    /// Publish a workspace update to all subscribers
    pub fn publish(&self, event: WorkspaceUpdateEvent) {
        log::trace!("[WorkspaceUpdateBus] Publishing event: {:?}", event);
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

    /// Lock and access the bus directly (for backward compatibility)
    pub fn lock(
        &self,
    ) -> Result<WorkspaceUpdateBusLock, std::sync::PoisonError<WorkspaceUpdateBusLock>> {
        Ok(WorkspaceUpdateBusLock {
            container: self.clone(),
        })
    }
}

impl Default for WorkspaceUpdateBusContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Backward compatibility wrapper for lock-based access
pub struct WorkspaceUpdateBusLock {
    container: WorkspaceUpdateBusContainer,
}

impl WorkspaceUpdateBusLock {
    /// Subscribe to workspace updates (backward compatible API)
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + Sync + 'static,
    {
        self.container.subscribe(callback);
    }

    /// Publish a workspace update (backward compatible API)
    pub fn publish(&self, event: WorkspaceUpdateEvent) {
        self.container.publish(event);
    }
}

impl std::ops::Deref for WorkspaceUpdateBusLock {
    type Target = WorkspaceUpdateBusContainer;

    fn deref(&self) -> &Self::Target {
        &self.container
    }
}

// Make the lock wrapper compatible with unwrap() pattern
impl WorkspaceUpdateBusContainer {
    /// Create a lock wrapper (for backward compatibility with .lock().unwrap() pattern)
    pub fn lock_compat(
        &self,
    ) -> Result<WorkspaceUpdateBusLock, std::sync::PoisonError<WorkspaceUpdateBusLock>> {
        self.lock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_subscribe_and_publish() {
        let bus = WorkspaceUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.publish(WorkspaceUpdateEvent::WorkspaceAdded {
            workspace_id: "ws-1".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_subscribe_workspace_filter() {
        let bus = WorkspaceUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_workspace("ws-1".to_string(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(WorkspaceUpdateEvent::WorkspaceAdded {
            workspace_id: "ws-2".to_string(),
        });

        // Should pass filter
        bus.publish(WorkspaceUpdateEvent::TaskCreated {
            workspace_id: "ws-1".to_string(),
            task_id: "task-1".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_session_status() {
        let bus = WorkspaceUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_session_status(move |_, _| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should be filtered out
        bus.publish(WorkspaceUpdateEvent::WorkspaceAdded {
            workspace_id: "ws-1".to_string(),
        });

        // Should pass filter
        bus.publish(WorkspaceUpdateEvent::SessionStatusUpdated {
            session_id: "session-1".to_string(),
            agent_name: "agent-1".to_string(),
            status: SessionStatus::Active,
            last_active: Utc::now(),
            message_count: 0,
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscribe_task_events() {
        let bus = WorkspaceUpdateBusContainer::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count_clone = count.clone();
        bus.subscribe_task_events(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Should pass filter
        bus.publish(WorkspaceUpdateEvent::TaskCreated {
            workspace_id: "ws-1".to_string(),
            task_id: "task-1".to_string(),
        });

        // Should be filtered out
        bus.publish(WorkspaceUpdateEvent::WorkspaceAdded {
            workspace_id: "ws-1".to_string(),
        });

        // Should pass filter
        bus.publish(WorkspaceUpdateEvent::TaskUpdated {
            task_id: "task-1".to_string(),
        });

        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_backward_compatible_lock() {
        let bus = Arc::new(std::sync::Mutex::new(WorkspaceUpdateBusContainer::new()));
        let count = Arc::new(AtomicUsize::new(0));

        {
            let count_clone = count.clone();
            let lock = bus.lock().unwrap();
            lock.subscribe(move |_| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        {
            let lock = bus.lock().unwrap();
            lock.publish(WorkspaceUpdateEvent::WorkspaceAdded {
                workspace_id: "ws-1".to_string(),
            });
        }

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
