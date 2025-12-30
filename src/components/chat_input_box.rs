use gpui::{
    App, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use std::{rc::Rc, sync::Arc};

use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    popover::Popover,
    select::{Select, SelectState},
    v_flex,
};

use agent_client_protocol::{AvailableCommand, ImageContent};

use crate::app::actions::AddCodeSelection;
use crate::components::{
    AgentItem, FileItem, InputSuggestion, InputSuggestionItem, InputSuggestionState,
};
use crate::core::config::McpServerConfig;
use crate::core::services::SessionStatus;

impl InputSuggestionItem for AvailableCommand {
    fn label(&self) -> SharedString {
        SharedString::from(self.name.clone())
    }

    fn apply_text(&self) -> SharedString {
        SharedString::from(format!("/{} ", self.name))
    }
}

#[derive(Clone)]
enum ChatSuggestion {
    Command(AvailableCommand),
    File(FileItem),
}

impl InputSuggestionItem for ChatSuggestion {
    fn label(&self) -> SharedString {
        match self {
            Self::Command(command) => SharedString::from(command.name.clone()),
            Self::File(file) => SharedString::from(file.name.clone()),
        }
    }

    fn apply_text(&self) -> SharedString {
        match self {
            Self::Command(command) => SharedString::from(format!("/{} ", command.name)),
            Self::File(file) => {
                let mut path = file.relative_path.clone();
                if file.is_folder && !path.ends_with('/') {
                    path.push('/');
                }
                SharedString::from(format!("@{} ", path))
            }
        }
    }
}

/// A reusable chat input component with context controls and send button.
///
/// Features:
/// - @ trigger for file suggestions
/// - Multi-line textarea with auto-grow (2-8 rows)
/// - Action buttons (attach, mode select, sources)
/// - Send button with icon
/// - Optional title displayed above the input box
/// - Support for pasting multiple images with filename display
#[derive(IntoElement)]
pub struct ChatInputBox {
    id: ElementId,
    input_state: Entity<InputState>,
    title: Option<String>,
    on_send: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    on_cancel: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    mode_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    agent_select: Option<Entity<SelectState<Vec<AgentItem>>>>,
    session_select: Option<Entity<SelectState<Vec<String>>>>,
    on_new_session: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    pasted_images: Vec<(ImageContent, String)>, // (ImageContent, filename for display)
    code_selections: Vec<AddCodeSelection>,     // Code selections from editor
    selected_files: Vec<String>,                // Selected file paths from file picker
    on_remove_image: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    on_remove_code_selection: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    on_remove_file: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    on_paste: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    session_status: Option<SessionStatus>, // Session status for button state
    session_id: Option<String>,            // Session ID for cancel action
    file_suggestions: Vec<FileItem>,
    on_file_select: Option<Box<dyn Fn(&FileItem, &mut Window, &mut App) + 'static>>,
    /// Command suggestions to display
    command_suggestions: Vec<AvailableCommand>,
    /// Whether to show command suggestions
    show_command_suggestions: bool,
    /// Optional click/confirm handler for command selection
    on_command_select: Option<Box<dyn Fn(&AvailableCommand, &mut Window, &mut App) + 'static>>,
    /// Available MCP servers (name, config)
    available_mcps: Vec<(String, McpServerConfig)>,
    /// Selected MCP server names
    selected_mcps: Vec<String>,
    /// Callback when MCP checkbox is clicked (passes (name, checked) tuple)
    on_mcp_toggle: Option<Rc<dyn Fn(&(String, bool), &mut Window, &mut App) + 'static>>,
}

