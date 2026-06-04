//! # agentx-app — application layer (use-cases)
//!
//! One type per use-case (e.g. `SendMessage`, `CreateSession`,
//! `ResolvePermission`). Each holds only the ports it needs (`Arc<dyn Port>`)
//! and orchestrates the domain. This replaces the legacy god-object services
//! and the `AppState` service-locator: dependencies are explicit and injected.
//!
//! ## Dependency rule
//! Depends on `agentx-domain` and `agentx-bus` only. It depends on **ports
//! (traits)**, never on concrete adapter crates (`agentx-acp`, `agentx-store`).
//! The composition root wires adapters into these ports at startup.
