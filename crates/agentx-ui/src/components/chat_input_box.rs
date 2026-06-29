//! The signature chat composer: a bordered card with an auto-growing textarea,
//! attachment chips, `@`/`/` autocomplete, in-bar agent/mode/model selects, an
//! MCP toggle popover, and a round send/cancel button.
//!
//! A [`RenderOnce`] builder: it owns nothing long-lived. The parent supplies the
//! shared [`InputState`] and the three [`SelectState`]s plus data + callbacks;
//! the suggestion engine is kept in per-render keyed state. This is how the
//! composer stays decoupled from the chat/launcher logic that drives it.

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, ClickEvent, ElementId, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, SharedString, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState, Paste},
    popover::Popover,
    select::{Select, SelectState},
    v_flex,
};

use agentx_domain::{McpServerConfig, SessionStatus, SlashCommand};

use crate::components::{
    AgentItem, FileItem, InputSuggestion, InputSuggestionItem, InputSuggestionState,
    ModeSelectItem, ModelSelectItem,
};

/// A pasted image shown as a chip. The parent keeps the real bytes; the composer
/// only displays the filename.
#[derive(Clone, Debug)]
pub struct ImageAttachment {
    pub filename: String,
    pub mime_type: String,
    pub data: String,
}

/// A code selection forwarded from an editor, shown as a chip.
#[derive(Clone, Debug)]
pub struct CodeSelection {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// One entry in the composer's autocomplete popover: a slash command or a file.
#[derive(Clone)]
enum ChatSuggestion {
    Command(SlashCommand),
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

/// A reusable chat composer with context controls and a send/cancel button.
#[derive(IntoElement)]
pub struct ChatInputBox {
    id: ElementId,
    input_state: Entity<InputState>,
    title: Option<String>,
    on_send: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    on_cancel: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    mode_select: Option<Entity<SelectState<Vec<ModeSelectItem>>>>,
    model_select: Option<Entity<SelectState<Vec<ModelSelectItem>>>>,
    agent_select: Option<Entity<SelectState<Vec<AgentItem>>>>,
    agent_status_text: Option<String>,
    pasted_images: Vec<ImageAttachment>,
    code_selections: Vec<CodeSelection>,
    selected_files: Vec<String>,
    on_remove_image: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    on_remove_code_selection: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    on_remove_file: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
    on_paste: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    session_status: Option<SessionStatus>,
    file_suggestions: Vec<FileItem>,
    on_file_select: Option<Box<dyn Fn(&FileItem, &mut Window, &mut App) + 'static>>,
    command_suggestions: Vec<SlashCommand>,
    show_command_suggestions: bool,
    on_command_select: Option<Box<dyn Fn(&SlashCommand, &mut Window, &mut App) + 'static>>,
    available_mcps: Vec<(String, McpServerConfig)>,
    selected_mcps: Vec<String>,
    on_mcp_toggle: Option<Rc<dyn Fn(&(String, bool), &mut Window, &mut App) + 'static>>,
    disabled: bool,
    /// When true, the send button stays enabled on empty input (the launcher
    /// starts a session with an optional first message).
    allow_empty_send: bool,
}

impl ChatInputBox {
    pub fn new(id: impl Into<ElementId>, input_state: Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            input_state,
            title: None,
            on_send: None,
            on_cancel: None,
            mode_select: None,
            model_select: None,
            agent_select: None,
            agent_status_text: None,
            pasted_images: Vec::new(),
            code_selections: Vec::new(),
            selected_files: Vec::new(),
            on_remove_image: None,
            on_remove_code_selection: None,
            on_remove_file: None,
            on_paste: None,
            session_status: None,
            file_suggestions: Vec::new(),
            on_file_select: None,
            command_suggestions: Vec::new(),
            show_command_suggestions: false,
            on_command_select: None,
            available_mcps: Vec::new(),
            selected_mcps: Vec::new(),
            on_mcp_toggle: None,
            disabled: false,
            allow_empty_send: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn on_send<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_send = Some(Box::new(callback));
        self
    }

    pub fn on_cancel<F>(mut self, callback: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        self.on_cancel = Some(Box::new(callback));
        self
    }

    pub fn mode_select(mut self, select: Entity<SelectState<Vec<ModeSelectItem>>>) -> Self {
        self.mode_select = Some(select);
        self
    }

