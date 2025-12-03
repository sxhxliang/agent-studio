use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    button::Button,
    dock::{Panel, PanelControl, PanelEvent, PanelInfo, PanelState, TitleStyle},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    menu::PopupMenu,
    notification::Notification,
    v_flex, ActiveTheme, IconName, WindowExt,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::{ShowPanelInfo, ToggleSearch};

#[derive(IntoElement)]
pub struct DockPanelSection {
    base: Div,
    title: SharedString,
    sub_title: Vec<AnyElement>,
    children: Vec<AnyElement>,
}

impl DockPanelSection {
    pub fn sub_title(mut self, sub_title: impl IntoElement) -> Self {
        self.sub_title.push(sub_title.into_any_element());
        self
    }

    #[allow(unused)]
    pub fn max_w_md(mut self) -> Self {
        self.base = self.base.max_w(rems(48.));
        self
    }

    #[allow(unused)]
    pub fn max_w_lg(mut self) -> Self {
        self.base = self.base.max_w(rems(64.));
        self
    }

    #[allow(unused)]
    pub fn max_w_xl(mut self) -> Self {
        self.base = self.base.max_w(rems(80.));
        self
    }

    #[allow(unused)]
    pub fn max_w_2xl(mut self) -> Self {
        self.base = self.base.max_w(rems(96.));
        self
    }
}

impl ParentElement for DockPanelSection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for DockPanelSection {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for DockPanelSection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        GroupBox::new()
            .id(self.title.clone())
            .outline()
            .title(
                h_flex()
                    .justify_between()
                    .w_full()
                    .gap_4()
                    .child(self.title)
                    .children(self.sub_title),
            )
            .content_style(
                StyleRefinement::default()
                    .rounded(cx.theme().radius_lg)
                    .overflow_x_hidden()
                    .items_center()
                    .justify_center(),
            )
            .child(self.base.children(self.children))
    }
}

pub fn section(title: impl Into<SharedString>) -> DockPanelSection {
    DockPanelSection {
        title: title.into(),
        sub_title: vec![],
        base: h_flex()
            .flex_wrap()
            .justify_center()
            .items_center()
            .w_full()
            .gap_4(),
        children: vec![],
    }
}

pub struct DockPanelContainer {
    pub focus_handle: gpui::FocusHandle,
    pub name: SharedString,
    pub title_bg: Option<Hsla>,
    pub description: SharedString,
    pub width: Option<gpui::Pixels>,
    pub height: Option<gpui::Pixels>,
    pub story: Option<AnyView>,
    pub story_klass: Option<SharedString>,
    pub closable: bool,
    pub zoomable: Option<PanelControl>,
    pub paddings: Pixels,
    pub on_active: Option<fn(AnyView, bool, &mut Window, &mut App)>,
}

#[derive(Debug)]
pub enum ContainerEvent {
    Close,
}

pub trait DockPanel: Render + Sized {
    fn klass() -> &'static str {
        std::any::type_name::<Self>().split("::").last().unwrap()
    }

    fn title() -> &'static str;

    fn description() -> &'static str {
        ""
    }

    fn closable() -> bool {
        true
    }

    fn zoomable() -> Option<PanelControl> {
        Some(PanelControl::default())
    }

    fn title_bg() -> Option<Hsla> {
        None
    }

    fn paddings() -> Pixels {
        px(0.)
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render>;

    fn on_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
        let _ = active;
        let _ = window;
        let _ = cx;
    }

    fn on_active_any(view: AnyView, active: bool, window: &mut Window, cx: &mut App)
    where
        Self: 'static,
    {
        if let Some(story) = view.downcast::<Self>().ok() {
            cx.update_entity(&story, |story, cx| {
                story.on_active(active, window, cx);
            });
        }
    }
}

impl EventEmitter<ContainerEvent> for DockPanelContainer {}