impl ChatInputBox {
    /// Create a new ChatInputBox with the given input state
    pub fn new(id: impl Into<ElementId>, input_state: Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            input_state,
            title: None,
            on_send: None,
            on_cancel: None,
            mode_select: None,
            agent_select: None,
            session_select: None,
            on_new_session: None,
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            selected_files: Vec::new(),
            on_remove_image: None,
            on_remove_code_selection: None,
            on_remove_file: None,
            on_paste: None,
            session_status: None,
            session_id: None,
            file_suggestions: Vec::new(),
            on_file_select: None,
            command_suggestions: Vec::new(),
            show_command_suggestions: false,
            on_command_select: None,
            available_mcps: Vec::new(),
            selected_mcps: Vec::new(),
            on_mcp_toggle: None,
        }
    }

    /// Set an optional title to display above the input box
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set a callback for when the send button is clicked
    pub fn on_send<F>(mut self, callback: F) -> Self
    where
        F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_send = Some(Box::new(callback));
        self
    }

    /// Set a callback for when the cancel button is clicked (when session is in progress)
    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_cancel = Some(Box::new(callback));
        self
    }

    /// Set the mode select state
    pub fn mode_select(mut self, select: Entity<SelectState<Vec<&'static str>>>) -> Self {
        self.mode_select = Some(select);
        self
    }

    /// Set the agent select state
    pub fn agent_select(mut self, select: Entity<SelectState<Vec<AgentItem>>>) -> Self {
        self.agent_select = Some(select);
        self
    }

    /// Set the session select state
    pub fn session_select(mut self, select: Entity<SelectState<Vec<String>>>) -> Self {
        self.session_select = Some(select);
        self
    }

    /// Set a callback for when the new session button is clicked
    pub fn on_new_session<F>(mut self, callback: F) -> Self
    where
        F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_new_session = Some(Box::new(callback));
        self
    }

    /// Set the list of pasted images
    pub fn pasted_images(mut self, images: Vec<(ImageContent, String)>) -> Self {
        self.pasted_images = images;
        self
    }

    /// Set a callback for when an image is removed
    pub fn on_remove_image<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_image = Some(Rc::new(callback));
        self
    }

    /// Set a callback for when paste event occurs
    pub fn on_paste<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_paste = Some(Rc::new(callback));
        self
    }

    /// Set the list of code selections
    pub fn code_selections(mut self, selections: Vec<AddCodeSelection>) -> Self {
        self.code_selections = selections;
        self
    }

    /// Set a callback for when a code selection is removed
    pub fn on_remove_code_selection<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_code_selection = Some(Rc::new(callback));
        self
    }

    /// Set the list of selected files
    pub fn selected_files(mut self, files: Vec<String>) -> Self {
        self.selected_files = files;
        self
    }

    /// Set a callback for when a file is removed
    pub fn on_remove_file<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_file = Some(Rc::new(callback));
        self
    }

    /// Set the session status (affects send button appearance)
    pub fn session_status(mut self, status: Option<SessionStatus>) -> Self {
        self.session_status = status;
        self
    }

    /// Set the session ID (required for cancel action)
    pub fn session_id(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// Set file suggestions to display
    pub fn file_suggestions(mut self, files: Vec<FileItem>) -> Self {
        self.file_suggestions = files;
        self
    }

    /// Set a callback for when a file suggestion is selected
    pub fn on_file_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(&FileItem, &mut Window, &mut App) + 'static,
    {
        self.on_file_select = Some(Box::new(callback));
        self
    }

    /// Set command suggestions to display
    pub fn command_suggestions(mut self, commands: Vec<AvailableCommand>) -> Self {
        self.command_suggestions = commands;
        self
    }

    /// Set whether to show command suggestions
    pub fn show_command_suggestions(mut self, show: bool) -> Self {
        self.show_command_suggestions = show;
        self
    }

    /// Set a callback for when a command suggestion is selected
    pub fn on_command_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(&AvailableCommand, &mut Window, &mut App) + 'static,
    {
        self.on_command_select = Some(Box::new(callback));
        self
    }

    /// Set available MCP servers
    pub fn available_mcps(mut self, mcps: Vec<(String, McpServerConfig)>) -> Self {
        self.available_mcps = mcps;
        self
    }

    /// Set selected MCP server names
    pub fn selected_mcps(mut self, mcps: Vec<String>) -> Self {
        self.selected_mcps = mcps;
        self
    }

    /// Set a callback for when MCP checkbox is toggled
    pub fn on_mcp_toggle<F>(mut self, callback: F) -> Self
    where
        F: Fn(&(String, bool), &mut Window, &mut App) + 'static,
    {
        self.on_mcp_toggle = Some(Rc::new(callback));
        self
    }
}