    pub fn model_select(mut self, select: Entity<SelectState<Vec<ModelSelectItem>>>) -> Self {
        self.model_select = Some(select);
        self
    }

    pub fn agent_select(mut self, select: Entity<SelectState<Vec<AgentItem>>>) -> Self {
        self.agent_select = Some(select);
        self
    }

    pub fn agent_status_text(mut self, text: impl Into<String>) -> Self {
        self.agent_status_text = Some(text.into());
        self
    }

    pub fn pasted_images(mut self, images: Vec<ImageAttachment>) -> Self {
        self.pasted_images = images;
        self
    }

    pub fn on_remove_image<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_image = Some(Rc::new(callback));
        self
    }

    pub fn on_paste<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_paste = Some(Rc::new(callback));
        self
    }

    pub fn code_selections(mut self, selections: Vec<CodeSelection>) -> Self {
        self.code_selections = selections;
        self
    }

    pub fn on_remove_code_selection<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_code_selection = Some(Rc::new(callback));
        self
    }

    pub fn selected_files(mut self, files: Vec<String>) -> Self {
        self.selected_files = files;
        self
    }

    pub fn on_remove_file<F>(mut self, callback: F) -> Self
    where
        F: Fn(&usize, &mut Window, &mut App) + 'static,
    {
        self.on_remove_file = Some(Rc::new(callback));
        self
    }

    pub fn session_status(mut self, status: Option<SessionStatus>) -> Self {
        self.session_status = status;
        self
    }

    pub fn file_suggestions(mut self, files: Vec<FileItem>) -> Self {
        self.file_suggestions = files;
        self
    }

    pub fn on_file_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(&FileItem, &mut Window, &mut App) + 'static,
    {
        self.on_file_select = Some(Box::new(callback));
        self
    }

    pub fn command_suggestions(mut self, commands: Vec<SlashCommand>) -> Self {
        self.command_suggestions = commands;
        self
    }

    pub fn show_command_suggestions(mut self, show: bool) -> Self {
        self.show_command_suggestions = show;
        self
    }

    pub fn on_command_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(&SlashCommand, &mut Window, &mut App) + 'static,
    {
        self.on_command_select = Some(Box::new(callback));
        self
    }

    pub fn available_mcps(mut self, mcps: Vec<(String, McpServerConfig)>) -> Self {
        self.available_mcps = mcps;
        self
    }

    pub fn selected_mcps(mut self, mcps: Vec<String>) -> Self {
        self.selected_mcps = mcps;
        self
    }

    pub fn on_mcp_toggle<F>(mut self, callback: F) -> Self
    where
        F: Fn(&(String, bool), &mut Window, &mut App) + 'static,
    {
        self.on_mcp_toggle = Some(Rc::new(callback));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn allow_empty_send(mut self, allow: bool) -> Self {
        self.allow_empty_send = allow;
        self
    }
}

