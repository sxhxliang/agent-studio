//! Event Bus System
//!
//! Provides a unified, type-safe event bus implementation with:
//! - Subscription lifecycle management (subscribe/unsubscribe)
//! - Advanced filtering capabilities
//! - Performance metrics and monitoring
//! - Event batching and debouncing
//! - Automatic cleanup for one-shot subscriptions

// Core event bus implementation
pub mod batching;
pub mod core;

// Event types and hub
pub mod events;
pub mod hub;

// Re-export core types
pub use batching::{BatchedEventCollector, BatchedEvents, Debouncer, DebouncerContainer};
pub use core::{EventBus, EventBusContainer, EventBusStats, SubscriptionId};

// Re-export event types + hub
pub use events::{
    AgentConfigEvent, CodeSelectionEvent, PermissionRequestEvent, SessionUpdateEvent,
    WorkspaceUpdateEvent,
};
pub use hub::{subscribe_entity_to_code_selections, AppEvent, EventHub};
