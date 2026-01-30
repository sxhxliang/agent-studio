pub mod clipboard;
pub mod external_editor;
pub mod file;
pub mod time;
pub mod tool_call;
/// Open a folder picker dialog and return the selected path
pub async fn pick_folder(title: &str) -> Option<std::path::PathBuf> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_folder()
        .await?;

    Some(folder.path().to_path_buf())
}

/// Open a folder picker dialog and log the selected path
pub async fn pick_and_log_folder(title: &str, context: &str) {
    match pick_folder(title).await {
        Some(path) => {
            println!("{} - Selected folder: {}", context, path.display());
            log::info!("{} - Selected folder: {}", context, path.display());
        }
        None => {
            println!("{} - No folder selected", context);
            log::info!("{} - Folder selection cancelled", context);
        }
    }
}
