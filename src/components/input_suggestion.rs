use std::{rc::Rc, sync::Arc};

use gpui::{
    App, AppContext as _, Bounds, ClickEvent, Context, Corner, ElementId, Entity, EventEmitter,
    Focusable, InteractiveElement as _, IntoElement, KeystrokeEvent, Length, MouseButton,
    ParentElement as _, Pixels, RenderOnce, SharedString, StyleRefinement, Styled, Subscription,
    Window, anchored, deferred, div, prelude::FluentBuilder as _, px,
};

use gpui_component::{
    ActiveTheme, ElementExt as _, StyledExt as _,
    input::{Input, InputEvent, InputState},
    list::ListItem,
    scroll::ScrollableElement as _,
    v_flex,
};

/// A suggestion item used by [`InputSuggestion`].
pub trait InputSuggestionItem: Clone {
    /// Display label for the suggestion item.
    fn label(&self) -> SharedString;

    /// Text applied to the input when the item is confirmed.
    fn apply_text(&self) -> SharedString {
        self.label()
    }
}

impl InputSuggestionItem for SharedString {
    fn label(&self) -> SharedString {
        self.clone()
    }
}

impl InputSuggestionItem for String {
    fn label(&self) -> SharedString {
        SharedString::from(self.clone())
    }
}

impl InputSuggestionItem for &'static str {
    fn label(&self) -> SharedString {
        SharedString::from(*self)
    }
}

/// Events emitted by [`InputSuggestionState`].
pub enum InputSuggestionEvent<T: InputSuggestionItem + 'static> {
    QueryChange(SharedString),
    Select(Option<usize>),
    Confirm(T),
    OpenChange(bool),
    Focus,
    Blur,
}

/// State for the [`InputSuggestion`] component.
pub struct InputSuggestionState<T: InputSuggestionItem + 'static> {
    input_state: Entity<InputState>,
    items: Vec<T>,
    selected: Option<usize>,
    open: bool,
    focused: bool,
    enabled: bool,
    clear_on_confirm: bool,
    apply_on_confirm: bool,
    ignore_next_change: bool,
    input_bounds: Option<Bounds<Pixels>>,
    on_query_change: Option<Rc<dyn Fn(&SharedString, &mut Window, &mut App)>>,
    on_confirm: Option<Rc<dyn Fn(&T, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    on_select_change: Option<Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>>,
    _subscriptions: Vec<Subscription>,
    _keystroke_subscription: Option<Subscription>,
}

