//! File-mention data + row rendering for the composer's `@` picker.
//!
//! [`FileItem`] is plain data produced by the app-layer file-listing use-case —
//! the UI does no filesystem scanning. [`render_file_item`] is the pure row the
//! composer's suggestion popover shows (folder/file icon + name + dim path).

use gpui::{AnyElement, IntoElement, ParentElement as _, Styled as _, div};
use gpui_component::{Icon, IconName, Sizable as _, Theme, h_flex};

/// A file or folder offered as an `@`-mention suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileItem {
    pub name: String,
    /// Path relative to the workspace root, with `/` separators.
    pub relative_path: String,
    pub is_folder: bool,
}

impl FileItem {
    pub fn new(name: impl Into<String>, relative_path: impl Into<String>, is_folder: bool) -> Self {
        Self {
            name: name.into(),
            relative_path: relative_path.into(),
            is_folder,
        }
    }
}

/// A suggestion row: folder/file icon + name, with the relative path dimmed on
/// the right. Pure — the caller supplies the theme.
pub fn render_file_item(item: &FileItem, theme: &Theme) -> AnyElement {
    let icon = if item.is_folder {
        IconName::Folder
    } else {
        IconName::File
    };
    let icon_color = if item.is_folder {
        theme.accent
    } else {
        theme.foreground
    };
    h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(icon).small().text_color(icon_color))
                .child(div().text_sm().child(item.name.clone())),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(item.relative_path.clone()),
        )
        .into_any_element()
}
