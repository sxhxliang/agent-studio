// Event bus modules
pub mod agent_config_bus;
pub mod code_selection_bus;
pub mod permission_bus;
pub mod session_bus;
pub mod workspace_bus;

// Re-export event bus types
pub use agent_config_bus::{AgentConfigBusContainer, AgentConfigEvent};
pub use code_selection_bus::{
    CodeSelectionBusContainer, CodeSelectionEvent, subscribe_entity_to_code_selections,
};
pub use permission_bus::{PermissionBusContainer, PermissionRequestEvent};
pub use session_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
pub use workspace_bus::{WorkspaceUpdateBusContainer, WorkspaceUpdateEvent};