impl<T: InputSuggestionItem + 'static> InputSuggestionState<T> {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| InputState::new(window, cx));
        Self::with_input(input_state, window, cx)
    }

    pub fn with_input(
        input_state: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _subscriptions = vec![cx.subscribe_in(&input_state, window, Self::on_input_event)];
        let weak_state = cx.weak_entity();
        let input_for_focus = input_state.clone();
        let _keystroke_subscription = Some(cx.intercept_keystrokes(move |event, window, cx| {
            if !input_for_focus.focus_handle(cx).is_focused(window) {
                return;
            }
            let Some(state) = weak_state.upgrade() else {
                return;
            };
            let mut handled = false;
            state.update(cx, |state, cx| {
                handled = state.handle_keystroke(event, window, cx);
            });
            if handled {
                cx.stop_propagation();
            }
        }));

        Self {
            input_state,
            items: Vec::new(),
            selected: None,
            open: false,
            focused: false,
            enabled: true,
            clear_on_confirm: true,
            apply_on_confirm: true,
            ignore_next_change: false,
            input_bounds: None,
            on_query_change: None,
            on_confirm: None,
            on_open_change: None,
            on_select_change: None,
            _subscriptions,
            _keystroke_subscription,
        }
    }

    pub fn input_state(&self) -> &Entity<InputState> {
        &self.input_state
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn input_bounds(&self) -> Option<Bounds<Pixels>> {
        self.input_bounds
    }

    pub fn set_items(&mut self, items: Vec<T>, window: &mut Window, cx: &mut Context<Self>) {
        self.items = items;
        self.ensure_selection();
        self.update_open(window, cx);
        cx.notify();
    }

    pub fn clear_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.items.clear();
        self.selected = None;
        self.update_open(window, cx);
        cx.notify();
    }

    pub fn set_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.enabled = enabled;
        self.update_open(window, cx);
        cx.notify();
    }

    pub fn set_clear_on_confirm(&mut self, clear_on_confirm: bool) {
        self.clear_on_confirm = clear_on_confirm;
    }

    pub fn set_apply_on_confirm(&mut self, apply_on_confirm: bool) {
        self.apply_on_confirm = apply_on_confirm;
    }

    pub fn set_callbacks(
        &mut self,
        on_query_change: Option<Rc<dyn Fn(&SharedString, &mut Window, &mut App)>>,
        on_confirm: Option<Rc<dyn Fn(&T, &mut Window, &mut App)>>,
        on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
        on_select_change: Option<Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>>,
    ) {
        self.on_query_change = on_query_change;
        self.on_confirm = on_confirm;
        self.on_open_change = on_open_change;
        self.on_select_change = on_select_change;
    }

    pub fn set_selected_index(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = index;
        self.emit_select(window, cx);
        cx.notify();
    }

    pub fn move_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        let count = self.items.len() as isize;
        let next = match self.selected {
            Some(current) => ((current as isize + delta).rem_euclid(count)) as usize,
            None => {
                if delta > 0 {
                    0
                } else {
                    (count - 1) as usize
                }
            }
        };
        self.selected = Some(next);
        self.emit_select(window, cx);
        cx.notify();
    }

    pub fn confirm_index(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.items.len() {
            return;
        }
        self.selected = Some(index);
        self.confirm_selected(window, cx);
    }

    fn handle_keystroke(
        &mut self,
        event: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.open || !self.enabled {
            return false;
        }
        if event.keystroke.modifiers.modified() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "up" => {
                self.move_selection(-1, window, cx);
                true
            }
            "down" => {
                self.move_selection(1, window, cx);
                true
            }
            "enter" => self.confirm_selected(window, cx),
            _ => false,
        }
    }

    fn on_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                if self.ignore_next_change {
                    self.ignore_next_change = false;
                    return;
                }
                let value = self.input_state.read(cx).value();
                if let Some(callback) = self.on_query_change.as_ref() {
                    callback(&value, window, cx);
                }
                cx.emit(InputSuggestionEvent::QueryChange(value));
            }
            InputEvent::Focus => {
                self.focused = true;
                self.update_open(window, cx);
                cx.emit(InputSuggestionEvent::Focus);
            }
            InputEvent::Blur => {
                self.focused = false;
                self.open = false;
                self.emit_open_change(window, cx);
                cx.emit(InputSuggestionEvent::Blur);
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {}
        }
    }

    fn ensure_selection(&mut self) {
        if self.items.is_empty() {
            self.selected = None;
        } else if self
            .selected
            .map(|ix| ix >= self.items.len())
            .unwrap_or(true)
        {
            self.selected = Some(0);
        }
    }

    fn update_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next_open = self.enabled && self.focused && !self.items.is_empty();
        if next_open == self.open {
            return;
        }
        self.open = next_open;
        if self.open && self.selected.is_none() && !self.items.is_empty() {
            self.selected = Some(0);
        }
        self.emit_open_change(window, cx);
    }

    fn emit_open_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_open_change.as_ref() {
            callback(&self.open, window, cx);
        }
        cx.emit(InputSuggestionEvent::OpenChange(self.open));
    }

    fn emit_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_select_change.as_ref() {
            callback(self.selected, window, cx);
        }
        cx.emit(InputSuggestionEvent::Select(self.selected));
    }

    fn confirm_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.selected else {
            return false;
        };
        let Some(item) = self.items.get(index).cloned() else {
            return false;
        };
        if !self.open {
            return false;
        }

        if self.apply_on_confirm {
            self.apply_item(&item, window, cx);
        }

        if self.clear_on_confirm {
            self.items.clear();
            self.selected = None;
        }

        self.open = false;
        self.emit_open_change(window, cx);

        if let Some(callback) = self.on_confirm.as_ref() {
            callback(&item, window, cx);
        }
        cx.emit(InputSuggestionEvent::Confirm(item));
        cx.notify();
        true
    }

    fn apply_item(&mut self, item: &T, window: &mut Window, cx: &mut Context<Self>) {
        let value = item.apply_text();
        self.ignore_next_change = true;
        let input_state = self.input_state.clone();
        window.defer(cx, move |window, cx| {
            input_state.update(cx, |state, cx| {
                state.set_value(value, window, cx);
                state.focus(window, cx);
            });
        });
    }
}

