//! # agentx-store — persistence adapter (driven / outbound)
//!
//! Implements the domain's persistence ports against the local filesystem:
//! - [`FsSessionRepository`] — session timelines as JSON Lines.
//! - [`FsConfigStore`] — the JSON configuration file, mapped to/from the domain
//!   via an internal DTO (the anti-corruption layer for configuration).
//!
//! [`paths`] is the single source of truth for on-disk locations, consolidating
//! the two duplicated `config_manager` copies from the legacy code.
//!
//! ## Dependency rule
//! Depends on `agentx-domain` only (to implement its port traits) plus pure I/O
//! crates. It must not depend on `agentx-app` or `agentx-ui`. The composition
//! root injects this adapter into the ports the application consumes.

mod config_store;
pub mod paths;
mod session_repo;
mod workspace_files;
mod workspace_repo;

pub use config_store::FsConfigStore;
pub use session_repo::FsSessionRepository;
pub use workspace_files::FsWorkspaceFiles;
pub use workspace_repo::FsWorkspaceRepository;

use agentx_domain::StoreError;

/// Map a filesystem error into a [`StoreError`], preserving "not found" so
/// callers can distinguish a missing file from a real I/O failure.
pub(crate) fn io_err(error: std::io::Error) -> StoreError {
    if error.kind() == std::io::ErrorKind::NotFound {
        StoreError::NotFound(error.to_string())
    } else {
        StoreError::Io(error.to_string())
    }
}

/// Map a JSON (de)serialization error into a [`StoreError`].
pub(crate) fn serde_err(error: serde_json::Error) -> StoreError {
    StoreError::Serde(error.to_string())
}
