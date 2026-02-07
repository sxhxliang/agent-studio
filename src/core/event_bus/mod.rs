//! Event Bus System
//!
//! Re-exports from agentx-event-bus crate, plus GPUI-specific helpers.

// Re-export everything from the agentx-event-bus crate
pub use agentx_event_bus::batching::{
    BatchedEventCollector, BatchedEvents, Debouncer, DebouncerContainer,
};
pub use agentx_event_bus::core::{EventBus, EventBusContainer, EventBusStats, SubscriptionId};
pub use agentx_event_bus::hub::{AppEvent, EventHub};
pub use agentx_event_bus::{
    AgentConfigEvent, CodeSelectionEvent, PermissionRequestEvent, SessionUpdateEvent,
    WorkspaceUpdateEvent,
};

// GPUI-specific helpers that depend on gpui types
mod code_selection_helper;
pub use code_selection_helper::subscribe_entity_to_code_selections;