impl<T: InputSuggestionItem + 'static> EventEmitter<InputSuggestionEvent<T>>
    for InputSuggestionState<T>
{
}

/// A suggestion-enabled input element.
#[derive(IntoElement)]
pub struct InputSuggestion<T: InputSuggestionItem + 'static> {
    id: ElementId,
    state: Entity<InputSuggestionState<T>>,
    items: Option<Vec<T>>,
    enabled: Option<bool>,
    header: Option<SharedString>,
    menu_width: Length,
    max_height: Option<Pixels>,
    anchor: Corner,
    mouse_button: MouseButton,
    clear_on_confirm: bool,
    apply_on_confirm: bool,
    on_query_change: Option<Rc<dyn Fn(&SharedString, &mut Window, &mut App)>>,
    on_confirm: Option<Rc<dyn Fn(&T, &mut Window, &mut App)>>,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    on_select_change: Option<Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>>,
    input_builder: Option<Rc<dyn Fn(&Entity<InputState>) -> Input>>,
    render_item: Option<Rc<dyn Fn(&T, bool, &mut Window, &mut App) -> gpui::AnyElement>>,
    style: StyleRefinement,
}

impl<T: InputSuggestionItem + 'static> InputSuggestion<T> {
    pub fn new(state: &Entity<InputSuggestionState<T>>) -> Self {
        Self {
            id: ElementId::Name("input-suggestion".into()),
            state: state.clone(),
            items: None,
            enabled: None,
            header: None,
            menu_width: Length::Auto,
            max_height: None,
            anchor: Corner::BottomLeft,
            mouse_button: MouseButton::Right,
            clear_on_confirm: true,
            apply_on_confirm: true,
            on_query_change: None,
            on_confirm: None,
            on_open_change: None,
            on_select_change: None,
            input_builder: None,
            render_item: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn items(mut self, items: Vec<T>) -> Self {
        self.items = Some(items);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub fn header(mut self, header: impl Into<SharedString>) -> Self {
        self.header = Some(header.into());
        self
    }

    pub fn menu_width(mut self, width: impl Into<Length>) -> Self {
        self.menu_width = width.into();
        self
    }

    pub fn max_height(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = Some(height.into());
        self
    }

    pub fn anchor(mut self, anchor: Corner) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn mouse_button(mut self, button: MouseButton) -> Self {
        self.mouse_button = button;
        self
    }

    pub fn clear_on_confirm(mut self, clear: bool) -> Self {
        self.clear_on_confirm = clear;
        self
    }

    pub fn apply_on_confirm(mut self, apply: bool) -> Self {
        self.apply_on_confirm = apply;
        self
    }

    pub fn on_query_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&SharedString, &mut Window, &mut App) + 'static,
    {
        self.on_query_change = Some(Rc::new(callback));
        self
    }

    pub fn on_confirm<F>(mut self, callback: F) -> Self
    where
        F: Fn(&T, &mut Window, &mut App) + 'static,
    {
        self.on_confirm = Some(Rc::new(callback));
        self
    }

    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_open_change = Some(Rc::new(callback));
        self
    }

    pub fn on_select_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(Option<usize>, &mut Window, &mut App) + 'static,
    {
        self.on_select_change = Some(Rc::new(callback));
        self
    }

    pub fn input<F>(mut self, builder: F) -> Self
    where
        F: Fn(&Entity<InputState>) -> Input + 'static,
    {
        self.input_builder = Some(Rc::new(builder));
        self
    }

    pub fn render_item<F, E>(mut self, builder: F) -> Self
    where
        F: Fn(&T, bool, &mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.render_item = Some(Rc::new(move |item, selected, window, cx| {
            builder(item, selected, window, cx).into_any_element()
        }));
        self
    }

    fn resolved_corner(anchor: Corner, bounds: Bounds<Pixels>) -> gpui::Point<Pixels> {
        bounds.corner(match anchor {
            Corner::TopLeft => Corner::BottomLeft,
            Corner::TopRight => Corner::BottomRight,
            Corner::BottomLeft => Corner::TopLeft,
            Corner::BottomRight => Corner::TopRight,
        }) + gpui::Point {
            x: px(0.),
            y: -bounds.size.height,
        }
    }
}

