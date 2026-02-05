/// Utilities for external system editors
use std::path::{Path, PathBuf};
use std::process::Command;

/// Detected editor configuration
#[derive(Clone)]
pub struct EditorConfig {
    pub name: String,
    pub command: String,
    pub icon: gpui_component::IconName,
}

/// Available system editors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEditor {
    VSCode,
    VSCodeInsiders,
    Cursor,
    Zed,
    Windsurf,
    Antigravity,
    Trae,
    IntelliJIdea,
    IntelliJIdeaCE,
    WebStorm,
    PyCharm,
    Sublime,
    Atom,
}

impl SystemEditor {
    fn command_name(&self) -> &str {
        match self {
            SystemEditor::VSCode => "code",
            SystemEditor::VSCodeInsiders => "code-insiders",
            SystemEditor::Cursor => "cursor",
            SystemEditor::Zed => "zed",
            SystemEditor::Windsurf => "windsurf",
            SystemEditor::Antigravity => "agy",
            SystemEditor::Trae => "trae",
            SystemEditor::IntelliJIdea => "idea",
            SystemEditor::IntelliJIdeaCE => "idea-ce",
            SystemEditor::WebStorm => "webstorm",
            SystemEditor::PyCharm => "pycharm",
            SystemEditor::Sublime => "subl",
            SystemEditor::Atom => "atom",
        }
    }

    fn display_name(&self) -> &str {
        match self {
            SystemEditor::VSCode => "VS Code",
            SystemEditor::VSCodeInsiders => "VS Code Insiders",
            SystemEditor::Cursor => "Cursor",
            SystemEditor::Zed => "Zed",
            SystemEditor::Windsurf => "Windsurf",
            SystemEditor::Antigravity => "Antigravity",
            SystemEditor::Trae => "Trae",
            SystemEditor::IntelliJIdea => "IntelliJ IDEA",
            SystemEditor::IntelliJIdeaCE => "IntelliJ IDEA CE",
            SystemEditor::WebStorm => "WebStorm",
            SystemEditor::PyCharm => "PyCharm",
            SystemEditor::Sublime => "Sublime Text",
            SystemEditor::Atom => "Atom",
        }
    }

    fn icon(&self) -> gpui_component::IconName {
        // Use available icons from the IconName enum
        gpui_component::IconName::File
    }

    fn is_available(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            Command::new("where")
                .arg(self.command_name())
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Command::new("which")
                .arg(self.command_name())
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
    }
}

/// Detect the first available system editor
pub fn detect_system_editor() -> Option<EditorConfig> {
    let editors = [
        SystemEditor::VSCode,
        SystemEditor::Cursor,
        SystemEditor::Zed,
        SystemEditor::Windsurf,
        SystemEditor::Antigravity,
        SystemEditor::Trae,
        SystemEditor::VSCodeInsiders,
        SystemEditor::IntelliJIdea,
        SystemEditor::IntelliJIdeaCE,
        SystemEditor::WebStorm,
        SystemEditor::PyCharm,
        SystemEditor::Sublime,
        SystemEditor::Atom,
    ];

    for editor in &editors {
        if editor.is_available() {
            return Some(EditorConfig {
                name: editor.display_name().to_string(),
                command: editor.command_name().to_string(),
                icon: editor.icon(),
            });
        }
    }

    None
}

/// Detect all available system editors
pub fn detect_all_system_editors() -> Vec<EditorConfig> {
    let editors = [
        SystemEditor::VSCode,
        SystemEditor::Cursor,
        SystemEditor::Zed,
        SystemEditor::Windsurf,
        SystemEditor::Antigravity,
        SystemEditor::Trae,
        SystemEditor::VSCodeInsiders,
        SystemEditor::IntelliJIdea,
        SystemEditor::IntelliJIdeaCE,
        SystemEditor::WebStorm,
        SystemEditor::PyCharm,
        SystemEditor::Sublime,
        SystemEditor::Atom,
    ];

    editors
        .iter()
        .filter(|editor| editor.is_available())
        .map(|editor| EditorConfig {
            name: editor.display_name().to_string(),
            command: editor.command_name().to_string(),
            icon: editor.icon(),
        })
        .collect()
}

/// Open a path in the detected system editor
pub fn open_in_system_editor(path: &Path) -> Result<(), String> {
    let editor = detect_system_editor()
        .ok_or_else(|| "No system editor found. Please install VSCode, IntelliJ IDEA, or another supported editor.".to_string())?;
    log::info!("Opening {:?} in {}", path, editor.name);
    Command::new(&editor.command)
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open editor: {}", e))?;

    log::info!("Opened {:?} in {}", path, editor.name);
    Ok(())
}

/// Open a path with a specific editor command
pub fn open_with_editor(path: &Path, command: &str, editor_name: &str) -> Result<(), String> {
    log::info!("Opening {:?} in {}", path, editor_name);
    Command::new(command)
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open editor: {}", e))?;

    log::info!("Opened {:?} in {}", path, editor_name);
    Ok(())
}

/// Open a folder in the system file manager
pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    log::info!("Opening {:?} in file manager", path);

    #[cfg(target_os = "macos")]
    let command = "open";

    #[cfg(target_os = "windows")]
    let command = "explorer";

    #[cfg(target_os = "linux")]
    let command = "xdg-open";

    Command::new(command)
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open file manager: {}", e))?;

    log::info!("Opened {:?} in file manager", path);
    Ok(())
}
