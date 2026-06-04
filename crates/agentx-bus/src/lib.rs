//! # agentx-bus — typed asynchronous event bus (infrastructure)
//!
//! A generic publish/subscribe bus: `publish::<E>(event)` and
//! `subscribe::<E>() -> Receiver<E>`, routed by [`std::any::TypeId`] so each
//! event type gets its own channel.
//!
//! It replaces the legacy hub's ~20 hand-written `subscribe_*` methods and its
//! synchronous-while-locked dispatch (a deadlock hazard): publishing hands the
//! event to a channel and returns immediately; subscribers receive on their own
//! tasks.
//!
//! ## Dependency rule
//! Standalone infrastructure depending only on `tokio` for channels. It is
//! generic over the event type and knows nothing about the domain. Event
//! *types* live in `agentx-domain`; the bus only moves them around.
