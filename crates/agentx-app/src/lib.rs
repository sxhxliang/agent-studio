//! # agentx-app — application layer (use-cases)
//!
//! Cohesive application services that orchestrate the domain through its ports.
//! Each service holds only the ports it needs (`Arc<dyn Port>`) plus any state
//! its use-cases share — for example [`SessionService`] owns the live-session
//! registry that its create/send/close operations all touch. This replaces the
//! legacy god-object services and the `AppState` service-locator: dependencies
//! are explicit and injected, and state is scoped to the service that owns it.
//!
//! Events flow outward on the bus: services publish [`DomainEvent`]s and
//! [`PersistenceProjector`] turns the timeline ones into stored rows, so the UI
//! and persistence react without anyone calling them directly.
//!
//! ## Dependency rule
//! Depends on `agentx-domain` and `agentx-bus` only. It depends on **ports
//! (traits)**, never on concrete adapter crates (`agentx-acp`, `agentx-store`).
//! The composition root wires adapters into these ports at startup.
//!
//! [`DomainEvent`]: agentx_domain::DomainEvent

mod config_service;
mod persistence;
mod session_service;
mod workspace_service;

#[cfg(test)]
mod fakes;

pub use config_service::ConfigService;
pub use persistence::PersistenceProjector;
pub use session_service::SessionService;
pub use workspace_service::{TaskView, WorkspaceService};
