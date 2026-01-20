use agent_client_protocol::ToolCall;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, IconName, WindowExt,
    button::Button,
    dock::{Panel, PanelControl, PanelEvent, PanelInfo, PanelState, TitleStyle},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    menu::PopupMenu,
    notification::Notification,
};

use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::panels::{
    CodeEditorPanel, ConversationPanel, SessionManagerPanel, SettingsPanel, TaskPanel,
    TerminalPanel, ToolCallDetailPanel, WelcomePanel,
};
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
    pub title_key: Option<SharedString>,
    pub title_bg: Option<Hsla>,
    pub description: SharedString,
    pub width: Option<gpui::Pixels>,
    pub height: Option<gpui::Pixels>,
    pub agent_studio: Option<AnyView>,
    pub agent_studio_klass: Option<SharedString>,
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

    fn title_key() -> Option<&'static str> {
        None
    }

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
        if let Some(agent_studio) = view.downcast::<Self>().ok() {
            cx.update_entity(&agent_studio, |agent_studio, cx| {
                agent_studio.on_active(active, window, cx);
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
            title_key: None,
            title_bg: None,
            description: "".into(),
            width: None,
            height: None,
            agent_studio: None,
            agent_studio_klass: None,
            closable: true,
            zoomable: Some(PanelControl::default()),
            paddings: px(0.0),
            on_active: None,
        }
    }

    pub fn panel<S: DockPanel>(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let name = S::title();
        let title_key = S::title_key();
        let description = S::description();
        let agent_studio = S::new_view(window, cx);
        let agent_studio_klass = S::klass();
        log::debug!("=====>>> Panel: {}, paddings: {}", name, S::paddings());
        let view = cx.new(|cx| {
            let mut agent_studio = Self::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(S::on_active_any);
            agent_studio.focus_handle = cx.focus_handle();
            agent_studio.closable = S::closable();
            agent_studio.zoomable = S::zoomable();
            agent_studio.name = name.into();
            agent_studio.title_key = title_key.map(SharedString::from);
            agent_studio.description = description.into();
            agent_studio.title_bg = S::title_bg();
            agent_studio.paddings = S::paddings();
            agent_studio
        });

        view
    }

    pub fn panel_for_tool_call_detail(
        tool_call: ToolCall,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = ToolCallDetailPanel::title();
        let title_key = ToolCallDetailPanel::title_key();
        let description = ToolCallDetailPanel::description();
        let mut agent_studio = ToolCallDetailPanel::new(window, cx);
        let agent_studio_klass = ToolCallDetailPanel::klass();
        agent_studio.set_tool_call(tool_call);

        let entity = cx.new(|_cx| agent_studio);

        let view = cx.new(|cx| {
            let mut container = Self::new(cx)
                .agent_studio(entity.into(), agent_studio_klass)
                .on_active(ToolCallDetailPanel::on_active_any);
            container.focus_handle = cx.focus_handle();
            container.closable = ToolCallDetailPanel::closable();
            container.zoomable = ToolCallDetailPanel::zoomable();
            container.name = name.into();
            container.title_key = title_key.map(SharedString::from);
            container.description = description.into();
            container.title_bg = ToolCallDetailPanel::title_bg();
            container.paddings = ToolCallDetailPanel::paddings();
            container
        });

        view
    }
    /// Create a panel for a specific session (currently only supports ConversationPanel)
    /// This will load the conversation hiagent_studio for that session
    pub fn panel_for_session(
        session_id: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = ConversationPanel::title();
        let title_key = ConversationPanel::title_key();
        let description = ConversationPanel::description();
        let agent_studio = ConversationPanel::view_for_session(session_id, window, cx);
        let agent_studio_klass = ConversationPanel::klass();

        let view = cx.new(|cx| {
            let mut container = Self::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(ConversationPanel::on_active_any);
            container.focus_handle = cx.focus_handle();
            container.closable = ConversationPanel::closable();
            container.zoomable = ConversationPanel::zoomable();
            container.name = name.into();
            container.title_key = title_key.map(SharedString::from);
            container.description = description.into();
            container.title_bg = ConversationPanel::title_bg();
            container.paddings = ConversationPanel::paddings();
            container
        });

        view
    }

    pub fn replace_with_conversation_session(
        &mut self,
        session_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = ConversationPanel::title();
        let title_key = ConversationPanel::title_key();
        let description = ConversationPanel::description();
        let agent_studio = match session_id {
            Some(session_id) => ConversationPanel::view_for_session(session_id, window, cx),
            None => ConversationPanel::view(window, cx),
        };
        let agent_studio_klass = ConversationPanel::klass();

        self.agent_studio = Some(agent_studio.into());
        self.agent_studio_klass = Some(agent_studio_klass.into());
        self.on_active = Some(ConversationPanel::on_active_any);
        self.closable = ConversationPanel::closable();
        self.zoomable = ConversationPanel::zoomable();
        self.name = name.into();
        self.title_key = title_key.map(SharedString::from);
        self.description = description.into();
        self.title_bg = ConversationPanel::title_bg();
        self.paddings = ConversationPanel::paddings();
        cx.notify();
    }

    /// Create a WelcomePanel for a specific workspace
    /// This will display the workspace name when creating a new task
    pub fn panel_for_workspace(
        workspace_id: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = WelcomePanel::title();
        let title_key = WelcomePanel::title_key();
        let description = WelcomePanel::description();
        let agent_studio = WelcomePanel::view_for_workspace(workspace_id, window, cx);
        let agent_studio_klass = WelcomePanel::klass();

        let view = cx.new(|cx| {
            let mut container = Self::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(WelcomePanel::on_active_any);
            container.focus_handle = cx.focus_handle();
            container.closable = WelcomePanel::closable();
            container.zoomable = WelcomePanel::zoomable();
            container.name = name.into();
            container.title_key = title_key.map(SharedString::from);
            container.description = description.into();
            container.title_bg = WelcomePanel::title_bg();
            container.paddings = WelcomePanel::paddings();
            container
        });

        view
    }

    fn panel_for_welcome_with_dir(
        workspace_id: Option<String>,
        working_directory: std::path::PathBuf,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = WelcomePanel::title();
        let title_key = WelcomePanel::title_key();
        let description = WelcomePanel::description();
        let agent_studio =
            WelcomePanel::view_with_workspace_and_dir(workspace_id, working_directory, window, cx);
        let agent_studio_klass = WelcomePanel::klass();

        let view = cx.new(|cx| {
            let mut container = Self::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(WelcomePanel::on_active_any);
            container.focus_handle = cx.focus_handle();
            container.closable = WelcomePanel::closable();
            container.zoomable = WelcomePanel::zoomable();
            container.name = name.into();
            container.title_key = title_key.map(SharedString::from);
            container.description = description.into();
            container.title_bg = WelcomePanel::title_bg();
            container.paddings = WelcomePanel::paddings();
            container
        });

        view
    }

    /// 创建带指定工作目录的终端面板
    pub fn panel_for_terminal_with_cwd(
        working_directory: std::path::PathBuf,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        use crate::TerminalPanel;

        let name = TerminalPanel::title();
        let title_key = TerminalPanel::title_key();
        let description = TerminalPanel::description();
        let agent_studio = TerminalPanel::view_with_cwd(working_directory, window, cx);
        let agent_studio_klass = TerminalPanel::klass();

        let view = cx.new(|cx| {
            let mut container = Self::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(TerminalPanel::on_active_any);
            container.focus_handle = cx.focus_handle();
            container.closable = TerminalPanel::closable();
            container.zoomable = TerminalPanel::zoomable();
            container.name = name.into();
            container.title_key = title_key.map(SharedString::from);
            container.description = description.into();
            container.title_bg = TerminalPanel::title_bg();
            container.paddings = TerminalPanel::paddings();
            container
        });

        view
    }

    pub fn panel_from_state(
        agent_state: &DockPanelState,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        fn canonicalize_path(path: &str) -> Option<std::path::PathBuf> {
            std::path::PathBuf::from(path).canonicalize().ok()
        }

        match agent_state.agent_studio_klass.as_ref() {
            "TaskPanel" => Self::panel::<TaskPanel>(window, cx),
            "SessionManagerPanel" => Self::panel::<SessionManagerPanel>(window, cx),
            "SettingsPanel" => Self::panel::<SettingsPanel>(window, cx),
            "ToolCallDetailPanel" => Self::panel::<ToolCallDetailPanel>(window, cx),
            "ConversationPanel" => {
                if let Some(session_id) = agent_state
                    .session_id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                {
                    Self::panel_for_session(session_id.to_string(), window, cx)
                } else {
                    Self::panel::<ConversationPanel>(window, cx)
                }
            }
            "CodeEditorPanel" => {
                if let Some(working_dir) = agent_state
                    .working_directory
                    .as_deref()
                    .filter(|path| !path.is_empty())
                    .and_then(canonicalize_path)
                {
                    Self::panel_for_code_editor_with_cwd(working_dir, window, cx)
                } else {
                    Self::panel::<CodeEditorPanel>(window, cx)
                }
            }
            "TerminalPanel" => {
                if let Some(working_dir) = agent_state
                    .working_directory
                    .as_deref()
                    .filter(|path| !path.is_empty())
                    .and_then(canonicalize_path)
                {
                    Self::panel_for_terminal_with_cwd(working_dir, window, cx)
                } else {
                    Self::panel::<TerminalPanel>(window, cx)
                }
            }
            "WelcomePanel" => {
                let working_dir = agent_state
                    .working_directory
                    .as_deref()
                    .filter(|path| !path.is_empty())
                    .and_then(canonicalize_path);
                let workspace_id = agent_state.workspace_id.clone();

                match (workspace_id, working_dir) {
                    (Some(workspace_id), Some(working_dir)) => Self::panel_for_welcome_with_dir(
                        Some(workspace_id),
                        working_dir,
                        window,
                        cx,
                    ),
                    (Some(workspace_id), None) => {
                        Self::panel_for_workspace(workspace_id, window, cx)
                    }
                    (None, Some(working_dir)) => {
                        Self::panel_for_welcome_with_dir(None, working_dir, window, cx)
                    }
                    (None, None) => Self::panel::<WelcomePanel>(window, cx),
                }
            }
            _ => {
                unreachable!("Invalid agent_studio klass: {}", agent_state.agent_studio_klass.as_ref());
            }
        }
    }

    /// 创建带指定工作目录的代码编辑器面板
    pub fn panel_for_code_editor_with_cwd(
        working_directory: std::path::PathBuf,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        use crate::CodeEditorPanel;

        let name = CodeEditorPanel::title();
        let title_key = CodeEditorPanel::title_key();
        let description = CodeEditorPanel::description();
        let agent_studio = CodeEditorPanel::view_with_working_dir(window, Some(working_directory), cx);
        let agent_studio_klass = CodeEditorPanel::klass();

        let view = cx.new(|cx| {
            let mut container = Self::new(cx)
                .agent_studio(agent_studio.into(), agent_studio_klass)
                .on_active(CodeEditorPanel::on_active_any);
            container.focus_handle = cx.focus_handle();
            container.closable = CodeEditorPanel::closable();
            container.zoomable = CodeEditorPanel::zoomable();
            container.name = name.into();
            container.title_key = title_key.map(SharedString::from);
            container.description = description.into();
            container.title_bg = CodeEditorPanel::title_bg();
            container.paddings = CodeEditorPanel::paddings();
            container
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

    pub fn agent_studio(mut self, agent_studio: AnyView, agent_studio_klass: impl Into<SharedString>) -> Self {
        self.agent_studio = Some(agent_studio);
        self.agent_studio_klass = Some(agent_studio_klass.into());
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
    pub agent_studio_klass: SharedString,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub workspace_name: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
}

impl DockPanelState {
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "agent_studio_klass": self.agent_studio_klass,
            "session_id": self.session_id,
            "workspace_id": self.workspace_id,
            "workspace_name": self.workspace_name,
            "working_directory": self.working_directory,
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

    fn title(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<'_, DockPanelContainer>,
    ) -> impl gpui::IntoElement {
        let title = if let Some(key) = &self.title_key {
            SharedString::from(t!(key.as_ref()).to_string())
        } else {
            self.name.clone()
        };
        title.into_any_element()
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

    fn set_zoomed(
        &mut self,
        zoomed: bool,
        _window: &mut Window,
        _cx: &mut gpui::Context<'_, DockPanelContainer>,
    ) {
        println!("panel: {} zoomed: {}", self.name, zoomed);
    }

    fn set_active(
        &mut self,
        active: bool,
        _window: &mut Window,
        cx: &mut gpui::Context<'_, DockPanelContainer>,
    ) {
        println!("panel: {} active: {}", self.name, active);
        if let Some(on_active) = self.on_active {
            if let Some(agent_studio) = self.agent_studio.clone() {
                on_active(agent_studio, active, _window, cx);
            }
        }
    }

    // fn dropdown_menu(
    //     &mut self,
    //     menu: PopupMenu,
    //     _window: &mut gpui::Window,
    //     _cx: &mut gpui::Context<'_, DockPanelContainer>,
    // ) -> PopupMenu {
    //     menu.menu("Info", Box::new(ShowPanelInfo))
    // }

    fn toolbar_buttons(
        &mut self,
        _window: &mut Window,
        _cx: &mut gpui::Context<'_, DockPanelContainer>,
    ) -> Option<Vec<Button>> {
        Some(vec![
            // Button::new("info")
            //     .icon(IconName::Info)
            //     .on_click(|_, window, cx| {
            //         window.push_notification("You have clicked info button", cx);
            //     }),
            // Button::new("search")
            //     .icon(IconName::Search)
            //     .on_click(|_, window, cx| {
            //         window.push_notification("You have clicked search button", cx);
            //     }),
        ])
    }

    fn dump(&self, cx: &App) -> PanelState {
        let mut state = PanelState::new(self);

        let agent_studio_klass = self.agent_studio_klass.clone().unwrap();
        let mut session_id = None;
        let mut workspace_id = None;
        let mut workspace_name = None;
        let mut working_directory = None;

        // Helper function to normalize Windows paths (remove \\?\ prefix)
        fn normalize_path(path: std::path::PathBuf) -> String {
            let path_str = path.to_string_lossy().to_string();
            // Remove Windows extended-length path prefix
            if cfg!(windows) && path_str.starts_with(r"\\?\") {
                path_str.trim_start_matches(r"\\?\").to_string()
            } else {
                path_str
            }
        }

        if let Some(agent_studio) = &self.agent_studio {
            match agent_studio_klass.as_ref() {
                "ConversationPanel" => {
                    if let Ok(entity) = agent_studio.clone().downcast::<ConversationPanel>() {
                        let panel = entity.read(cx);
                        session_id = panel.session_id();
                        workspace_id = panel.workspace_id();
                        workspace_name = panel.workspace_name();
                        working_directory = panel.working_directory();
                    }
                }
                "WelcomePanel" => {
                    if let Ok(entity) = agent_studio.clone().downcast::<WelcomePanel>() {
                        let panel = entity.read(cx);
                        workspace_id = panel.workspace_id();
                        workspace_name = panel.workspace_name();
                        working_directory = Some(normalize_path(panel.working_directory()));
                    }
                }
                "CodeEditorPanel" => {
                    if let Ok(entity) = agent_studio.clone().downcast::<CodeEditorPanel>() {
                        let panel = entity.read(cx);
                        workspace_id = panel.workspace_id();
                        workspace_name = panel.workspace_name();
                        working_directory = Some(normalize_path(panel.working_directory()));
                    }
                }
                "TerminalPanel" => {
                    if let Ok(entity) = agent_studio.clone().downcast::<TerminalPanel>() {
                        let panel = entity.read(cx);
                        workspace_id = panel.workspace_id();
                        workspace_name = panel.workspace_name();
                        working_directory = panel.working_directory().map(normalize_path);
                    }
                }
                _ => {}
            }
        }

        let agent_state = DockPanelState {
            agent_studio_klass,
            session_id,
            workspace_id,
            workspace_name,
            working_directory,
        };
        state.info = PanelInfo::panel(agent_state.to_value());
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
        div()
            .id("agent_studio-container")
            .size_full()
            .p(self.paddings)
            .track_focus(&self.focus_handle)
            // .on_action(cx.listener(Self::on_action_panel_info))
            // .on_action(cx.listener(Self::on_action_toggle_search))
            .when_some(self.agent_studio.clone(), |this, agent_studio| this.child(agent_studio))
    }
}
