//! # agentx-ui — GPUI driving adapter (inbound)
//!
//! The user-facing adapter: panels, view-models, and window chrome. Each panel
//! follows the same shape — a `mod.rs`/model half (state and intents) and a
//! `view.rs` half (pure render) — so the layout is predictable.
//!
//! ## Dependency rule
//! Depends on `agentx-app` + `agentx-domain` (use-cases and types), `agentx-bus`
//! (to observe events), and `gpui`. It must NOT depend on the driven adapters
//! (`agentx-acp`, `agentx-store`) — it only knows the application's use-cases and
//! the domain. Views hold no business logic and never reach for a global service
//! locator. The composition root (`agentx-shell`) wires the adapters in.
//!
//! ## The chat panel
//! [`ChatView`] drives the rewritten stack: it sends prompts through
//! `SessionService`, renders the streamed `DomainEvent`s, handles permission
//! requests, mode/model/command selection, and session browsing. Open it with
//! [`open_chat_window`].
//!
//! ## The launcher
//! [`open_welcome_window`] opens the app's entry surface: an agent picker plus a
//! recent-session list. Selecting an agent boots it and opens a [`ChatView`].

mod chat;
mod welcome;
mod workspace;

pub use chat::{ChatView, open_chat_window};
pub use welcome::open_welcome_window;
