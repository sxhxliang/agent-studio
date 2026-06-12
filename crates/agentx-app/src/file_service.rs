//! File-listing use-case for the composer's `@`-mention picker.
//!
//! A thin use-case over the [`WorkspaceFiles`] port; the UI calls it instead of
//! touching the filesystem itself.

use std::path::Path;
use std::sync::Arc;

use agentx_domain::{FileEntry, StoreError, WorkspaceFiles};

pub struct FileService {
    files: Arc<dyn WorkspaceFiles>,
}

impl FileService {
    pub fn new(files: Arc<dyn WorkspaceFiles>) -> Self {
        Self { files }
    }

    /// Files/directories under `root` matching `query`.
    pub async fn list_files(&self, root: &Path, query: &str) -> Result<Vec<FileEntry>, StoreError> {
        self.files.list_files(root, query).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeWorkspaceFiles;

    #[tokio::test]
    async fn list_files_delegates_and_filters() {
        let files = Arc::new(FakeWorkspaceFiles::with_entries(vec![
            FileEntry {
                name: "main.rs".into(),
                relative_path: "src/main.rs".into(),
                is_dir: false,
            },
            FileEntry {
                name: "lib.rs".into(),
                relative_path: "src/lib.rs".into(),
                is_dir: false,
            },
        ]));
        let service = FileService::new(files);

        assert_eq!(
            service.list_files(Path::new("/"), "").await.unwrap().len(),
            2
        );
        let filtered = service.list_files(Path::new("/"), "main").await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "main.rs");
    }
}
