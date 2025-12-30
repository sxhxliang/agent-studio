use gpui::{App, Context, ParentElement, Styled, Task, Window, div, px};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath,
    list::{ListDelegate, ListItem, ListState},
};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// File item in the file picker
#[derive(Clone, Debug)]
pub struct FileItem {
    pub name: String,
    pub path: PathBuf,
    pub is_folder: bool,
    pub relative_path: String,
}

impl FileItem {
    fn new(name: String, path: PathBuf, is_folder: bool, base_path: &Path) -> Self {
        let relative_path = path
            .strip_prefix(base_path)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        Self {
            name,
            path,
            is_folder,
            relative_path,
        }
    }
}

/// Delegate for the file picker list
pub struct FilePickerDelegate {
    all_items: Vec<FileItem>,
    filtered_items: Vec<FileItem>,
    search_query: String,
    selected_index: Option<usize>,
    on_select: Option<Box<dyn Fn(FileItem) + 'static>>,
    selection_tx: Option<mpsc::UnboundedSender<FileItem>>,
}

impl FilePickerDelegate {
    pub fn new(root_path: &Path) -> Self {
        let all_items = Self::scan_directory(root_path, root_path);

        Self {
            filtered_items: all_items.clone(),
            all_items,
            search_query: String::new(),
            selected_index: None,
            on_select: None,
            selection_tx: None,
        }
    }

    pub fn with_selection_sender(mut self, tx: mpsc::UnboundedSender<FileItem>) -> Self {
        self.selection_tx = Some(tx);
        self
    }

    pub fn on_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(FileItem) + 'static,
    {
        self.on_select = Some(Box::new(callback));
        self
    }

    pub fn filtered_items(&self) -> &[FileItem] {
        &self.filtered_items
    }

    /// Scan directory recursively and return all files and folders
    fn scan_directory(path: &Path, base_path: &Path) -> Vec<FileItem> {
        let mut items = Vec::new();

        // Skip common ignore patterns
        let ignore_patterns = [
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

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                let file_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Skip ignored directories
                if ignore_patterns.contains(&file_name.as_str()) {
                    continue;
                }

                let is_folder = entry_path.is_dir();

                // Add the current item (folder or file)
                items.push(FileItem::new(
                    file_name.clone(),
                    entry_path.clone(),
                    is_folder,
                    base_path,
                ));

                // Recursively scan subdirectories (limit depth to avoid too many files)
                if is_folder {
                    // Only go 3 levels deep
                    let depth = entry_path
                        .strip_prefix(base_path)
                        .map(|p| p.components().count())
                        .unwrap_or(0);

                    if depth < 3 {
                        items.extend(Self::scan_directory(&entry_path, base_path));
                    }
                }
            }
        }

        // Sort: folders first, then files, alphabetically within each group
        items.sort_by(|a, b| match (a.is_folder, b.is_folder) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.relative_path.cmp(&b.relative_path),
        });

        items
    }

    /// Update search query and filter items
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query.to_lowercase();

        if self.search_query.is_empty() {
            self.filtered_items = self.all_items.clone();
        } else {
            self.filtered_items = self
                .all_items
                .iter()
                .filter(|item| {
                    item.name.to_lowercase().contains(&self.search_query)
                        || item
                            .relative_path
                            .to_lowercase()
                            .contains(&self.search_query)
                })
                .cloned()
                .collect();
        }
    }
}

impl ListDelegate for FilePickerDelegate {
    type Item = ListItem;

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.set_search_query(query.to_string());
        Task::ready(())
    }

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.filtered_items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<'_, ListState<FilePickerDelegate>>,
    ) -> Option<Self::Item> {
        let item = self.filtered_items.get(ix.row)?;
        let theme = cx.theme();

        let icon = if item.is_folder {
            Icon::new(IconName::Folder)
        } else {
            Icon::new(IconName::File)
        };

        Some(
            ListItem::new(ix).w_full().py_1().px_2().child(
                gpui_component::h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .child(
                        gpui_component::h_flex()
                            .gap_2()
                            .items_center()
                            .child(icon.size(px(16.)).text_color(if item.is_folder {
                                theme.accent
                            } else {
                                theme.foreground
                            }))
                            .child(div().text_sm().child(item.name.clone())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(item.relative_path.clone()),
                    ),
            ),
        )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix.map(|i| i.row);
    }

    fn confirm(&mut self, _: bool, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {
        if let Some(selected_row) = self.selected_index {
            if let Some(item) = self.filtered_items.get(selected_row).cloned() {
                // Only select files, not folders
                if !item.is_folder {
                    if let Some(callback) = &self.on_select {
                        callback(item.clone());
                    }
                    if let Some(tx) = &self.selection_tx {
                        let _ = tx.send(item);
                    }
                }
            }
        }
    }

    fn cancel(&mut self, _: &mut Window, _: &mut Context<ListState<Self>>) {
        // Close the popover on cancel
    }
}