impl<T: InputSuggestionItem + 'static> Styled for InputSuggestion<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: InputSuggestionItem + 'static> RenderOnce for InputSuggestion<T> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.clone();
        let input_state = state.read(cx).input_state().clone();

        let items = self.items;
        let enabled = self.enabled;
        let on_query_change = self.on_query_change.clone();
        let on_confirm = self.on_confirm.clone();
        let on_open_change = self.on_open_change.clone();
        let on_select_change = self.on_select_change.clone();
        let clear_on_confirm = self.clear_on_confirm;
        let apply_on_confirm = self.apply_on_confirm;

        state.update(cx, |state, cx| {
            if let Some(items) = items {
                state.set_items(items, window, cx);
            }
            if let Some(enabled) = enabled {
                state.set_enabled(enabled, window, cx);
            }
            state.set_callbacks(
                on_query_change,
                on_confirm,
                on_open_change,
                on_select_change,
            );
            state.set_clear_on_confirm(clear_on_confirm);
            state.set_apply_on_confirm(apply_on_confirm);
        });

        let (items, selected, open, bounds) = {
            let state = state.read(cx);
            (
                state.items.clone(),
                state.selected,
                state.open,
                state.input_bounds,
            )
        };

        let input = if let Some(builder) = self.input_builder.as_ref() {
            builder(&input_state)
        } else {
            Input::new(&input_state)
        };

        let list = {
            let mut list = v_flex().gap_1().w_full();
            if let Some(header) = self.header.clone() {
                list = list.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(header),
                );
            }
            let item_count = items.len();
            let list = list.children(items.into_iter().enumerate().map(|(ix, item)| {
                let state_for_hover = state.clone();
                let state_for_click = state.clone();
                let render_item = self.render_item.clone();
                let selected = selected == Some(ix);
                let content = if let Some(render_item) = render_item.as_ref() {
                    render_item(&item, selected, window, cx)
                } else {
                    item.label().into_any_element()
                };

                ListItem::new(ElementId::NamedChild(
                    Arc::new(self.id.clone()),
                    format!("item-{ix}").into(),
                ))
                .selected(selected)
                .w_full()
                .text_sm()
                .on_mouse_enter(move |_, window, cx| {
                    let state = state_for_hover.clone();
                    state.update(cx, |state, cx| {
                        state.set_selected_index(Some(ix), window, cx);
                    });
                })
                .on_click(move |_: &ClickEvent, window, cx| {
                    let state = state_for_click.clone();
                    state.update(cx, |state, cx| {
                        state.confirm_index(ix, window, cx);
                    });
                })
                .when(ix + 1 < item_count, |item| {
                    item.border_b_1().border_color(cx.theme().border)
                })
                .child(content)
            }));

            if let Some(height) = self.max_height {
                list.max_h(height).overflow_y_scrollbar().into_any_element()
            } else {
                list.into_any_element()
            }
        };

        let popover = if open {
            bounds.map(|bounds| {
                let mut content = v_flex()
                    .occlude()
                    .popover_style(cx)
                    .p_2()
                    .when_some(self.max_height, |this, height| this.max_h(height))
                    .child(list)
                    .refine_style(&self.style);

                content = match self.menu_width {
                    Length::Auto => content.w(bounds.size.width),
                    Length::Definite(width) => content.w(width),
                };

                let content = match self.anchor {
                    Corner::TopLeft | Corner::TopRight => content.top_1(),
                    Corner::BottomLeft | Corner::BottomRight => content.bottom_1(),
                };

                let position = Self::resolved_corner(self.anchor, bounds);
                let mouse_button = self.mouse_button;
                let content = content.on_mouse_up_out(mouse_button, {
                    let state = state.clone();
                    move |_, window, cx| {
                        state.update(cx, |state, cx| {
                            state.open = false;
                            state.emit_open_change(window, cx);
                            cx.notify();
                        });
                    }
                });

                deferred(
                    anchored()
                        .snap_to_window_with_margin(px(8.))
                        .anchor(self.anchor)
                        .position(position)
                        .child(content),
                )
                .with_priority(1)
                .into_any_element()
            })
        } else {
            None
        };

        div()
            .id(self.id.clone())
            .child(
                div()
                    .on_prepaint({
                        let state = state.clone();
                        move |bounds, _, cx| {
                            state.update(cx, |state, _| {
                                state.input_bounds = Some(bounds);
                            });
                        }
                    })
                    .child(input),
            )
            .children(popover)
    }
}
