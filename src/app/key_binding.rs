use gpui::{App, KeyBinding};

use crate::app::actions::{Open, Paste, Quit, ToggleSearch};
use gpui_term::{Clear, Copy, SelectAll};

// 导出KeyBinding设置函数,供主应用使用
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("/", ToggleSearch, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", Open, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-o", Open, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-f4", Quit, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, None),
        // Terminal keybindings
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some("Terminal")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-c", Copy, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", gpui_term::Paste, Some("Terminal")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-v", gpui_term::Paste, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-k", Clear, Some("Terminal")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-k", Clear, Some("Terminal")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some("Terminal")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-a", SelectAll, Some("Terminal")),
    ]);
}
