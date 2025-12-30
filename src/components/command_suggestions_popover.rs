use gpui::{
    App, Bounds, Context, Corner, ElementId, IntoElement, ParentElement, Pixels, Point, RenderOnce,
    Styled, Window, anchored, deferred, div, prelude::FluentBuilder, px,
};

use gpui_component::{
    ActiveTheme, IndexPath, h_flex,
    list::{List, ListDelegate, ListItem, ListState},
    v_flex,
};

use agent_client_protocol::AvailableCommand;

struct CommandSuggestionsListDelegate {
    commands: Vec<AvailableCommand>,
    selected_index: Option<usize>,
    on_select: Option<Box<dyn Fn(&AvailableCommand, &mut Window, &mut App) + 'static>>,
}

impl CommandSuggestionsListDelegate {
    fn new(
        commands: Vec<AvailableCommand>,
        on_select: Option<Box<dyn Fn(&AvailableCommand, &mut Window, &mut App) + 'static>>,
    ) -> Self {
        Self {
            commands,
            selected_index: None,
            on_select,
        }
    }

    fn set_commands(
        &mut self,
        commands: Vec<AvailableCommand>,
        on_select: Option<Box<dyn Fn(&AvailableCommand, &mut Window, &mut App) + 'static>>,
    ) {
        self.commands = commands;
        self.on_select = on_select;
        self.selected_index = None;
    }
}

impl ListDelegate for CommandSuggestionsListDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.commands.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let command = self.commands.get(ix.row)?;
        let theme = cx.theme();
        let command_count = self.commands.len();

        Some(
            ListItem::new(ix)
                .w_full()
                .child(
                    h_flex()
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
                )
                .when(ix.row + 1 < command_count, |item| {
                    item.border_b_1().border_color(theme.border)
                }),
        )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix.map(|ix| ix.row);
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(selected_index) = self.selected_index else {
            return;
        };
        let Some(command) = self.commands.get(selected_index) else {
            return;
        };
        if let Some(on_select) = &self.on_select {
            on_select(command, window, cx);
        }
    }
}

/// A popover component that displays command suggestions above an anchor element.
///
/// Features:
/// - Displays a list of available commands with names and descriptions
/// - Positioned above the anchor element
/// - Auto-adjusts to window boundaries
/// - Styled with theme colors
#[derive(IntoElement)]
pub struct CommandSuggestionsPopover {
    /// The bounds of the anchor element (typically the input box)
    anchor_bounds: Option<Bounds<Pixels>>,
    /// List of commands to display
    commands: Vec<AvailableCommand>,
    /// Whether the popover should be visible
    visible: bool,
    /// Optional click handler for command selection
    on_select: Option<Box<dyn Fn(&AvailableCommand, &mut Window, &mut App) + 'static>>,
    /// Key used to persist the list state across renders
    list_id: ElementId,
}

impl CommandSuggestionsPopover {
    /// Create a new CommandSuggestionsPopover
    pub fn new(commands: Vec<AvailableCommand>) -> Self {
        Self {
            anchor_bounds: None,
            commands,
            visible: true,
            on_select: None,
            list_id: ElementId::Name("command-suggestions-list".into()),
        }
    }

    /// Set the anchor bounds for positioning the popover
    pub fn anchor_bounds(mut self, bounds: Option<Bounds<Pixels>>) -> Self {
        self.anchor_bounds = bounds;
        self
    }

    /// Set whether the popover is visible
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set a callback for when a command is selected
    pub fn on_select<F>(mut self, callback: F) -> Self
    where
        F: Fn(&AvailableCommand, &mut Window, &mut App) + 'static,
    {
        self.on_select = Some(Box::new(callback));
        self
    }

    /// Set the list id used to persist the list state across renders
    pub fn list_id(mut self, list_id: ElementId) -> Self {
        self.list_id = list_id;
        self
    }
}

impl RenderOnce for CommandSuggestionsPopover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Early return if not visible or no commands
        if !self.visible || self.commands.is_empty() {
            return div().into_any_element();
        }

        // Calculate position based on anchor bounds
        match self.anchor_bounds {
            Some(bounds) => {
                let position = bounds.corner(Corner::TopLeft)
                    + Point {
                        x: px(0.),
                        y: -px(8.),
                    };

                let list_state = window.use_keyed_state(self.list_id.clone(), cx, |window, cx| {
                    ListState::new(
                        CommandSuggestionsListDelegate::new(Vec::new(), None),
                        window,
                        cx,
                    )
                });

                let commands = self.commands;
                let has_commands = !commands.is_empty();
                let on_select = self.on_select;
                list_state.update(cx, |state, cx| {
                    state.delegate_mut().set_commands(commands, on_select);
                    let selected = if has_commands {
                        Some(IndexPath::default())
                    } else {
                        None
                    };
                    state.set_selected_index(selected, window, cx);
                    cx.notify();
                });

                // Get theme after list state creation to avoid borrow conflicts.
                let theme = cx.theme();

                deferred(
                    anchored()
                        .snap_to_window_with_margin(px(8.))
                        .anchor(Corner::BottomLeft)
                        .position(position)
                        .child(
                            v_flex()
                                // .occlude()
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
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(140.0))
                                        .child(List::new(&list_state).size_full()),
                                ),
                        ),
                )
                .with_priority(1)
                .into_any_element()
            }
            None => div().into_any_element(),
        }
    }
}
