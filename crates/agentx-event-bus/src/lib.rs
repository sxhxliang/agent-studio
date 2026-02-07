pub mod batching;
pub mod core;
pub mod hub;

pub use core::{EventBus, EventBusContainer, EventBusStats, SubscriptionId};
pub use hub::{AppEvent, EventHub};

// Re-export types for convenience
pub use agentx_types::{
    AgentConfigEvent, CodeSelectionEvent, PermissionRequestEvent, SessionUpdateEvent,
    WorkspaceUpdateEvent,
};
