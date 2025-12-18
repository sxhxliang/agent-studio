use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

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

/// Event bus for workspace updates
pub struct WorkspaceUpdateBus {
    subscribers: Vec<Box<dyn Fn(&WorkspaceUpdateEvent) + Send + 'static>>,
}

impl WorkspaceUpdateBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Subscribe to workspace update events
    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&WorkspaceUpdateEvent) + Send + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    /// Publish a workspace update event to all subscribers
    pub fn publish(&self, event: WorkspaceUpdateEvent) {
        log::debug!("[WorkspaceUpdateBus] Publishing event: {:?}", event);
        for subscriber in &self.subscribers {
            subscriber(&event);
        }
    }
}

/// Thread-safe container for WorkspaceUpdateBus
pub type WorkspaceUpdateBusContainer = Arc<Mutex<WorkspaceUpdateBus>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_bus() {
        let bus = Arc::new(Mutex::new(WorkspaceUpdateBus::new()));
        let received = Arc::new(Mutex::new(Vec::new()));

        // Subscribe
        let received_clone = received.clone();
        bus.lock().unwrap().subscribe(move |event| {
            received_clone.lock().unwrap().push(format!("{:?}", event));
        });

        // Publish
        let event = WorkspaceUpdateEvent::TaskCreated {
            workspace_id: "ws-1".to_string(),
            task_id: "task-1".to_string(),
        };
        bus.lock().unwrap().publish(event);

        // Verify
        assert_eq!(received.lock().unwrap().len(), 1);
    }
}
