//! # agentx-domain — the domain core (center of the hexagon)
//!
//! Entities, value objects, domain state machines, and **port traits** — the
//! contracts the rest of the system is built around.
//!
//! ## Dependency rule (hard constraint)
//! This crate depends on **no framework**: never add `gpui`, `tokio`,
//! `agent-client-protocol`, or any I/O crate here. Only pure-data crates are
//! allowed (`serde`, `thiserror`, `chrono`, `uuid`).
//!
//! All external capabilities (agent communication, persistence, configuration)
//! are declared here as traits ("ports") and implemented by outer adapter
//! crates ([`agentx-acp`], [`agentx-store`]). The domain never names a concrete
//! adapter — adapters depend on the domain, not the other way around.
//!
//! [`agentx-acp`]: https://docs.rs/agentx-acp
//! [`agentx-store`]: https://docs.rs/agentx-store
