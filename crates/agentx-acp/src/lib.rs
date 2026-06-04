//! # agentx-acp — agent gateway adapter (driven / outbound)
//!
//! Implements the domain's `AgentGateway` port over the Agent Client Protocol.
//! Owns agent subprocess lifecycle as supervised actors (one task/runtime per
//! agent, command-in / event-out) and translates ACP schema types to and from
//! domain types — an anti-corruption layer that keeps ACP out of the core.
//!
//! ## Dependency rule
//! Depends on `agentx-domain` (the port and the types it maps to) and
//! `agentx-bus` (to emit agent events). It must not depend on `agentx-app` or
//! `agentx-ui`. ACP schema types stop here; they never leak inward.
