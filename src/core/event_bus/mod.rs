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

// Specialized event buses
pub mod agent_config_bus;
pub mod code_selection_bus;
pub mod permission_bus;
pub mod session_bus;
pub mod workspace_bus;

// Re-export core types
pub use batching::{BatchedEventCollector, BatchedEvents, Debouncer, DebouncerContainer};
pub use core::{EventBus, EventBusContainer, EventBusStats, SubscriptionId};

// Re-export specialized event bus types
pub use agent_config_bus::{AgentConfigBusContainer, AgentConfigEvent};
pub use code_selection_bus::{
    CodeSelectionBusContainer, CodeSelectionEvent, subscribe_entity_to_code_selections,
};
pub use permission_bus::{PermissionBusContainer, PermissionRequestEvent};
pub use session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
pub use workspace_bus::{WorkspaceUpdateBusContainer, WorkspaceUpdateEvent};
