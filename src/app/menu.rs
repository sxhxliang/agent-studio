use crate::app::actions::{Copy, Cut, Info, Paste, SearchAll, ToggleCheck};
use crate::panels::dock_panel::DockPanel;
use crate::panels::dock_panel::section;
use gpui::{
    App, AppContext, Context, Corner, Entity, InteractiveElement, IntoElement, KeyBinding,
    ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use gpui_component::{
    ActiveTheme as _, IconName, StyledExt,
    button::Button,
    h_flex,
    menu::{ContextMenuExt, DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use rust_i18n::t;

const CONTEXT: &str = "menu";
pub fn init(cx: &mut App) {
    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-f", SearchAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-f", SearchAll, Some(CONTEXT)),
    ])
}

pub struct UIMenu {
    checked: bool,
    message: String,
}

impl DockPanel for UIMenu {
    fn title() -> &'static str {
        "Menu"
    }

    fn description() -> &'static str {
        "Popup menu and context menu"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl UIMenu {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            checked: true,
            message: "".to_string(),
        }
    }

    fn on_copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.message = t!("menu.message.copy").to_string();
        cx.notify()
    }

    fn on_cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        self.message = t!("menu.message.cut").to_string();
        cx.notify()
    }

    fn on_paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        self.message = t!("menu.message.paste").to_string();
        cx.notify()
    }

    fn on_search_all(&mut self, _: &SearchAll, _: &mut Window, cx: &mut Context<Self>) {
        self.message = t!("menu.message.search_all").to_string();
        cx.notify()
    }

    fn on_action_info(&mut self, info: &Info, _: &mut Window, cx: &mut Context<Self>) {
        self.message = t!("menu.message.info", info = info.0).to_string();
        cx.notify()
    }

    fn on_action_toggle_check(&mut self, _: &ToggleCheck, _: &mut Window, cx: &mut Context<Self>) {
        self.checked = !self.checked;
        self.message = t!("menu.message.toggle_check", checked = self.checked).to_string();
        cx.notify()
    }
}

