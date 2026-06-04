//! # agentx-store — persistence adapter (driven / outbound)
//!
//! Implements the domain's persistence ports (session history as JSONL,
//! configuration, dock layout) against the local filesystem. Consolidates the
//! two duplicated `config_manager` copies into a single source of truth.
//!
//! ## Dependency rule
//! Depends on `agentx-domain` only (to implement its port traits) plus pure I/O
//! crates. It must not depend on `agentx-app` or `agentx-ui`. The composition
//! root injects this adapter into the ports the application consumes.
