//! A file/directory entry returned by the workspace file listing (the `@`-mention
//! picker). Plain data so the [`WorkspaceFiles`](crate::ports::WorkspaceFiles)
//! port stays free of any filesystem type.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    /// Path relative to the listing root, with `/` separators.
    pub relative_path: String,
    pub is_dir: bool,
}