impl Render for UIMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let checked = self.checked;
        let view = cx.entity();

        v_flex()
            .key_context(CONTEXT)
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_cut))
            .on_action(cx.listener(Self::on_paste))
            // .on_action(cx.listener(Self::on_search_all))
            // .on_action(cx.listener(Self::on_action_info))
            // .on_action(cx.listener(Self::on_action_toggle_check))
            .size_full()
            .min_h(px(400.))
            .gap_6()
            .child(
                section(t!("menu.section.popup").to_string())
                    .child(
                        Button::new("popup-menu-1")
                            .outline()
                            .label(t!("menu.button.edit").to_string())
                            .dropdown_menu(move |this, window, cx| {
                                this.link(
                                    t!("menu.link.about").to_string(),
                                    "https://github.com/sxhxliang",
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::new(
                                        t!("menu.item.handle_click").to_string(),
                                    )
                                    .on_click(
                                        window.listener_for(&view, |this, _, _, cx| {
                                            this.message =
                                                t!("menu.message.handle_click").to_string();
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .separator()
                                .menu(t!("menu.menu.copy").to_string(), Box::new(Copy))
                                .menu(t!("menu.menu.cut").to_string(), Box::new(Cut))
                                .menu(t!("menu.menu.paste").to_string(), Box::new(Paste))
                                .separator()
                                .menu_with_check(
                                    t!("menu.menu.toggle_check").to_string(),
                                    checked,
                                    Box::new(ToggleCheck),
                                )
                                .separator()
                                .menu_with_icon(
                                    t!("menu.menu.search").to_string(),
                                    IconName::Search,
                                    Box::new(SearchAll),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::element(|_, cx| {
                                        v_flex()
                                            .child(t!("menu.item.custom_element").to_string())
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(
                                                        t!("menu.item.sub_title").to_string(),
                                                    ),
                                            )
                                    })
                                    .on_click(
                                        window.listener_for(&view, |this, _, _, cx| {
                                            this.message =
                                                t!("menu.message.custom_element").to_string();
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .menu_element_with_check(checked, Box::new(Info(0)), |_, cx| {
                                    h_flex()
                                        .gap_1()
                                        .child(t!("menu.item.custom_element").to_string())
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(t!("menu.item.checked").to_string()),
                                        )
                                })
                                .menu_element_with_icon(
                                    IconName::Info,
                                    Box::new(Info(0)),
                                    |_, cx| {
                                        h_flex()
                                            .gap_1()
                                            .child(t!("menu.item.custom").to_string())
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(
                                                        t!("menu.item.element").to_string(),
                                                    ),
                                            )
                                    },
                                )
                                .separator()
                                .menu_with_disabled(
                                    t!("menu.item.disabled").to_string(),
                                    Box::new(Info(0)),
                                    true,
                                )
                                .separator()
                                .submenu(
                                    t!("menu.submenu.links").to_string(),
                                    window,
                                    cx,
                                    |menu, _, _| {
                                        menu.link_with_icon(
                                            t!("menu.link.gpui_component").to_string(),
                                            IconName::GitHub,
                                            "https://github.com/sxhxliang/agent-studio",
                                        )
                                        .separator()
                                        .link(
                                            t!("menu.link.gpui").to_string(),
                                            "https://gpui.rs",
                                        )
                                        .link(
                                            t!("menu.link.zed").to_string(),
                                            "https://zed.dev",
                                        )
                                    },
                                )
                                .separator()
                                .submenu(
                                    t!("menu.submenu.other_links").to_string(),
                                    window,
                                    cx,
                                    |menu, _, _| {
                                        menu.link(
                                            t!("menu.link.crates").to_string(),
                                            "https://crates.io",
                                        )
                                        .link(
                                            t!("menu.link.rust_docs").to_string(),
                                            "https://docs.rs",
                                        )
                                    },
                                )
                            }),
                    )
                    .child(self.message.clone()),
            )
            .child(
                section(t!("menu.section.context").to_string())
                    .v_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .w_full()
                            .p_4()
                            .items_center()
                            .justify_center()
                            .min_h_20()
                            .rounded_lg()
                            .border_2()
                            .border_dashed()
                            .border_color(cx.theme().border)
                            .child(t!("menu.context.open").to_string())
                            .context_menu({
                                move |this, window, cx| {
                                    this.external_link_icon(false)
                                        .link(
                                            t!("menu.link.about").to_string(),
                                            "https://github.com/sxhxliang/agent-studio",
                                        )
                                        .separator()
                                        .menu(t!("menu.menu.cut").to_string(), Box::new(Cut))
                                        .menu(
                                            t!("menu.menu.copy").to_string(),
                                            Box::new(Copy),
                                        )
                                        .menu(
                                            t!("menu.menu.paste").to_string(),
                                            Box::new(Paste),
                                        )
                                        .separator()
                                        .label(t!("menu.context.label").to_string())
                                        .menu_with_check(
                                            t!("menu.menu.toggle_check").to_string(),
                                            checked,
                                            Box::new(ToggleCheck),
                                        )
                                        .separator()
                                        .submenu(
                                            t!("menu.context.settings").to_string(),
                                            window,
                                            cx,
                                            move |menu, _, _| {
                                                menu.menu(
                                                    t!("menu.context.info_0").to_string(),
                                                    Box::new(Info(0)),
                                                )
                                                .separator()
                                                .menu(
                                                    t!("menu.context.item_1").to_string(),
                                                    Box::new(Info(1)),
                                                )
                                                .menu(
                                                    t!("menu.context.item_2").to_string(),
                                                    Box::new(Info(2)),
                                                )
                                            },
                                        )
                                        .separator()
                                        .menu(
                                            t!("menu.context.search_all").to_string(),
                                            Box::new(SearchAll),
                                        )
                                        .separator()
                                }
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("menu.context.hint").to_string()),
                            ),
                    )
                    .child(
                        div()
                            .id("other")
                            .flex()
                            .w_full()
                            .p_4()
                            .items_center()
                            .justify_center()
                            .min_h_20()
                            .rounded_lg()
                            .border_2()
                            .border_dashed()
                            .border_color(cx.theme().border)
                            .child(t!("menu.context.area_hint").to_string())
                            .context_menu({
                                move |this, _, _| {
                                    this.link(
                                        t!("menu.link.about").to_string(),
                                        "https://github.com/sxhxliang/agent-studio",
                                    )
                                    .separator()
                                    .menu(
                                        t!("menu.context.item_1").to_string(),
                                        Box::new(Info(1)),
                                    )
                                }
                            }),
                    ),
            )
            .child(
                section(t!("menu.section.scrollable").to_string())
                    .child(
                        Button::new("dropdown-menu-scrollable-1")
                            .outline()
                            .label(t!("menu.scrollable.button_100").to_string())
                            .dropdown_menu_with_anchor(Corner::TopRight, move |this, _, _| {
                                let mut this = this.scrollable(true).max_h(px(300.)).label(
                                    t!("menu.scrollable.total_items", count = 100)
                                        .to_string(),
                                );
                                for i in 0..100 {
                                    this = this.menu(
                                        SharedString::from(
                                            t!("menu.scrollable.item", index = i).to_string(),
                                        ),
                                        Box::new(Info(i)),
                                    )
                                }
                                this.min_w(px(100.))
                            }),
                    )
                    .child(
                        Button::new("dropdown-menu-scrollable-2")
                            .outline()
                            .label(t!("menu.scrollable.button_5").to_string())
                            .dropdown_menu_with_anchor(Corner::TopRight, move |this, _, _| {
                                let mut this = this.scrollable(true).max_h(px(300.)).label(
                                    t!("menu.scrollable.total_items", count = 100)
                                        .to_string(),
                                );
                                for i in 0..5 {
                                    this = this.menu(
                                        SharedString::from(
                                            t!("menu.scrollable.item", index = i).to_string(),
                                        ),
                                        Box::new(Info(i)),
                                    )
                                }
                                this.min_w(px(100.))
                            }),
                    ),
            )
    }
}
