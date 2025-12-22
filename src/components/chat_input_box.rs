use gpui::{
    AnyElement, App, Bounds, Corner, ElementId, Entity, Focusable, InteractiveElement, IntoElement,
    ParentElement, Pixels, Point, RenderOnce, Styled, Window, anchored, deferred, div,
    prelude::FluentBuilder, px,
};
use std::rc::Rc;

use gpui_component::{
    ActiveTheme, Disableable, ElementExt, Icon, IconName, Sizable, button::{Button, ButtonCustomVariant, ButtonVariants}, h_flex, input::{Input, InputState}, list::{List, ListDelegate, ListState}, popover::Popover, select::{Select, SelectState}, v_flex
};

use agent_client_protocol::{AvailableCommand, ImageContent};

use crate::app::actions::AddCodeSelection;
use crate::components::AgentItem;
use crate::core::services::SessionStatus;

/// A reusable chat input component with context controls and send button.
///
/// Features:
/// - Add context button at the top with popover containing searchable list
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
    context_list: Option<AnyElement>,
    context_list_focus: Option<gpui::FocusHandle>,
    context_popover_open: bool,
    on_context_popover_change: Option<Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
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
    /// Command suggestions to display
    command_suggestions: Vec<AvailableCommand>,
    /// Whether to show command suggestions
    show_command_suggestions: bool,
}

#[derive(Default)]
struct CommandPopoverAnchor {
    bounds: Option<Bounds<Pixels>>,
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
            context_list: None,
            context_list_focus: None,
            context_popover_open: false,
            on_context_popover_change: None,
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
            command_suggestions: Vec::new(),
            show_command_suggestions: false,
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

    /// Set the context list state for the popover
    pub fn context_list<D: ListDelegate + 'static>(
        mut self,
        list: Entity<ListState<D>>,
        cx: &App,
    ) -> Self {
        self.context_list_focus = Some(list.focus_handle(cx));
        self.context_list = Some(List::new(&list).into_any_element());
        self
    }

    /// Set whether the context popover is open
    pub fn context_popover_open(mut self, open: bool) -> Self {
        self.context_popover_open = open;
        self
    }

    /// Set a callback for when the context popover open state changes
    pub fn on_context_popover_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_context_popover_change = Some(Box::new(callback));
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
}

impl RenderOnce for ChatInputBox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_send = self.on_send;
        let on_cancel = self.on_cancel;
        let on_new_session = self.on_new_session;
        let on_paste_callback = self.on_paste.clone();
        let input_state_for_paste = self.input_state.clone();
        let command_anchor = window.use_keyed_state(
            "command-popover-anchor",
            cx,
            |_, _| CommandPopoverAnchor::default(),
        );
        let input_value = self.input_state.read(cx).value();
        let is_empty = input_value.trim().is_empty();

        // Get theme after use_keyed_state to avoid borrow conflicts
        let theme = cx.theme();

        // Build the context popover with searchable list
        let add_context_button = Button::new("add-context")
            .label("Add context")
            .icon(Icon::new(IconName::Asterisk))
            .ghost()
            .small();
        
        let context_element = if let Some(context_list) = self.context_list {
            let on_change = self.on_context_popover_change;
            let mut popover = Popover::new("context-popover")
                .p_0()
                .text_sm()
                .open(self.context_popover_open)
                .on_open_change(move |open, window, cx| {
                    if let Some(ref callback) = on_change {
                        callback(open, window, cx);
                    }
                })
                .trigger(add_context_button)
                .child(context_list)
                .w(px(280.))
                .h(px(300.));

            if let Some(focus) = self.context_list_focus {
                popover = popover.track_focus(&focus);
            }

            popover.into_any_element()
        } else {
            add_context_button.into_any_element()
        };

        let show_commands = self.show_command_suggestions && !self.command_suggestions.is_empty();
        let command_popover = if show_commands {
            let bounds = command_anchor.read(cx).bounds;
            let commands = self.command_suggestions;
            let command_count = commands.len();

            bounds.map(|bounds| {
                let position = bounds.corner(Corner::TopLeft)
                    + Point {
                        x: px(0.),
                        y: -px(8.),
                    };

                deferred(
                    anchored()
                        .snap_to_window_with_margin(px(8.))
                        .anchor(Corner::BottomLeft)
                        .position(position)
                        .child(
                            v_flex()
                                .occlude()
                                .w(bounds.size.width)
                                .gap_2()
                                .p_3()
                                .rounded(px(12.))
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.popover)
                                .shadow_lg()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Available Commands:"),
                                )
                                .children(commands.into_iter().enumerate().map(
                                    |(idx, command)| {
                                        let row = h_flex()
                                            .w_full()
                                            .gap_3()
                                            .items_center()
                                            .py_1()
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
                                                    .child(command.description),
                                            );

                                        row.when(idx + 1 < command_count, |row| {
                                            row.border_b_1().border_color(theme.border)
                                        })
                                    },
                                )),
                        ),
                )
                .with_priority(1)
                .into_any_element()
            })
        } else {
            None
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
                        // Top row: Pasted images, code selections, and Add context button with popover
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
                            .child(context_element),
                    )
                    .child(
                        // Textarea (multi-line input)
                        div()
                            .w_full()
                            .on_prepaint({
                                let command_anchor = command_anchor.clone();
                                move |bounds, _, cx| {
                                    command_anchor.update(cx, |state, cx| {
                                        state.bounds = Some(bounds);
                                        cx.notify();
                                    });
                                }
                            })
                            .child(Input::new(&self.input_state).appearance(false)),
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
                                    .child(
                                        Button::new("mcp")
                                            .label("mcp")
                                            .icon(Icon::new(IconName::Globe))
                                            .ghost()
                                            .small(),
                                    ),
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
            .when_some(command_popover, |this, command_popover| {
                this.child(command_popover)
            })
    }
}
