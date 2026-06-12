//! Filesystem implementation of [`WorkspaceFiles`].
//!
//! A bounded recursive scan (depth ≤ 3, common build/VCS dirs skipped), filtered
//! by a query and capped, for the composer's `@`-mention picker. Blocking
//! `std::fs` like the other store adapters, so it can be awaited from GPUI's
//! executor; callers run it off the UI thread.

use std::path::Path;

use async_trait::async_trait;

use agentx_domain::{FileEntry, StoreError, WorkspaceFiles};

const IGNORE: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    ".next",
    ".cache",
    "dist",
    "build",
    ".vscode",
    ".idea",
];
const MAX_DEPTH: usize = 3;
const MAX_RESULTS: usize = 30;

#[derive(Default)]
pub struct FsWorkspaceFiles;

impl FsWorkspaceFiles {
    pub fn new() -> Self {
        Self
    }
}

fn scan(dir: &Path, base: &Path, out: &mut Vec<FileEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() || IGNORE.contains(&name.as_str()) {
            continue;
        }
        let is_dir = path.is_dir();
        let relative_path = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(FileEntry {
            name,
            relative_path,
            is_dir,
        });
        if is_dir {
            let depth = path
                .strip_prefix(base)
                .map(|p| p.components().count())
                .unwrap_or(0);
            if depth < MAX_DEPTH {
                scan(&path, base, out);
            }
        }
    }
}

#[async_trait]
impl WorkspaceFiles for FsWorkspaceFiles {
    async fn list_files(&self, root: &Path, query: &str) -> Result<Vec<FileEntry>, StoreError> {
        let mut items = Vec::new();
        scan(root, root, &mut items);

        // Folders first, then alphabetical by relative path.
        items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.relative_path.cmp(&b.relative_path),
        });

        let query = query.to_lowercase();
        if !query.is_empty() {
            items.retain(|item| {
                item.name.to_lowercase().contains(&query)
                    || item.relative_path.to_lowercase().contains(&query)
            });
        }
        items.truncate(MAX_RESULTS);
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_and_filters_files_skipping_ignored_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.rs"), "").unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("lib.rs"), "").unwrap();
        std::fs::create_dir(root.join("node_modules")).unwrap();
        std::fs::write(root.join("node_modules").join("ignored.js"), "").unwrap();

        let files = FsWorkspaceFiles::new();

        let all = files.list_files(root, "").await.unwrap();
        // node_modules content is skipped; src + its file + main.rs remain.
        assert!(all.iter().any(|f| f.relative_path == "main.rs"));
        assert!(all.iter().any(|f| f.relative_path == "src/lib.rs"));
        assert!(
            !all.iter()
                .any(|f| f.relative_path.contains("node_modules/"))
        );

        let filtered = files.list_files(root, "lib").await.unwrap();
        assert!(filtered.iter().all(|f| {
            f.name.to_lowercase().contains("lib") || f.relative_path.to_lowercase().contains("lib")
        }));
        assert!(filtered.iter().any(|f| f.relative_path == "src/lib.rs"));
    }
}
