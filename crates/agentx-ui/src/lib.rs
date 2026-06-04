//! # agentx-ui — GPUI driving adapter (inbound)
//!
//! The user-facing adapter: dock framework, panels, view-models, reusable
//! widgets, and window chrome. Each panel follows the same shape — `mod.rs`
//! (wiring) + `model.rs` (state and intents) + `view.rs` (pure render) — so the
//! layout is predictable. A `PanelKind` enum replaces string-based dispatch.
//!
//! ## Dependency rule
//! Depends on `agentx-app` + `agentx-domain` (use-cases and types), `agentx-bus`
//! (to observe events), `agentx-acp-ui` (pure presentational ACP widgets), and
//! `gpui`. It must NOT depend on the driven adapters (`agentx-acp`,
//! `agentx-store`) — it only knows the application's use-cases and the domain.
//! Views hold no business logic and never reach for a global service locator.