impl DockPanelContainer {
    pub fn new(cx: &mut App) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            name: "".into(),
            title_bg: None,
            description: "".into(),
            width: None,
            height: None,
            story: None,
            story_klass: None,
            closable: true,
            zoomable: Some(PanelControl::default()),
            paddings: px(0.0),
            on_active: None,
        }
    }

    pub fn panel<S: DockPanel>(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let name = S::title();
        let description = S::description();
        let story = S::new_view(window, cx);
        let story_klass = S::klass();
        log::debug!("Panel: {}, paddings: {}", name, S::paddings());
        let view = cx.new(|cx| {
            let mut story = Self::new(cx)
                .story(story.into(), story_klass)
                .on_active(S::on_active_any);
            story.focus_handle = cx.focus_handle();
            story.closable = S::closable();
            story.zoomable = S::zoomable();
            story.name = name.into();
            story.description = description.into();
            story.title_bg = S::title_bg();
            story.paddings = S::paddings();
            story
        });

        view
    }

    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = Some(height);
        self
    }

    pub fn story(mut self, story: AnyView, story_klass: impl Into<SharedString>) -> Self {
        self.story = Some(story);
        self.story_klass = Some(story_klass.into());
        self
    }

    pub fn on_active(mut self, on_active: fn(AnyView, bool, &mut Window, &mut App)) -> Self {
        self.on_active = Some(on_active);
        self
    }

    fn on_action_panel_info(
        &mut self,
        _: &ShowPanelInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        struct Info;
        let note = Notification::new()
            .message(format!("You have clicked panel info on: {}", self.name))
            .id::<Info>();
        window.push_notification(note, cx);
    }

    fn on_action_toggle_search(
        &mut self,
        _: &ToggleSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.propagate();
        if window.has_focused_input(cx) {
            return;
        }

        struct Search;
        let note = Notification::new()
            .message(format!("You have toggled search on: {}", self.name))
            .id::<Search>();
        window.push_notification(note, cx);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DockPanelState {
    pub story_klass: SharedString,
}

impl DockPanelState {
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "story_klass": self.story_klass,
        })
    }

    pub fn from_value(value: serde_json::Value) -> Self {
        serde_json::from_value(value).unwrap()
    }
}

impl Panel for DockPanelContainer {
    fn panel_name(&self) -> &'static str {
        "DockPanelContainer"
    }

    fn title(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<'_, DockPanelContainer>) -> impl gpui::IntoElement {
        self.name.clone().into_any_element()
    }

    fn title_style(&self, cx: &App) -> Option<TitleStyle> {
        if let Some(bg) = self.title_bg {
            Some(TitleStyle {
                background: bg,
                foreground: cx.theme().foreground,
            })
        } else {
            None
        }
    }

    fn closable(&self, _cx: &App) -> bool {
        self.closable
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
        self.zoomable
    }

    fn visible(&self, cx: &App) -> bool {
        !AppState::global(cx)
            .invisible_panels
            .read(cx)
            .contains(&self.name)
    }

    fn set_zoomed(&mut self, zoomed: bool, _window: &mut Window, _cx: &mut gpui::Context<'_, DockPanelContainer>) {
        println!("panel: {} zoomed: {}", self.name, zoomed);
    }

    fn set_active(&mut self, active: bool, _window: &mut Window, cx: &mut gpui::Context<'_, DockPanelContainer>) {
        println!("panel: {} active: {}", self.name, active);
        if let Some(on_active) = self.on_active {
            if let Some(story) = self.story.clone() {
                on_active(story, active, _window, cx);
            }
        }
    }

    fn dropdown_menu(&mut self, menu: PopupMenu, _window: &mut gpui::Window, _cx: &mut gpui::Context<'_, DockPanelContainer>) -> PopupMenu {
        menu.menu("Info", Box::new(ShowPanelInfo))
    }

    fn toolbar_buttons(&mut self, _window: &mut Window, _cx: &mut gpui::Context<'_, DockPanelContainer>) -> Option<Vec<Button>> {
        Some(vec![
            Button::new("info")
                .icon(IconName::Info)
                .on_click(|_, window, cx| {
                    window.push_notification("You have clicked info button", cx);
                }),
            Button::new("search")
                .icon(IconName::Search)
                .on_click(|_, window, cx| {
                    window.push_notification("You have clicked search button", cx);
                }),
        ])
    }

    fn dump(&self, _cx: &App) -> PanelState {
        let mut state = PanelState::new(self);
        let story_state = DockPanelState {
            story_klass: self.story_klass.clone().unwrap(),
        };
        state.info = PanelInfo::panel(story_state.to_value());
        state
    }
}

impl EventEmitter<PanelEvent> for DockPanelContainer {}
impl Focusable for DockPanelContainer {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
impl Render for DockPanelContainer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("story-container")
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_action_panel_info))
            .on_action(cx.listener(Self::on_action_toggle_search))
            .when_some(self.story.clone(), |this, story| {
                this.child(
                    v_flex()
                        .id("story-children")
                        .w_full()
                        .flex_1()
                        .p(self.paddings)
                        .child(story),
                )
            })
    }
}