impl RenderOnce for ChatInputBox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_send = self.on_send;
        let on_cancel = self.on_cancel;
        let on_new_session = self.on_new_session;
        let on_paste_callback = self.on_paste.clone();
        let input_state_for_paste = self.input_state.clone();
        let input_state = self.input_state.clone();
        let suggestion_state_id =
            ElementId::NamedChild(Arc::new(self.id.clone()), "command-suggestions".into());
        let suggestion_state = window.use_keyed_state(suggestion_state_id, cx, |window, cx| {
            InputSuggestionState::with_input(input_state.clone(), window, cx)
        });
        let input_value = self.input_state.read(cx).value();
        let is_empty = input_value.trim().is_empty();

        // Get theme after use_keyed_state to avoid borrow conflicts
        let theme = cx.theme();

        let show_commands = self.show_command_suggestions && !self.command_suggestions.is_empty();
        let show_files = !self.file_suggestions.is_empty();
        let (suggestions, suggestion_header, apply_on_confirm) = if show_files {
            (
                self.file_suggestions
                    .clone()
                    .into_iter()
                    .map(ChatSuggestion::File)
                    .collect::<Vec<_>>(),
                Some("Files"),
                self.on_file_select.is_none(),
            )
        } else if show_commands {
            (
                self.command_suggestions
                    .clone()
                    .into_iter()
                    .map(ChatSuggestion::Command)
                    .collect::<Vec<_>>(),
                Some("Available Commands"),
                self.on_command_select.is_none(),
            )
        } else {
            (Vec::new(), None, true)
        };

        v_flex()
            .w_full()
            .gap_3()
            .px(px(32.)) // Left and right padding for spacing
            .when_some(self.title, |this, title| {
                this.child(
                    h_flex().w_full().pb_2().child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(title),
                    ),
                )
            })
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .p_4()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.secondary)
                    .shadow_lg()
                    .when_some(on_paste_callback, |this, callback| {
                        let input_state = input_state_for_paste.clone();
                        this.on_action(move |_: &crate::app::actions::Paste, window, cx| {
                            // First, try to handle images via the callback
                            callback(window, cx);

                            // Check if clipboard has text (and no images were handled)
                            // If the callback handled images, we don't want to paste text
                            // The callback should handle image detection, we just handle text fallback
                            if let Some(clipboard_item) = cx.read_from_clipboard() {
                                // Check if there are any images in clipboard
                                let has_images = clipboard_item
                                    .entries()
                                    .iter()
                                    .any(|entry| matches!(entry, gpui::ClipboardEntry::Image(_)));

                                // If no images, try to paste text to input
                                if !has_images {
                                    if let Some(text) = clipboard_item.text() {
                                        let input = input_state.clone();
                                        input.update(cx, |state, cx| {
                                            // Insert text at cursor position
                                            state.insert(text, window, cx);
                                        });
                                    }
                                }
                            }
                        })
                    })
                    .child(
                        // Top row: Pasted images, code selections, and selected files
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            // Render pasted images
                            .children(self.pasted_images.iter().enumerate().map(
                                |(idx, (_image, filename))| {
                                    let on_remove = self.on_remove_image.clone();
                                    let idx_clone = idx;

                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .p_1()
                                        .px_2()
                                        .rounded(theme.radius)
                                        .bg(theme.muted)
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            Icon::new(IconName::File)
                                                .size(px(14.))
                                                .text_color(theme.accent),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.foreground)
                                                .child(filename.clone()),
                                        )
                                        .child(
                                            Button::new(("remove-image", idx))
                                                .icon(Icon::new(IconName::Close))
                                                .ghost()
                                                .xsmall()
                                                .when_some(on_remove, |btn, callback| {
                                                    btn.on_click(move |_ev, window, cx| {
                                                        callback(&idx_clone, window, cx);
                                                    })
                                                }),
                                        )
                                        .into_any_element()
                                },
                            ))
                            // Render code selections
                            .children(self.code_selections.iter().enumerate().map(
                                |(idx, selection)| {
                                    let on_remove = self.on_remove_code_selection.clone();
                                    let idx_clone = idx;

                                    // Extract filename from path
                                    let filename = std::path::Path::new(&selection.file_path)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or(&selection.file_path);

                                    // Format the display text as "filename:start_line~end_line"
                                    let display_text = if selection.start_line == selection.end_line
                                    {
                                        format!("{}:{}", filename, selection.start_line)
                                    } else {
                                        format!(
                                            "{}:{}~{}",
                                            filename, selection.start_line, selection.end_line
                                        )
                                    };

                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .p_1()
                                        .px_2()
                                        .rounded(theme.radius)
                                        .bg(theme.muted)
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            Icon::new(IconName::Frame)
                                                .size(px(14.))
                                                .text_color(theme.accent),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.foreground)
                                                .child(display_text),
                                        )
                                        .child(
                                            Button::new(("remove-code-selection", idx))
                                                .icon(Icon::new(IconName::Close))
                                                .ghost()
                                                .xsmall()
                                                .when_some(on_remove, |btn, callback| {
                                                    btn.on_click(move |_ev, window, cx| {
                                                        callback(&idx_clone, window, cx);
                                                    })
                                                }),
                                        )
                                        .into_any_element()
                                },
                            ))
                            // Render selected files
                            .children(
                                self.selected_files
                                    .into_iter()
                                    .enumerate()
                                    .map(|(idx, file_path)| {
                                    let on_remove = self.on_remove_file.clone();
                                    let idx_clone = idx;

                                    // Extract filename from path
                                    let filename = std::path::Path::new(&file_path)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or(file_path.clone());

                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .p_1()
                                        .px_2()
                                        .rounded(theme.radius)
                                        .bg(theme.muted)
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            Icon::new(IconName::File)
                                                .size(px(14.))
                                                .text_color(theme.accent),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.foreground)
                                                .child(filename),
                                        )
                                        .child(
                                            Button::new(("remove-file", idx))
                                                .icon(Icon::new(IconName::Close))
                                                .ghost()
                                                .xsmall()
                                                .when_some(on_remove, |btn, callback| {
                                                    btn.on_click(move |_ev, window, cx| {
                                                        callback(&idx_clone, window, cx);
                                                    })
                                                }),
                                        )
                                        .into_any_element()
                                    })
                                    .collect::<Vec<_>>(),
                            )
                    )
                    .child(
                        // Textarea (multi-line input)
                        {
                            let mut input = InputSuggestion::new(&suggestion_state)
                                .id(ElementId::NamedChild(
                                    Arc::new(self.id.clone()),
                                    "command-suggestion-input".into(),
                                ))
                                .items(suggestions)
                                .enabled(show_files || show_commands)
                                .when_some(suggestion_header, |input, header| {
                                    input.header(header)
                                })
                                .max_height(px(200.))
                                .apply_on_confirm(apply_on_confirm)
                                .input(|state| Input::new(state).appearance(false))
                                .render_item(|item, _selected, _window, cx| {
                                    let theme = cx.theme();
                                    match item {
                                        ChatSuggestion::Command(command) => {
                                            h_flex()
                                                .w_full()
                                                .gap_3()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .w(px(140.))
                                                        .text_sm()
                                                        .font_family(
                                                            "Monaco, 'Courier New', monospace",
                                                        )
                                                        .text_color(theme.popover_foreground)
                                                        .child(format!("/{}", command.name)),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_sm()
                                                        .text_color(theme.muted_foreground)
                                                        .overflow_x_hidden()
                                                        .text_ellipsis()
                                                        .child(command.description.clone()),
                                                )
                                        }
                                        ChatSuggestion::File(file) => {
                                            let icon = if file.is_folder {
                                                Icon::new(IconName::Folder)
                                            } else {
                                                Icon::new(IconName::File)
                                            };

                                            h_flex()
                                                .w_full()
                                                .gap_3()
                                                .items_center()
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(
                                                            icon.size(px(16.)).text_color(
                                                                if file.is_folder {
                                                                    theme.accent
                                                                } else {
                                                                    theme.foreground
                                                                },
                                                            ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(
                                                                    theme.popover_foreground,
                                                                )
                                                                .child(file.name.clone()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .overflow_x_hidden()
                                                        .text_ellipsis()
                                                        .child(file.relative_path.clone()),
                                                )
                                        }
                                    }
                                });

                            if self.on_command_select.is_some() || self.on_file_select.is_some() {
                                let on_command_select = self.on_command_select;
                                let on_file_select = self.on_file_select;
                                input = input.on_confirm(move |item, window, cx| match item {
                                    ChatSuggestion::Command(command) => {
                                        if let Some(callback) = &on_command_select {
                                            callback(command, window, cx);
                                        }
                                    }
                                    ChatSuggestion::File(file) => {
                                        if let Some(callback) = &on_file_select {
                                            callback(file, window, cx);
                                        }
                                    }
                                });
                            }

                            div().w_full().child(input)
                        },
                    )
                    .child(
                        // Bottom row: Action buttons
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .when_some(self.agent_select.clone(), |this, agent_select| {
                                        this.child(
                                            Select::new(&agent_select)
                                                .small()
                                                .appearance(false)
                                                .w(px(140.)),
                                        )
                                    })
                                    .when_some(
                                        self.session_select.clone(),
                                        |this, session_select| {
                                            this.child(
                                                Select::new(&session_select)
                                                    .small()
                                                    .appearance(false),
                                            )
                                        },
                                    )
                                    .when_some(on_new_session, |this, on_new_session_callback| {
                                        this.child(
                                            Button::new("new-session")
                                                .icon(Icon::new(IconName::Plus))
                                                .ghost()
                                                .small()
                                                .on_click(on_new_session_callback),
                                        )
                                    })
                                    .when_some(self.mode_select, |this, mode_select| {
                                        this.child(
                                            Select::new(&mode_select).small().appearance(false),
                                        )
                                    })
                                    // MCP multi-select popover (simplified)
                                    .child({
                                        let selected_count = self.selected_mcps.len();
                                        let has_mcps = !self.available_mcps.is_empty();
                                        let available_mcps = self.available_mcps.clone();
                                        let selected_mcps = self.selected_mcps.clone();
                                        let on_mcp_toggle = self.on_mcp_toggle.clone();

                                        let label_text = if selected_count > 0 {
                                            format!("MCP ({})", selected_count)
                                        } else {
                                            "MCP".to_string()
                                        };

                                        Popover::new("mcp-popover")
                                            .trigger(
                                                Button::new("mcp")
                                                    .label(label_text)
                                                    .icon(Icon::new(IconName::Globe))
                                                    .ghost()
                                                    .small()
                                                    .disabled(!has_mcps)
                                            )
                                            .content(move |_state, _window, cx| {
                                                use gpui_component::checkbox::Checkbox;

                                                let theme = cx.theme();

                                                let mut content = v_flex()
                                                    .w(px(280.))
                                                    .max_h(px(350.))
                                                    .gap_2()
                                                    .p_3();

                                                if available_mcps.is_empty() {
                                                    content = content.child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(theme.muted_foreground)
                                                            .child("No MCP servers")
                                                    );
                                                } else {
                                                    // Header
                                                    content = content.child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .pb_2()
                                                            .border_b_1()
                                                            .border_color(theme.border)
                                                            .child("Select MCP Servers")
                                                    );

                                                    // Checkboxes
                                                    for (idx, (name, config)) in available_mcps.iter().enumerate() {
                                                        let is_selected = selected_mcps.contains(name);
                                                        let mcp_name = name.clone();
                                                        let callback = on_mcp_toggle.clone();

                                                        content = content.child(
                                                            Checkbox::new(("mcp-cb", idx))
                                                                .label(name.clone())
                                                                .checked(is_selected)
                                                                .disabled(!config.enabled)
                                                                .on_click(move |checked, window, cx| {
                                                                    if let Some(cb) = &callback {
                                                                        cb(&(mcp_name.clone(), *checked), window, cx);
                                                                    }
                                                                })
                                                        );
                                                    }
                                                }

                                                content
                                            })
                                    }),
                            )
                            .child({
                                // Determine button icon and color based on session status
                                let (btn, is_in_progress) = match self.session_status {
                                    Some(SessionStatus::InProgress) => {
                                        ( Button::new("cancel")
                                    .icon(Icon::new(crate::assets::Icon::SquarePause)), true)
                                    },
                                    _ => ( Button::new("send")
                                    .icon(Icon::new(IconName::ArrowUp)), false),
                                };

                                let mut btn = btn.rounded_full()
                                    .small()
                                    .disabled(is_empty && !is_in_progress);

                                // Set button colors based on state
                                if is_empty && !is_in_progress {
                                    // Disabled state: lighter/muted color
                                    btn = btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(theme.muted.opacity(0.5))
                                            .foreground(theme.muted_foreground.opacity(0.5)),
                                    );
                                } else if is_in_progress {
                                    // InProgress state: red background with white foreground
                                    btn = btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(theme.red)
                                            .foreground(theme.background)
                                            .hover(theme.red.opacity(0.85)),
                                    );
                                } else {
                                    // Normal/Pending state: primary color with hover effect
                                    btn = btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(theme.primary)
                                            .foreground(theme.background)
                                            .hover(theme.primary.opacity(0.85)),
                                    );
                                }

                                // Handle button click
                                if is_in_progress {
                                    // When in progress, call the on_cancel callback
                                    if let Some(on_cancel_handler) = on_cancel {
                                        btn = btn.on_click(move |ev, window, cx| {
                                            log::info!("ChatInputBox: Cancel button clicked for session");
                                            on_cancel_handler(ev, window, cx);
                                        });
                                    } else {
                                        log::warn!("ChatInputBox: Cannot cancel - on_cancel handler not set");
                                    }
                                } else if let Some(handler) = on_send {
                                    // Normal send behavior
                                    btn = btn.on_click(move |ev, window, cx| {
                                        handler(ev, window, cx);
                                    });
                                }

                                btn
                            }),
                    ),
            )
    }
}