impl RenderOnce for ChatInputBox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_send = self.on_send;
        let on_cancel = self.on_cancel;
        let on_paste_callback = self.on_paste.clone();
        let input_state_for_paste = self.input_state.clone();
        let input_state = self.input_state.clone();
        let disabled = self.disabled;
        let allow_empty_send = self.allow_empty_send;

        // The suggestion engine wraps the shared input; keyed state keeps it alive
        // across renders without the parent having to own it.
        let suggestion_state_id =
            ElementId::NamedChild(Arc::new(self.id.clone()), "command-suggestions".into());
        let suggestion_state = window.use_keyed_state(suggestion_state_id, cx, |window, cx| {
            InputSuggestionState::with_input(input_state.clone(), window, cx)
        });

        let input_value = self.input_state.read(cx).value();
        let is_empty = input_value.trim().is_empty();
        let has_attachments = !self.pasted_images.is_empty()
            || !self.code_selections.is_empty()
            || !self.selected_files.is_empty();

        let theme = cx.theme();
        let muted_foreground = theme.muted_foreground;
        let border = theme.border;
        let background = theme.background;
        let accent = theme.accent;
        let primary = theme.primary;
        let muted = theme.muted;
        let foreground = theme.foreground;

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
            .gap_2()
            .px(px(24.))
            .when_some(self.title, |this, title| {
                this.child(
                    h_flex()
                        .w_full()
                        .pb_1p5()
                        .child(div().text_sm().text_color(muted_foreground).child(title)),
                )
            })
            .child(
                v_flex()
                    .w_full()
                    .gap_2p5()
                    .p_3()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(border)
                    .bg(background)
                    .shadow_md()
                    .when_some(on_paste_callback, |this, callback| {
                        let input_state = input_state_for_paste.clone();
                        this.on_action(move |_: &Paste, window, cx| {
                            callback(window, cx);

                            if let Some(clipboard_item) = cx.read_from_clipboard() {
                                let has_images = clipboard_item
                                    .entries()
                                    .iter()
                                    .any(|entry| matches!(entry, gpui::ClipboardEntry::Image(_)));

                                if !has_images {
                                    if let Some(text) = clipboard_item.text() {
                                        input_state.update(cx, |state, cx| {
                                            state.insert(text, window, cx);
                                        });
                                    }
                                }
                            }
                        })
                    })
                    .when(has_attachments, |this| {
                        let chip_text_color = foreground.opacity(0.85);
                        let render_chip = |id_prefix: &'static str,
                                           idx: usize,
                                           icon_name: IconName,
                                           label: String,
                                           bg_color,
                                           border_color,
                                           icon_color,
                                           on_remove: Option<
                            Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>,
                        >| {
                            h_flex()
                                .gap_1()
                                .items_center()
                                .py_0p5()
                                .px_1p5()
                                .rounded(px(6.))
                                .bg(bg_color)
                                .border_1()
                                .border_color(border_color)
                                .child(Icon::new(icon_name).size(px(13.)).text_color(icon_color))
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(chip_text_color)
                                        .child(label),
                                )
                                .child(
                                    Button::new((id_prefix, idx))
                                        .icon(Icon::new(IconName::Close))
                                        .ghost()
                                        .xsmall()
                                        .when_some(on_remove, |btn, callback| {
                                            btn.on_click(move |_ev, window, cx| {
                                                callback(&idx, window, cx);
                                            })
                                        }),
                                )
                                .into_any_element()
                        };

                        let mut chips = Vec::new();
                        chips.extend(self.pasted_images.iter().enumerate().map(|(idx, image)| {
                            render_chip(
                                "remove-image",
                                idx,
                                IconName::File,
                                image.filename.clone(),
                                accent.opacity(0.1),
                                accent.opacity(0.3),
                                accent,
                                self.on_remove_image.clone(),
                            )
                        }));
                        chips.extend(self.code_selections.iter().enumerate().map(
                            |(idx, selection)| {
                                let filename = std::path::Path::new(&selection.file_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(&selection.file_path);
                                let display_text = if selection.start_line == selection.end_line {
                                    format!("{}:{}", filename, selection.start_line)
                                } else {
                                    format!(
                                        "{}:{}~{}",
                                        filename, selection.start_line, selection.end_line
                                    )
                                };
                                render_chip(
                                    "remove-code-selection",
                                    idx,
                                    IconName::Frame,
                                    display_text,
                                    primary.opacity(0.1),
                                    primary.opacity(0.3),
                                    primary,
                                    self.on_remove_code_selection.clone(),
                                )
                            },
                        ));
                        chips.extend(self.selected_files.iter().enumerate().map(
                            |(idx, file_path)| {
                                let filename = std::path::Path::new(file_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| file_path.clone());
                                render_chip(
                                    "remove-file",
                                    idx,
                                    IconName::File,
                                    filename,
                                    muted.opacity(0.6),
                                    border,
                                    foreground.opacity(0.7),
                                    self.on_remove_file.clone(),
                                )
                            },
                        ));

                        this.child(
                            h_flex()
                                .w_full()
                                .gap_1p5()
                                .items_center()
                                .flex_wrap()
                                .children(chips),
                        )
                    })
                    .child({
                        let mut input = InputSuggestion::new(&suggestion_state)
                            .id(ElementId::NamedChild(
                                Arc::new(self.id.clone()),
                                "command-suggestion-input".into(),
                            ))
                            .items(suggestions)
                            .enabled((show_files || show_commands) && !disabled)
                            .when_some(suggestion_header, |input, header| input.header(header))
                            .max_height(px(200.))
                            .apply_on_confirm(apply_on_confirm)
                            .input(move |state| {
                                Input::new(state).appearance(false).disabled(disabled)
                            })
                            .render_item(|item, _selected, _window, cx| {
                                let theme = cx.theme();
                                match item {
                                    ChatSuggestion::Command(command) => h_flex()
                                        .w_full()
                                        .gap_3()
                                        .items_center()
                                        .child(
                                            div()
                                                .w(px(140.))
                                                .text_sm()
                                                .font_family("Monaco, 'Courier New', monospace")
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
                                        ),
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
                                                    .child(icon.size(px(16.)).text_color(
                                                        if file.is_folder {
                                                            theme.accent
                                                        } else {
                                                            theme.foreground
                                                        },
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(theme.popover_foreground)
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
                    })
                    .child(
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
                                    .when_some(self.agent_status_text.clone(), |this, text| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(muted_foreground)
                                                .child(text),
                                        )
                                    })
                                    .when_some(self.mode_select, |this, mode_select| {
                                        this.child(
                                            Select::new(&mode_select).small().appearance(false),
                                        )
                                    })
                                    .when_some(self.model_select, |this, model_select| {
                                        this.child(
                                            Select::new(&model_select).small().appearance(false),
                                        )
                                    })
                                    .child({
                                        let selected_count = self.selected_mcps.len();
                                        let has_mcps = !self.available_mcps.is_empty();
                                        let available_mcps = self.available_mcps.clone();
                                        let selected_mcps = self.selected_mcps.clone();
                                        let on_mcp_toggle = self.on_mcp_toggle.clone();
                                        let label_text = if selected_count > 0 {
                                            format!("MCP ({selected_count})")
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
                                                    .disabled(!has_mcps),
                                            )
                                            .content(move |_state, _window, cx| {
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
                                                            .child("No MCP servers"),
                                                    );
                                                } else {
                                                    content = content.child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                                            .pb_2()
                                                            .border_b_1()
                                                            .border_color(theme.border)
                                                            .child("Select MCP Servers"),
                                                    );
                                                    for (idx, (name, config)) in
                                                        available_mcps.iter().enumerate()
                                                    {
                                                        let is_selected =
                                                            selected_mcps.contains(name);
                                                        let mcp_name = name.clone();
                                                        let callback = on_mcp_toggle.clone();
                                                        content = content.child(
                                                            Checkbox::new(("mcp-cb", idx))
                                                                .label(name.clone())
                                                                .checked(is_selected)
                                                                .disabled(!config.enabled)
                                                                .on_click(
                                                                    move |checked, window, cx| {
                                                                        if let Some(cb) = &callback
                                                                        {
                                                                            cb(
                                                                                &(
                                                                                    mcp_name
                                                                                        .clone(),
                                                                                    *checked,
                                                                                ),
                                                                                window,
                                                                                cx,
                                                                            );
                                                                        }
                                                                    },
                                                                ),
                                                        );
                                                    }
                                                }
                                                content
                                            })
                                    }),
                            )
                            .child({
                                let (icon, is_in_progress) = match self.session_status {
                                    Some(SessionStatus::Running) => {
                                        (Icon::new(IconName::Pause), true)
                                    }
                                    _ => (Icon::new(IconName::ArrowUp), false),
                                };
                                let btn_disabled = disabled
                                    || (is_empty
                                        && !has_attachments
                                        && !is_in_progress
                                        && !allow_empty_send);

                                let mut btn = Button::new("send-or-cancel")
                                    .icon(icon)
                                    .rounded_full()
                                    .small()
                                    .disabled(btn_disabled);

                                btn = if btn_disabled {
                                    btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(muted.opacity(0.3))
                                            .foreground(muted_foreground.opacity(0.4)),
                                    )
                                } else if is_in_progress {
                                    btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(theme.red)
                                            .foreground(background)
                                            .hover(theme.red.opacity(0.9)),
                                    )
                                } else {
                                    btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(primary)
                                            .foreground(background)
                                            .hover(primary.opacity(0.9)),
                                    )
                                };

                                if is_in_progress {
                                    if let Some(on_cancel_handler) = on_cancel {
                                        btn = btn.on_click(move |ev, window, cx| {
                                            on_cancel_handler(ev, window, cx);
                                        });
                                    }
                                } else if let Some(handler) = on_send {
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
