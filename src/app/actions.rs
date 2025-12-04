//! Actions 统一管理模块
//!
//! 本模块集中管理所有应用中使用的 GPUI Actions，便于维护和查找。
//! Actions 是 GPUI 中用于触发用户操作的类型安全机制。

use gpui::{actions, Action, SharedString};
use gpui_component::{dock::DockPlacement, scroll::ScrollbarShow, ThemeMode};
use serde::Deserialize;

// ============================================================================
// Workspace Actions - 工作区相关操作
// ============================================================================

/// 添加面板到 Dock 区域
///
/// 参数为目标 DockPlacement，用于指定面板放置位置（Center/Left/Right/Bottom）
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct AddPanel(pub DockPlacement);

/// 切换面板的可见性
///
/// 参数为面板的名称（SharedString），用于显示或隐藏指定面板
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct TogglePanelVisible(pub SharedString);

/// 添加会话面板
///
/// 用于创建并添加一个新的会话面板到工作区
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct AddSessionPanel {
    /// 会话唯一标识符
    pub session_id: String,
    /// 面板放置位置，默认为 Center
    #[serde(skip, default = "default_dock_placement")]
    pub placement: DockPlacement,
}

fn default_dock_placement() -> DockPlacement {
    DockPlacement::Center
}

impl Default for AddSessionPanel {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            placement: DockPlacement::Center,
        }
    }
}

// 切换 Dock 切换按钮的显示状态
actions!(story, [ToggleDockToggleButton]);

// ============================================================================
// Task List Actions - 任务列表相关操作
// ============================================================================

// 选中的 Agent 任务 - 当用户在任务列表中选择某个任务时触发
actions!(list_task, [SelectedAgentTask]);

/// 添加会话到任务列表
///
/// 用于将新会话添加到左侧任务列表面板
#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = list_task, no_json)]
pub struct AddSessionToList {
    /// 会话唯一标识符
    pub session_id: String,
    /// 任务显示名称
    pub task_name: String,
}

// ============================================================================
// UI Settings Actions - 界面设置相关操作
// ============================================================================

/// 选择滚动条显示模式
///
/// 用于切换滚动条的显示策略（Always/Auto/Never）
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectScrollbarShow(pub ScrollbarShow);

/// 选择界面语言
///
/// 用于切换应用界面的显示语言
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectLocale(pub SharedString);

/// 选择字体
///
/// 参数为字体索引，用于切换编辑器和界面字体
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectFont(pub usize);

/// 选择圆角大小
///
/// 参数为圆角索引，用于调整界面组件的圆角样式
#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct SelectRadius(pub usize);

// ============================================================================
// General Application Actions - 通用应用操作
// ============================================================================

/// 从欢迎面板创建新任务
///
/// 参数为任务和 Agent 参数，用于快速创建新的 Agent 任务
#[derive(Action, Clone, Debug, PartialEq, Eq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct CreateTaskFromWelcome {
    /// 任务描述，由用户输入
    pub task_input: String,
    /// 使用的 Agent 名称
    pub agent_name: String,
    /// 任务模式
    pub mode: String,
}
/// 显示会话对话面板
///
#[derive(Action, Clone, PartialEq, Deserialize)]
#[action(namespace = task_list, no_json)]
pub struct NewSessionConversationPanel {
    /// Agent 的会话唯一标识符，保存在 Appstate 中
    pub session_id: String,
    /// 使用的 Agent 名称
    pub agent_name: String,
    /// 任务模式
    pub mode: String,
}

// 通用应用级操作 - 包含各种应用级别的命令和操作
actions!(
    story,
    [
        About,         // 显示关于对话框
        Open,          // 打开文件或项目
        Quit,          // 退出应用
        CloseWindow,   // 关闭当前窗口
        ToggleSearch,  // 切换搜索面板
        TestAction,    // 测试用操作
        Tab,           // 切换到下一个标签页
        TabPrev,       // 切换到上一个标签页
        ShowPanelInfo  // 显示面板信息
    ]
);

/// 显示欢迎面板
///
/// 可选参数:
/// - workspace_id: 要在欢迎面板中显示的工作区 ID
#[derive(Action, Clone, PartialEq, Deserialize)]
#[action(namespace = story, no_json)]
pub struct ShowWelcomePanel {
    #[serde(default)]
    pub workspace_id: Option<String>,
}

/// 显示对话面板
///
/// 参数为可选的 session_id，用于打开特定会话的对话面板
/// 如果不提供 session_id，则创建一个新的对话面板
#[derive(Action, Clone, PartialEq, Deserialize)]
#[action(namespace = agentx, no_json)]
pub struct ShowConversationPanel {
    pub session_id: Option<String>,
}

impl ShowConversationPanel {
    /// Create a new action to show conversation panel for a specific session
    pub fn with_session(session_id: String) -> Self {
        Self {
            session_id: Some(session_id),
        }
    }

    /// Create a new action to show conversation panel without a specific session
    pub fn new() -> Self {
        Self { session_id: None }
    }
}

// ============================================================================
// Menu Actions - 菜单相关操作
// ============================================================================

/// 菜单信息操作
///
/// 参数为菜单项索引，用于处理菜单相关的信息显示
#[derive(Action, Clone, PartialEq, Deserialize)]
#[action(namespace = menu_story, no_json)]
pub struct Info(pub usize);

// 菜单编辑操作 - 包含常见的编辑菜单命令
actions!(
    menu_story,
    [
        Copy,        // 复制
        Paste,       // 粘贴
        Cut,         // 剪切
        SearchAll,   // 全局搜索
        Submit,      // 提交
        ToggleCheck, // 切换勾选状态
        SelectLeft,  // 向左选择
        SelectRight, // 向右选择
    ]
);

// ============================================================================
// Theme Actions - 主题相关操作
// ============================================================================

/// 切换应用主题
///
/// 参数为主题名称，用于切换不同的颜色主题（如 Dark/Light/One/Ayu 等）
#[derive(Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub struct SwitchTheme(pub SharedString);

/// 切换主题模式
///
/// 用于在亮色和暗色模式之间切换
#[derive(Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub struct SwitchThemeMode(pub ThemeMode);
