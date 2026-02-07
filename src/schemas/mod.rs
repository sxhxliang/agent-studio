// Re-export conversation schema from agentx-types
pub mod conversation_schema {
    pub use agentx_types::schemas::conversation::*;
}

// These schemas depend on gpui::SharedString, so they stay in the main crate
pub mod task_schema;
pub mod workspace_schema;
