use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Pixels,
    Render, Styled, Window, prelude::FluentBuilder as _, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use gpui_term::{
    Clear, Copy, Event, InputOrigin, Paste, SelectAll, Terminal, TerminalBuilder, TerminalConfig,
    TerminalContent, TerminalMiddleware, TerminalView, TextStyle,
};

use crate::panels::dock_panel::DockPanel;

/// Terminal Panel - An integrated terminal emulator
pub struct TerminalPanel {
    focus_handle: FocusHandle,
    terminal: Option<Entity<Terminal>>,
    terminal_view: Option<Entity<TerminalView>>,
    text_style: TextStyle,
    status: TerminalStatus,
}

#[derive(Debug, Clone, PartialEq)]
enum TerminalStatus {
    Initializing,
    Ready,
    Failed(String),
}

impl DockPanel for TerminalPanel {
    fn title() -> &'static str {
        "Terminal"
    }

    fn description() -> &'static str {
        "Integrated terminal emulator"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn paddings() -> Pixels {
        px(0.)  // No padding for terminal to maximize space
    }
}

impl TerminalPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Load terminal configuration
        let terminal_config = TerminalConfig::load_or_create()
            .unwrap_or_else(|_| TerminalConfig::default());
        let text_style = TextStyle::from_config(&terminal_config);

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            terminal: None,
            terminal_view: None,
            text_style,
            status: TerminalStatus::Initializing,
        };

        // Initialize terminal asynchronously
        panel.initialize_terminal(window, cx);

        panel
    }

    /// Initialize the terminal in the background
    fn initialize_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = Self::platform_shell();
        let mut env_vars: HashMap<String, String> = env::vars().collect();
        env_vars.insert("TERM".to_string(), "xterm-256color".to_string());
        env_vars.insert("COLORTERM".to_string(), "truecolor".to_string());

        let window_id = window.window_handle().window_id().as_u64();
        let terminal_task = TerminalBuilder::new(
            env::current_dir().ok(),
            shell,
            env_vars,
            None,
            window_id,
            cx,
        );

        let text_style = self.text_style.clone();
        let weak_self = cx.entity().downgrade();
        let window_handle = window.window_handle();

        cx.spawn(async move |_entity, cx| {
            let builder = match terminal_task.await {
                Ok(b) => b,
                Err(e) => {
                    log::error!("[TerminalPanel] Failed to create terminal: {}", e);
                    let error_msg = format!("Failed to create terminal: {}", e);
                    _ = cx.update_window(window_handle, |_, _window, cx| {
                        if let Some(entity) = weak_self.upgrade() {
                            entity.update(cx, |this, cx| {
                                this.status = TerminalStatus::Failed(error_msg);
                                cx.notify();
                            });
                        }
                    });
                    return;
                }
            };

            _ = cx.update_window(window_handle, |_, window, cx| {
                if let Some(entity) = weak_self.upgrade() {
                    entity.update(cx, |this, cx| {
                        let terminal = cx.new(|cx| builder.subscribe(cx));

                        // Add middleware for logging if needed
                        terminal.update(cx, |terminal, _| {
                            terminal.add_middleware(Arc::new(LoggingMiddleware::new()));
                        });

                        let terminal_view = cx.new(|cx| {
                            TerminalView::new_with_style(terminal.clone(), text_style, window, cx)
                        });

                        this.terminal = Some(terminal);
                        this.terminal_view = Some(terminal_view);
                        this.status = TerminalStatus::Ready;
                        cx.notify();

                        // Focus the terminal view
                        if let Some(tv) = &this.terminal_view {
                            let focus_handle = tv.read(cx).focus_handle(cx);
                            focus_handle.focus(window, cx);
                        }
                    });
                }
            });
        })
        .detach();
    }

    /// Get the platform-specific shell
    fn platform_shell() -> Option<String> {
        #[cfg(windows)]
        {
            if let Ok(shell) = env::var("SHELL") {
                return Some(shell);
            }

            if let Some(pwsh) = Self::find_pwsh() {
                return Some(pwsh);
            }

            if let Ok(root) = env::var("SystemRoot") {
                let mut path = std::path::PathBuf::from(root);
                path.push("System32");
                path.push("WindowsPowerShell");
                path.push("v1.0");
                path.push("powershell.exe");
                return Some(path.to_string_lossy().into_owned());
            }

            return Some("powershell".to_string());
        }

        #[cfg(not(windows))]
        {
            if let Ok(shell) = env::var("SHELL") {
                return Some(shell);
            }
            if std::path::Path::new("/bin/zsh").exists() {
                Some("/bin/zsh".to_string())
            } else {
                Some("/bin/bash".to_string())
            }
        }
    }

    #[cfg(windows)]
    fn find_pwsh() -> Option<String> {
        if let Some(path) = Self::find_on_path("pwsh.exe") {
            return Some(path);
        }

        let roots = ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"];
        for key in roots {
            if let Ok(root) = env::var(key) {
                for suffix in ["PowerShell\\7\\pwsh.exe", "PowerShell\\7-preview\\pwsh.exe"] {
                    let path = std::path::PathBuf::from(&root).join(suffix);
                    if path.is_file() {
                        return Some(path.to_string_lossy().into_owned());
                    }
                }
            }
        }

        if let Ok(root) = env::var("LOCALAPPDATA") {
            for suffix in [
                "Microsoft\\PowerShell\\7\\pwsh.exe",
                "Microsoft\\PowerShell\\7-preview\\pwsh.exe",
            ] {
                let path = std::path::PathBuf::from(&root).join(suffix);
                if path.is_file() {
                    return Some(path.to_string_lossy().into_owned());
                }
            }
        }

        None
    }

    #[cfg(windows)]
    fn find_on_path(executable: &str) -> Option<String> {
        let path = env::var_os("PATH")?;
        for dir in env::split_paths(&path) {
            let candidate = dir.join(executable);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    }

    /// Clear the terminal
    fn clear_terminal(&mut self, _: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(Clear), cx);
    }

    /// Copy selected text
    fn copy(&mut self, _: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(Copy), cx);
    }

    /// Paste text
    fn paste(&mut self, _: &gpui::ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.dispatch_action(Box::new(Paste), cx);
    }
}

