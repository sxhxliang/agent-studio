use gpui::{
    px, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Pixels, Render, Styled, Window,
};

use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    v_flex, ActiveTheme, Icon, IconName, Sizable,
};

pub struct ChatInputPanel {
    focus_handle: FocusHandle,
    input_state: Entity<InputState>,
}

impl super::DockPanel for ChatInputPanel {
    fn title() -> &'static str {
        "Chat Input"
    }

    fn description() -> &'static str {
        "A chat input box for sending messages."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
    fn paddings() -> Pixels {
        px(0.)
    }
}

impl ChatInputPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut App) -> Self {
        let input_state = cx
            .new(|cx| InputState::new(window, cx).placeholder("Ask, search, or make anything..."));

        Self {
            focus_handle: cx.focus_handle(),
            input_state,
        }
    }
}

impl Focusable for ChatInputPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatInputPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_4()
            .p_4()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .p_4()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .shadow_lg()
                    .child(
                        // Top row: Add context button
                        h_flex().w_full().child(
                            Button::new("add-context")
                                .label("Add context")
                                .icon(Icon::new(IconName::Asterisk))
                                .ghost()
                                .small(),
                        ),
                    )
                    .child(
                        // Input area
                        Input::new(&self.input_state)
                            .appearance(false)
                            .cleanable(true),
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
                                    .child(Button::new("auto").label("Auto").ghost().small())
                                    .child(
                                        Button::new("sources")
                                            .label("All Sources")
                                            .icon(Icon::new(IconName::Globe))
                                            .ghost()
                                            .small(),
                                    ),
                            )
                            .child(
                                Button::new("send")
                                    .icon(Icon::new(IconName::ArrowUp))
                                    .primary()
                                    .rounded_full()
                                    .small(),
                            ),
                    ),
            )
    }
}
