use gpui::{
    div, prelude::FluentBuilder, px, AnyElement, App, ElementId, Entity, Focusable, IntoElement,
    ParentElement, RenderOnce, Styled, Window,
};
use std::rc::Rc;

use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    list::{List, ListDelegate, ListState},
    popover::Popover,
    select::{Select, SelectState},
    v_flex, ActiveTheme, Disableable, Icon, IconName, Sizable,
};

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
    context_list: Option<AnyElement>,
    context_list_focus: Option<gpui::FocusHandle>,
    context_popover_open: bool,
    on_context_popover_change: Option<Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    mode_select: Option<Entity<SelectState<Vec<&'static str>>>>,
    agent_select: Option<Entity<SelectState<Vec<String>>>>,
    session_select: Option<Entity<SelectState<Vec<String>>>>,
    on_new_session: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    pasted_images: Vec<PastedImage>,
    on_remove_image: Option<Rc<dyn Fn(&usize, &mut Window, &mut App) + 'static>>,
}

/// Information about a pasted image
#[derive(Clone, Debug)]
pub struct PastedImage {
    pub path: String,
    pub filename: String,
}

impl ChatInputBox {
    /// Create a new ChatInputBox with the given input state
    pub fn new(id: impl Into<ElementId>, input_state: Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            input_state,
            title: None,
            on_send: None,
            context_list: None,
            context_list_focus: None,
            context_popover_open: false,
            on_context_popover_change: None,
            mode_select: None,
            agent_select: None,
            session_select: None,
            on_new_session: None,
            pasted_images: Vec::new(),
            on_remove_image: None,
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
    pub fn agent_select(mut self, select: Entity<SelectState<Vec<String>>>) -> Self {
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
    pub fn pasted_images(mut self, images: Vec<PastedImage>) -> Self {
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
}

impl RenderOnce for ChatInputBox {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let on_send = self.on_send;
        let on_new_session = self.on_new_session;
        let input_value = self.input_state.read(cx).value();
        let is_empty = input_value.trim().is_empty();

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
                    .child(
                        // Top row: Pasted images (if any) and Add context button with popover
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .children(self.pasted_images.iter().enumerate().map(|(idx, image)| {
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
                                            .child(image.filename.clone()),
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
                            }))
                            .child(context_element),
                    )
                    .child(
                        // Textarea (multi-line input)
                        Input::new(&self.input_state).appearance(false),
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
                                    .child(
                                        Button::new("attach")
                                            .icon(Icon::new(IconName::Asterisk))
                                            .ghost()
                                            .small(),
                                    )
                                    .when_some(self.agent_select.clone(), |this, agent_select| {
                                        this.child(
                                            Select::new(&agent_select).small().appearance(false),
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
                                        Button::new("sources")
                                            .label("All Sources")
                                            .icon(Icon::new(IconName::Globe))
                                            .ghost()
                                            .small(),
                                    ),
                            )
                            .child({
                                let mut btn = Button::new("send")
                                    .icon(Icon::new(IconName::ArrowUp))
                                    .rounded_full()
                                    .small()
                                    .disabled(is_empty);

                                // Set button colors based on empty state
                                if is_empty {
                                    // Disabled state: lighter/muted color
                                    btn = btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(theme.muted.opacity(0.5))
                                            .foreground(theme.muted_foreground.opacity(0.5)),
                                    );
                                } else {
                                    // Enabled state: primary color with hover effect
                                    btn = btn.custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(theme.primary)
                                            .foreground(theme.background)
                                            .hover(theme.primary.opacity(0.85)),
                                    );
                                }

                                if let Some(handler) = on_send {
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