impl Focusable for TerminalPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_ready = self.status == TerminalStatus::Ready;
        let is_initializing = self.status == TerminalStatus::Initializing;
        let is_failed = matches!(self.status, TerminalStatus::Failed(_));

        v_flex()
            .size_full()
            .bg(theme.background)
            .child(
                // Header with terminal controls
                h_flex()
                    .w_full()
                    .h(px(40.))
                    .px_3()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child("Terminal"),
                            )
                            .when(is_ready, |el| {
                                el.child(
                                    gpui::div()
                                        .w(px(8.))
                                        .h(px(8.))
                                        .rounded(px(4.))
                                        .bg(theme.success),
                                )
                            })
                            .when(is_initializing, |el| {
                                el.child(
                                    gpui::div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Initializing..."),
                                )
                            })
                            .when(is_failed, |el| {
                                el.child(
                                    gpui::div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Failed"),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .when(is_ready, |el| {
                                el.child(
                                    Button::new("copy")
                                        .icon(Icon::new(IconName::Copy))
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(Self::copy)),
                                )
                                .child(
                                    Button::new("paste")
                                        .icon(Icon::new(IconName::File))
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(Self::paste)),
                                )
                                .child(
                                    Button::new("clear")
                                        .icon(Icon::new(crate::assets::Icon::Trash2))
                                        .ghost()
                                        .small()
                                        .on_click(cx.listener(Self::clear_terminal)),
                                )
                            }),
                    ),
            )
            .child(
                // Terminal content area
                gpui::div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .map(|el| {
                        if let Some(terminal_view) = &self.terminal_view {
                            el.child(terminal_view.clone())
                        } else if is_initializing {
                            el.flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    v_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(IconName::LoaderCircle)
                                                .text_color(theme.muted_foreground)
                                                .size_6(),
                                        )
                                        .child(
                                            gpui::div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child("Initializing terminal..."),
                                        ),
                                )
                        } else if let TerminalStatus::Failed(ref error) = self.status {
                            el.flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    v_flex()
                                        .gap_3()
                                        .items_center()
                                        .max_w(px(400.))
                                        .p_4()
                                        .child(
                                            Icon::new(IconName::TriangleAlert)
                                                .text_color(theme.muted_foreground)
                                                .size_12(),
                                        )
                                        .child(
                                            gpui::div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(theme.foreground)
                                                .child("Failed to Initialize Terminal"),
                                        )
                                        .child(
                                            gpui::div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(error.clone()),
                                        ),
                                )
                        } else {
                            el
                        }
                    }),
            )
    }
}

/// Middleware for logging terminal events (optional, can be disabled in production)
struct LoggingMiddleware {
    enabled: bool,
}

impl LoggingMiddleware {
    fn new() -> Self {
        // Enable logging in debug mode
        Self {
            enabled: cfg!(debug_assertions),
        }
    }
}

impl TerminalMiddleware for LoggingMiddleware {
    fn on_input(
        &self,
        input: std::borrow::Cow<'static, [u8]>,
        _origin: InputOrigin,
    ) -> Option<std::borrow::Cow<'static, [u8]>> {
        // Pass through input without logging to avoid spam
        Some(input)
    }

    fn on_event(&self, event: &Event) {
        if self.enabled {
            log::trace!("[TerminalPanel] Event: {:?}", event);
        }
    }

    fn on_output(&self, _content: &TerminalContent) {
        // Don't log output to avoid spam
    }
}
