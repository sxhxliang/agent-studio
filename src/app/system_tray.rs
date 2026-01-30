use anyhow::{Context, Result};
use std::sync::Arc;
use tray_icon::{
    MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent, TrayIconId,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

// 定义菜单项ID常量
const MENU_SHOW_ID: &str = "show_window";
const MENU_QUIT_ID: &str = "quit_app";

// 定义唯一的托盘图标 ID，避免与其他应用冲突
const TRAY_ICON_ID: &str = "plus.agentx.app.tray";

/// 在 Linux 平台上初始化 GTK
/// 必须在创建托盘图标之前调用
#[cfg(target_os = "linux")]
pub fn init_platform() -> Result<()> {
    gtk::init().context("Failed to initialize GTK")?;
    log::info!("GTK initialized for Linux tray icon support");
    Ok(())
}

/// 非 Linux 平台不需要额外初始化
#[cfg(not(target_os = "linux"))]
pub fn init_platform() -> Result<()> {
    Ok(())
}

/// 系统托盘管理器
pub struct SystemTray {
    tray_icon: TrayIcon,
    show_menu_id: MenuId,
    quit_menu_id: MenuId,
}

impl SystemTray {
    /// 创建系统托盘
    pub fn new() -> Result<Self> {
        // 创建托盘菜单
        let tray_menu = Menu::new();

        // 创建菜单项ID
        let show_menu_id = MenuId::new(MENU_SHOW_ID);
        let quit_menu_id = MenuId::new(MENU_QUIT_ID);

        // 添加显示/隐藏窗口菜单项
        let show_item = MenuItem::with_id(show_menu_id.clone(), "显示主窗口", true, None);
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::with_id(quit_menu_id.clone(), "退出", true, None);

        tray_menu
            .append(&show_item)
            .context("Failed to append show item")?;
        tray_menu
            .append(&separator)
            .context("Failed to append separator")?;
        tray_menu
            .append(&quit_item)
            .context("Failed to append quit item")?;

        // 加载托盘图标
        let icon = load_icon()?;

        // 创建托盘图标
        // 注意：菜单只在右键点击时显示，左键点击不显示菜单
        // 使用唯一的 ID 避免与其他应用冲突 (Fix #15)
        let tray_icon = TrayIconBuilder::new()
            .with_id(TrayIconId::new(TRAY_ICON_ID))
            .with_menu(Box::new(tray_menu))
            .with_menu_on_left_click(false) // 禁用左键显示菜单
            .with_tooltip("AgentX Studio")
            .with_icon(icon)
            .build()
            .context("Failed to build tray icon")?;

        Ok(Self {
            tray_icon,
            show_menu_id,
            quit_menu_id,
        })
    }
}

/// 托盘事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// 显示窗口
    Show,
    /// 退出应用
    Quit,
}

/// 加载托盘图标
fn load_icon() -> Result<tray_icon::Icon> {
    // 使用内嵌的 logo 图片
    let icon_bytes = include_bytes!("../../assets/logo1024x1024@2x.png");

    // 加载 PNG 图片
    let image = image::load_from_memory(icon_bytes).context("Failed to load icon image")?;

    // 转换为 RGBA8
    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let rgba_data = rgba_image.into_raw();

    // 创建托盘图标
    tray_icon::Icon::from_rgba(rgba_data, width, height).context("Failed to create tray icon")
}

/// 注册托盘事件处理器到 GPUI
///
/// 这个函数会启动一个后台线程,持续监听托盘事件并触发相应的操作
pub fn setup_tray_event_handler(tray: SystemTray, cx: &mut gpui::App) {
    use gpui::AppContext;

    // 获取全局菜单事件接收器
    let menu_event_receiver = MenuEvent::receiver().clone();
    // 获取托盘图标事件接收器
    let tray_icon_event_receiver = TrayIconEvent::receiver().clone();

    // 提取菜单ID用于后台线程
    let show_menu_id = tray.show_menu_id.clone();
    let quit_menu_id = tray.quit_menu_id.clone();

    // 将 SystemTray 存储为 static，保持托盘图标的生命周期
    // 这样托盘图标就不会被销毁
    let _tray = Box::leak(Box::new(tray));

    // 创建通道用于跨线程通信
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TrayEvent>();

    // 在独立线程中轮询托盘事件
    std::thread::spawn(move || {
        loop {
            let mut has_event = false;

            // 轮询托盘图标点击事件
            if let Ok(event) = tray_icon_event_receiver.try_recv() {
                has_event = true;
                match event {
                    TrayIconEvent::Click { button, .. } => {
                        // 只处理左键点击 - 显示窗口
                        // 右键点击会自动显示菜单，不需要额外处理
                        if button == MouseButton::Left {
                            log::info!("Tray icon left-clicked, showing window");
                            if tx.send(TrayEvent::Show).is_err() {
                                log::error!("Failed to send tray event, receiver dropped");
                                break;
                            }
                        }
                    }
                    TrayIconEvent::DoubleClick { button, .. } => {
                        // 只处理左键双击 - 也显示窗口
                        if button == MouseButton::Left {
                            log::info!("Tray icon double-clicked, showing window");
                            if tx.send(TrayEvent::Show).is_err() {
                                log::error!("Failed to send tray event, receiver dropped");
                                break;
                            }
                        }
                    }
                    _ => {
                        // 忽略其他事件
                    }
                }
            }

            // 轮询菜单事件
            if let Ok(event) = menu_event_receiver.try_recv() {
                has_event = true;
                let menu_id = event.id();

                // 根据菜单ID判断操作
                let tray_event = if menu_id == &show_menu_id {
                    Some(TrayEvent::Show)
                } else if menu_id == &quit_menu_id {
                    Some(TrayEvent::Quit)
                } else {
                    None
                };

                if let Some(tray_event) = tray_event {
                    // 发送事件到通道
                    if tx.send(tray_event).is_err() {
                        log::error!("Failed to send tray event, receiver dropped");
                        break;
                    }

                    // 如果是退出事件，停止轮询
                    if tray_event == TrayEvent::Quit {
                        break;
                    }
                }
            }

            // 如果没有事件，休眠以避免忙等待；如果有事件，立即继续轮询
            if !has_event {
                std::thread::sleep(std::time::Duration::from_millis(16)); // 约60fps的轮询频率
            }
        }
    });

    // 在 GPUI 异步任务中处理托盘事件
    cx.spawn(async move |cx| {
        while let Some(event) = rx.recv().await {
            match event {
                TrayEvent::Show => {
                    // 显示窗口 - 需要在 GPUI 上下文中处理
                    log::info!("Tray event: Show window");
                    let _ = cx.update(|cx| {
                        // 获取所有窗口并显示第一个
                        if let Some(window) = cx.windows().first() {
                            let _ = window.update(cx, |_, window, _| {
                                window.activate_window();
                            });
                        }
                    });
                }
                TrayEvent::Quit => {
                    // 退出应用
                    log::info!("Tray event: Quit application");
                    let _ = cx.update(|cx| {
                        cx.quit();
                    });
                    break;
                }
            }
        }

        Ok::<(), anyhow::Error>(())
    })
    .detach();
}
