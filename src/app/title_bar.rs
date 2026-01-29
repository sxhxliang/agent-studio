use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Corner, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Render, SharedString, Styled as _, Subscription,
    Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme as _, IconName, PixelsExt, Side, Sizable as _, Theme, TitleBar, WindowExt as _,
    badge::Badge,
    button::{Button, ButtonVariants as _},
    menu::AppMenuBar,
    menu::DropdownMenu as _,
    scroll::ScrollbarShow,
};
use rust_i18n::t;

use crate::{AppState, SelectFont, SelectRadius, SelectScrollbarShow, app_menus};

actions!(title_bar, [OpenSettings]);

pub struct AppTitleBar {
    app_menu_bar: Entity<AppMenuBar>,
    font_size_selector: Entity<FontSizeSelector>,
    child: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
    _subscriptions: Vec<Subscription>,
}

impl AppTitleBar {
    pub fn new(
        title: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = title.into();
        app_menus::init(title.clone(), cx);
        AppState::global_mut(cx).set_app_title(title);

        let font_size_selector = cx.new(|cx| FontSizeSelector::new(window, cx));
        let app_menu_bar = AppMenuBar::new(cx);

        Self {
            app_menu_bar,
            font_size_selector,
            child: Rc::new(|_, _| div().into_any_element()),
            _subscriptions: vec![],
        }
    }

    pub fn child<F, E>(mut self, f: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.child = Rc::new(move |window, cx| f(window, cx).into_any_element());
        self
    }

    fn on_action_open_settings(
        &mut self,
        _: &OpenSettings,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // SettingsWindow::open(window, cx).detach();
    }
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notifications_count = window.notifications(cx).len();

        div()
            .on_action(cx.listener(Self::on_action_open_settings))
            .child(
                TitleBar::new()
                    // left side
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .when(!cfg!(target_os = "macos"), |this| {
                                this.child(self.app_menu_bar.clone())
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .px_2()
                            .gap_2()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child((self.child.clone())(window, cx))
                            .child(self.font_size_selector.clone())
                            .child(
                                Button::new("settings-btn")
                                    .icon(IconName::Settings)
                                    .small()
                                    .ghost()
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(OpenSettings), cx);
                                    }),
                            ),
                    ),
            )
    }
}

struct FontSizeSelector {
    focus_handle: FocusHandle,
}

impl FontSizeSelector {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    fn on_select_font(
        &mut self,
        font_size: &SelectFont,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::panels::AppSettings;

        // Only update AppSettings - Theme will auto-sync via observe_global
        AppSettings::global_mut(cx).font_size = font_size.0 as f64;
        window.refresh();
    }

    fn on_select_radius(
        &mut self,
        radius: &SelectRadius,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Theme::global_mut(cx).radius = px(radius.0 as f32);
        window.refresh();
    }

    fn on_select_scrollbar_show(
        &mut self,
        show: &SelectScrollbarShow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Theme::global_mut(cx).scrollbar_show = show.0;
        window.refresh();
    }
}

impl Render for FontSizeSelector {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::panels::AppSettings;

        let focus_handle = self.focus_handle.clone();
        let font_size = AppSettings::global(cx).font_size as i32;
        let radius = cx.theme().radius.as_f32() as i32;
        let scroll_show = cx.theme().scrollbar_show;

        div()
            .id("font-size-selector")
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::on_select_font))
            .on_action(cx.listener(Self::on_select_radius))
            .on_action(cx.listener(Self::on_select_scrollbar_show))
            .child(
                Button::new("btn")
                    .small()
                    .ghost()
                    .icon(IconName::Settings2)
                    .dropdown_menu(move |this, _, _| {
                        this.scrollable(true)
                            .check_side(Side::Right)
                            .max_h(px(480.))
                            .label(t!("title_bar.font_size.label").to_string())
                            .menu_with_check(
                                t!("title_bar.font_size.large").to_string(),
                                font_size == 18,
                                Box::new(SelectFont(18)),
                            )
                            .menu_with_check(
                                t!("title_bar.font_size.medium_default").to_string(),
                                font_size == 16,
                                Box::new(SelectFont(16)),
                            )
                            .menu_with_check(
                                t!("title_bar.font_size.small").to_string(),
                                font_size == 14,
                                Box::new(SelectFont(14)),
                            )
                        // .separator()
                        // .label(t!("title_bar.border_radius.label").to_string())
                        // .menu_with_check(
                        //     t!("title_bar.border_radius.8px").to_string(),
                        //     radius == 8,
                        //     Box::new(SelectRadius(8)),
                        // )
                        // .menu_with_check(
                        //     t!("title_bar.border_radius.6px_default").to_string(),
                        //     radius == 6,
                        //     Box::new(SelectRadius(6)),
                        // )
                        // .menu_with_check(
                        //     t!("title_bar.border_radius.4px").to_string(),
                        //     radius == 4,
                        //     Box::new(SelectRadius(4)),
                        // )
                        // .menu_with_check(
                        //     t!("title_bar.border_radius.0px").to_string(),
                        //     radius == 0,
                        //     Box::new(SelectRadius(0)),
                        // )
                        // .separator()
                        // .label(t!("title_bar.scrollbar.label").to_string())
                        // .menu_with_check(
                        //     t!("title_bar.scrollbar.scrolling").to_string(),
                        //     scroll_show == ScrollbarShow::Scrolling,
                        //     Box::new(SelectScrollbarShow(ScrollbarShow::Scrolling)),
                        // )
                        // .menu_with_check(
                        //     t!("title_bar.scrollbar.hover").to_string(),
                        //     scroll_show == ScrollbarShow::Hover,
                        //     Box::new(SelectScrollbarShow(ScrollbarShow::Hover)),
                        // )
                        // .menu_with_check(
                        //     t!("title_bar.scrollbar.always").to_string(),
                        //     scroll_show == ScrollbarShow::Always,
                        //     Box::new(SelectScrollbarShow(ScrollbarShow::Always)),
                        // )
                    })
                    .anchor(Corner::TopRight),
            )
    }
}
