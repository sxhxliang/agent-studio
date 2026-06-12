//! # agentx-domain — the domain core (center of the hexagon)
//!
//! Entities, value objects, domain state machines, and **port traits** — the
//! contracts the rest of the system is built around.
//!
//! ## Dependency rule (hard constraint)
//! This crate depends on **no framework**: never add `gpui`, `tokio`,
//! `agent-client-protocol`, or any I/O crate here. Only pure-data crates are
//! allowed (`serde`, `thiserror`, `chrono`, `uuid`, and the `async-trait`
//! desugaring macro for object-safe ports).
//!
//! All external capabilities (agent communication, persistence, configuration)
//! are declared here as traits ("ports") and implemented by outer adapter
//! crates ([`agentx-acp`], [`agentx-store`]). The domain never names a concrete
//! adapter — adapters depend on the domain, not the other way around.
//!
//! ## Module map
//! - [`id`] — strongly-typed identifiers
//! - [`agent`], [`session`] — agents and the session lifecycle state machine
//! - [`message`], [`tool_call`], [`plan`] — the content an agent produces
//! - [`event`] — the in-session stream ([`SessionEvent`]) and bus notifications ([`DomainEvent`])
//! - [`permission`], [`workspace`], [`config`] — supporting models
//! - [`error`] — typed domain errors
//! - [`ports`] — the trait contracts adapters implement
//!
//! [`agentx-acp`]: https://docs.rs/agentx-acp
//! [`agentx-store`]: https://docs.rs/agentx-store

pub mod agent;
pub mod config;
pub mod error;
pub mod event;
pub mod files;
pub mod id;
pub mod message;
pub mod permission;
pub mod plan;
pub mod ports;
pub mod session;
pub mod tool_call;
pub mod workspace;

// Convenience re-exports of the most frequently used types so downstream crates
// can `use agentx_domain::SessionId` rather than `use agentx_domain::id::SessionId`.
pub use agent::{AgentDescriptor, AgentStatus, StopReason};
pub use config::{
    AgentConfig, CommandConfig, Config, DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES, McpServerConfig,
    ModelConfig, ProxyConfig,
};
pub use error::{AgentError, StoreError};
pub use event::{DomainEvent, PersistedEvent, SessionEvent};
pub use files::FileEntry;
pub use id::{AgentId, PermissionId, SessionId, TaskId, WorkspaceId};
pub use message::{ContentBlock, Message, ResourceContents, Role};
pub use permission::{
    PermissionOption, PermissionOptionKind, PermissionOutcome, PermissionRequest,
};
pub use plan::{Plan, PlanEntry, PlanEntryStatus, PlanPriority};
pub use ports::{
    AgentGateway, AgentRegistry, ConfigStore, SessionRepository, WorkspaceFiles,
    WorkspaceRepository,
};
pub use session::{
    ConfigOptionValue, InvalidTransition, Session, SessionConfigOption, SessionInit, SessionMode,
    SessionStatus, SlashCommand,
};
pub use tool_call::{ToolCall, ToolCallContent, ToolCallStatus, ToolKind};
pub use workspace::{Task, Workspace};
