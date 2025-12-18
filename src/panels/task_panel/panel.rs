//! Task Panel - Workspace sidebar for agentx
//!
//! This panel displays tasks organized by workspace with:
//! - Expandable/collapsible workspace groups
//! - Task items with status indicators
//! - Tree view (by workspace) and timeline view (by date)

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, InteractiveElementExt, Selectable, Sizable, StyledExt,
    button::{Button, ButtonGroup, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement as _,
    v_flex,
};
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::services::WorkspaceService;
use crate::core::{event_bus::WorkspaceUpdateEvent, services::SessionStatus};
use crate::panels::dock_panel::DockPanel;
use crate::schemas::workspace_schema::WorkspaceTask;
use crate::{AppState, ShowConversationPanel, ShowWelcomePanel, utils};

// ============================================================================
// Constants - Layout spacing
// ============================================================================

/// Left indent for child items under workspace header (matches chevron width + gap)
const CHILD_INDENT: f32 = 22.0; // ChevronIcon(16px) + gap(6px)

// ============================================================================
// Data Models
// ============================================================================

#[derive(Clone, Debug)]
pub struct WorkspaceGroup {
    pub id: String,
    pub name: String,
    pub tasks: Vec<Rc<WorkspaceTask>>,
    pub is_expanded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewMode {
    Tree,
    Timeline,
}

// ============================================================================
// Main Panel Component
// ============================================================================

pub struct TaskPanel {
    focus_handle: FocusHandle,
    workspaces: Vec<WorkspaceGroup>,
    selected_task_id: Option<String>,
    view_mode: ViewMode,
    _subscriptions: Vec<Subscription>,
    /// Search input state
    search_input: Entity<InputState>,
    load_generation: u64,
}

impl DockPanel for TaskPanel {
    fn title() -> &'static str {
        "任务"
    }

    fn description() -> &'static str {
        "Task list grouped by workspace"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> Pixels {
        px(0.)
    }
}

impl TaskPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        if let Some(workspace_service) = AppState::global(cx).workspace_service() {
            Self::load_workspace_data(&entity, workspace_service.clone(), cx);
            Self::subscribe_to_workspace_updates(&entity, cx);
        } else {
            log::warn!("WorkspaceService not available, TaskPanel will remain empty");
        }

        entity
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| InputState::new(window, cx));

        // Subscribe to search input changes to trigger re-render
        let search_subscription = cx.subscribe(
            &search_input,
            |_this, _input, _event: &gpui_component::input::InputEvent, cx| {
                cx.notify(); // Trigger re-render when search input changes
            },
        );

        Self {
            focus_handle: cx.focus_handle(),
            workspaces: Vec::new(),
            selected_task_id: None,
            view_mode: ViewMode::Tree,
            _subscriptions: vec![search_subscription],
            search_input,
            load_generation: 0,
        }
    }

    // ========================================================================
    // Data Loading
    // ========================================================================

    fn load_workspace_data(
        entity: &Entity<Self>,
        workspace_service: std::sync::Arc<WorkspaceService>,
        cx: &mut App,
    ) {
        let generation = entity.update(cx, |this, _| {
            this.load_generation = this.load_generation.wrapping_add(1);
            this.load_generation
        });

        let entity_clone = entity.clone();
        cx.spawn(async move |cx| {
            let config = workspace_service.get_config().await;
            let workspaces_list = config.workspaces;
            let tasks = config.tasks;

            let mut tasks_by_workspace: HashMap<String, Vec<Rc<WorkspaceTask>>> = HashMap::new();
            for task in tasks {
                tasks_by_workspace
                    .entry(task.workspace_id.clone())
                    .or_default()
                    .push(Rc::new(task));
            }

            cx.update(|cx| {
                entity_clone.update(cx, |this, cx| {
                    if this.load_generation != generation {
                        return;
                    }

                    let previously_expanded: HashMap<String, bool> = this
                        .workspaces
                        .iter()
                        .map(|w| (w.id.clone(), w.is_expanded))
                        .collect();

                    this.workspaces = workspaces_list
                        .into_iter()
                        .map(|ws| {
                            let tasks = tasks_by_workspace.remove(&ws.id).unwrap_or_default();

                            WorkspaceGroup {
                                id: ws.id.clone(),
                                name: ws.name.clone(),
                                tasks,
                                is_expanded: previously_expanded
                                    .get(&ws.id)
                                    .copied()
                                    .unwrap_or(true),
                            }
                        })
                        .collect();

                    this.ensure_selected_task_valid();
                    cx.notify();
                });
            })
            .ok();
        })
        .detach();
    }

    fn subscribe_to_workspace_updates(entity: &Entity<Self>, cx: &mut App) {
        let workspace_bus = AppState::global(cx).workspace_bus.clone();
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not available for subscription");
                return;
            }
        };

        let entity_weak = entity.downgrade();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // Subscribe to workspace bus
        workspace_bus.lock().unwrap().subscribe(move |event| {
            let _ = tx.send(event.clone());
        });

        // Spawn task to process events and update UI
        cx.spawn(async move |cx| {
            while let Some(event) = rx.recv().await {
                match event {
                    WorkspaceUpdateEvent::WorkspaceAdded { workspace_id } => {
                        log::debug!("TaskPanel received WorkspaceAdded: {}", workspace_id);
                        if let Some(entity) = entity_weak.upgrade() {
                            cx.update(|cx| {
                                Self::load_workspace_data(&entity, workspace_service.clone(), cx);
                            })
                            .ok();
                        }
                    }
                    WorkspaceUpdateEvent::WorkspaceRemoved { workspace_id } => {
                        log::debug!("TaskPanel received WorkspaceRemoved: {}", workspace_id);
                        if let Some(entity) = entity_weak.upgrade() {
                            cx.update(|cx| {
                                Self::load_workspace_data(&entity, workspace_service.clone(), cx);
                            })
                            .ok();
                        }
                    }
                    WorkspaceUpdateEvent::TaskCreated {
                        workspace_id,
                        task_id,
                    } => {
                        log::debug!(
                            "TaskPanel received TaskCreated: {} in {}",
                            task_id,
                            workspace_id
                        );
                        // Reload workspace data to include the new task
                        if let Some(entity) = entity_weak.upgrade() {
                            cx.update(|cx| {
                                Self::load_workspace_data(&entity, workspace_service.clone(), cx);
                            })
                            .ok();
                        }
                    }
                    WorkspaceUpdateEvent::TaskRemoved {
                        workspace_id,
                        task_id,
                    } => {
                        log::debug!(
                            "TaskPanel received TaskRemoved: {} from {}",
                            task_id,
                            workspace_id
                        );
                        // Reload workspace data to remove the task from UI
                        if let Some(entity) = entity_weak.upgrade() {
                            cx.update(|cx| {
                                Self::load_workspace_data(&entity, workspace_service.clone(), cx);
                            })
                            .ok();
                        }
                    }
                    WorkspaceUpdateEvent::TaskUpdated { task_id } => {
                        log::debug!("TaskPanel received TaskUpdated: {}", task_id);
                        if let Some(entity) = entity_weak.upgrade() {
                            cx.update(|cx| {
                                Self::load_workspace_data(&entity, workspace_service.clone(), cx);
                            })
                            .ok();
                        }
                    }
                    WorkspaceUpdateEvent::SessionStatusUpdated {
                        session_id, status, ..
                    } => {
                        log::debug!("TaskPanel received SessionStatusUpdated: {}", session_id);
                        if let Some(entity) = entity_weak.upgrade() {
                            let session_id = session_id.clone();
                            let status = status.clone();
                            cx.update(|cx| {
                                entity.update(cx, |this, cx| {
                                    this.update_task_status_by_session_id(&session_id, status, cx);
                                });
                            })
                            .ok();
                        }
                    }
                }
            }
        })
        .detach();
    }

    fn ensure_selected_task_valid(&mut self) {
        let selected_is_valid = self.selected_task_id.as_ref().is_some_and(|id| {
            self.workspaces
                .iter()
                .flat_map(|w| w.tasks.iter())
                .any(|t| &t.id == id)
        });

        if selected_is_valid {
            return;
        }

        self.selected_task_id = self
            .workspaces
            .iter()
            .flat_map(|w| w.tasks.iter())
            .next()
            .map(|t| t.id.clone());
    }

    fn update_task_status_by_session_id(
        &mut self,
        session_id: &str,
        status: SessionStatus,
        cx: &mut Context<Self>,
    ) {
        let mut updated = false;
        let mut task_id_to_update: Option<String> = None;

        // Update local state for immediate UI feedback
        for workspace in &mut self.workspaces {
            for task in &mut workspace.tasks {
                if task.session_id.as_deref() != Some(session_id) {
                    continue;
                }

                if task.status == status {
                    continue;
                }

                // Store task_id for persistence
                task_id_to_update = Some(task.id.clone());

                let mut updated_task = (**task).clone();
                updated_task.status = status.clone();
                *task = Rc::new(updated_task);
                updated = true;
            }
        }

        if updated {
            cx.notify();

            // Persist status to JSON file
            if let Some(task_id) = task_id_to_update {
                if let Some(workspace_service) = AppState::global(cx).workspace_service() {
                    let workspace_service = workspace_service.clone();
                    let status_clone = status.clone();
                    cx.spawn(async move |_entity, _cx| {
                        match workspace_service.update_task_status(&task_id, status_clone).await {
                            Ok(_) => {
                                log::debug!("Task status persisted: {} -> {:?}", task_id, status);
                            }
                            Err(e) => {
                                log::error!("Failed to persist task status: {}", e);
                            }
                        }
                    })
                    .detach();
                }
            }
        }
    }

    // ========================================================================
    // Event Handlers
    // ========================================================================

    fn toggle_workspace(&mut self, workspace_id: String, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            workspace.is_expanded = !workspace.is_expanded;
            cx.notify();
        }
    }

    fn add_workspace(&mut self, cx: &mut Context<Self>) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not available");
                return;
            }
        };

        cx.spawn(async move |entity, cx| {
            if let Some(folder_path) = utils::pick_folder("选择工作区文件夹").await {
                log::info!("Selected folder: {:?}", folder_path);

                match workspace_service.add_workspace(folder_path.clone()).await {
                    Ok(workspace) => {
                        log::info!("Successfully added workspace: {}", workspace.name);
                        cx.update(|cx| {
                            if let Some(entity_strong) = entity.upgrade() {
                                Self::load_workspace_data(
                                    &entity_strong,
                                    workspace_service.clone(),
                                    cx,
                                );
                            }
                        })
                        .ok();
                    }
                    Err(e) => {
                        log::error!("Failed to add workspace: {}", e);
                    }
                }
            }
        })
        .detach();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not available");
                return;
            }
        };

        cx.spawn(async move |entity, cx| {
            cx.update(|cx| {
                if let Some(entity_strong) = entity.upgrade() {
                    Self::load_workspace_data(&entity_strong, workspace_service.clone(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn remove_workspace(&mut self, workspace_id: String, cx: &mut Context<Self>) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not available");
                return;
            }
        };

        cx.spawn(async move |entity, cx| {
            match workspace_service.remove_workspace(&workspace_id).await {
                Ok(_) => {
                    log::info!("Successfully removed workspace: {}", workspace_id);
                    cx.update(|cx| {
                        if let Some(entity_strong) = entity.upgrade() {
                            Self::load_workspace_data(
                                &entity_strong,
                                workspace_service.clone(),
                                cx,
                            );
                        }
                    })
                    .ok();
                }
                Err(e) => {
                    log::error!("Failed to remove workspace: {}", e);
                }
            }
        })
        .detach();
    }

    fn remove_task(&mut self, task_id: String, cx: &mut Context<Self>) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("WorkspaceService not available");
                return;
            }
        };

        cx.spawn(async move |entity, cx| {
            match workspace_service.remove_task(&task_id).await {
                Ok(_) => {
                    log::info!("Successfully removed task: {}", task_id);
                    // The UI will be updated via the TaskRemoved event
                }
                Err(e) => {
                    log::error!("Failed to remove task: {}", e);
                }
            }
        })
        .detach();
    }

    fn select_task(&mut self, task_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_task_id = Some(task_id.clone());

        let session_id = self
            .workspaces
            .iter()
            .flat_map(|w| &w.tasks)
            .find(|t| t.id == task_id)
            .and_then(|t| t.session_id.clone());

        let action = match session_id {
            Some(id) => ShowConversationPanel::with_session(id),
            None => ShowConversationPanel::new(),
        };
        window.dispatch_action(Box::new(action), cx);
        cx.notify();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.view_mode = mode;
        cx.notify();
    }

    // ========================================================================
    // Search & Filter
    // ========================================================================

    fn get_filtered_workspaces(&self, cx: &Context<Self>) -> Vec<WorkspaceGroup> {
        let search_query = self.search_input.read(cx).text().to_string().to_lowercase();

        if search_query.is_empty() {
            return self.workspaces.clone();
        }

        self.workspaces
            .iter()
            .filter_map(|workspace| {
                // Filter tasks that match the search query
                let filtered_tasks: Vec<_> = workspace
                    .tasks
                    .iter()
                    .filter(|task| {
                        task.name.to_lowercase().contains(&search_query)
                            || task.agent_name.to_lowercase().contains(&search_query)
                            || task.mode.to_lowercase().contains(&search_query)
                            || task
                                .last_message
                                .as_ref()
                                .map(|msg| msg.to_lowercase().contains(&search_query))
                                .unwrap_or(false)
                    })
                    .cloned()
                    .collect();

                // Include workspace if it has matching tasks or its name matches
                if !filtered_tasks.is_empty()
                    || workspace.name.to_lowercase().contains(&search_query)
                {
                    Some(WorkspaceGroup {
                        id: workspace.id.clone(),
                        name: workspace.name.clone(),
                        tasks: filtered_tasks,
                        is_expanded: workspace.is_expanded,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    // ========================================================================
    // Render - Header & Footer
    // ========================================================================

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let view_mode = self.view_mode;

        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .px_3()
            .py_3()
            .border_b_1()
            .border_color(theme.border)
            .child(
                // Search box
                div().flex_1().child(
                    Input::new(&self.search_input)
                        .small()
                        // .bordered(false)
                        .cleanable(true)
                        .prefix(
                            Icon::new(IconName::Search)
                                .size_4()
                                .text_color(theme.muted_foreground),
                        ),
                ),
            )
            .child(
                // View toggle buttons
                ButtonGroup::new("view-toggle")
                    .small()
                    .child(
                        Button::new("tree-view")
                            .icon(IconName::LayoutDashboard)
                            .ghost()
                            .xsmall()
                            .selected(view_mode == ViewMode::Tree)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Tree, cx);
                            })),
                    )
                    .child(
                        Button::new("timeline-view")
                            .icon(IconName::Menu)
                            .ghost()
                            .xsmall()
                            .selected(view_mode == ViewMode::Timeline)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_view_mode(ViewMode::Timeline, cx);
                            })),
                    ),
            )
    }

    fn render_footer(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(theme.border)
            .child(
                Button::new("add-workspace")
                    .ghost()
                    .small()
                    .icon(IconName::FolderOpen)
                    .label("添加工作区")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.add_workspace(cx);
                    })),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("refresh")
                            .ghost()
                            .small()
                            .icon(Icon::new(crate::assets::Icon::FolderSync))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh(cx);
                            })),
                    )
                    .child(
                        Button::new("monitor")
                            .ghost()
                            .small()
                            .icon(crate::assets::Icon::Monitor),
                    )
                    .child(
                        Button::new("settings")
                            .ghost()
                            .small()
                            .icon(IconName::Settings),
                    ),
            )
    }

    // ========================================================================
    // Render - Tree View
    // ========================================================================

    fn render_tree_view(&self, cx: &Context<Self>) -> impl IntoElement {
        let filtered_workspaces = self.get_filtered_workspaces(cx);
        let entity = cx.entity().clone();

        div()
            .id("task-tree-scroll")
            .flex_1()
            .overflow_y_scroll()
            .children(
                filtered_workspaces
                    .iter()
                    .map(|workspace| self.render_workspace_group(workspace, entity.clone(), cx)),
            )
    }

    fn render_workspace_group(
        &self,
        workspace: &WorkspaceGroup,
        entity: Entity<Self>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let workspace_id = workspace.id.clone();
        let is_expanded = workspace.is_expanded;
        let workspace_name = workspace.name.clone();
        let task_count = workspace.tasks.len();

        v_flex()
            .w_full()
            // Workspace header row
            .child(
                h_flex()
                    .id(SharedString::from(format!("workspace-{}", workspace_id)))
                    .w_full()
                    .justify_between()
                    .items_center()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.accent.opacity(0.3)))
                    .on_click(cx.listener({
                        let workspace_id = workspace_id.clone();
                        move |this, _, _, cx| {
                            this.toggle_workspace(workspace_id.clone(), cx);
                        }
                    }))
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(
                                Icon::new(if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size_4()
                                .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(theme.foreground)
                                    .child(workspace_name),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .when(task_count > 0, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{}", task_count)),
                                )
                            })
                            .child({
                                let workspace_id = workspace_id.clone();
                                let entity = entity.clone();
                                Button::new(SharedString::from(format!(
                                    "workspace-menu-{}",
                                    workspace_id
                                )))
                                .icon(IconName::Ellipsis)
                                .ghost()
                                .xsmall()
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .dropdown_menu(
                                    move |menu, _, _| {
                                        let workspace_id = workspace_id.clone();
                                        let entity = entity.clone();
                                        menu.item(
                                            PopupMenuItem::new("移除工作区")
                                                .icon(Icon::new(crate::assets::Icon::Trash2))
                                                .on_click(move |_, _, cx| {
                                                    entity.update(cx, |this, cx| {
                                                        this.remove_workspace(
                                                            workspace_id.clone(),
                                                            cx,
                                                        );
                                                    });
                                                }),
                                        )
                                    },
                                )
                            }),
                    ),
            )
            // Expanded children
            .when(is_expanded, |this| {
                this.child(self.render_new_task_button(&workspace.id, cx))
                    .children(
                        workspace
                            .tasks
                            .iter()
                            .map(|task| self.render_task_item(task, entity.clone(), cx)),
                    )
            })
    }

    fn render_new_task_button(&self, workspace_id: &str, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let workspace_id = workspace_id.to_string();

        h_flex()
            .id(SharedString::from(format!(
                "new-task-button-{}",
                workspace_id
            )))
            .w_full()
            .items_center()
            .px_3()
            .py_1p5()
            .pl(px(CHILD_INDENT + 12.0)) // 12px = px_3
            .gap_2()
            .cursor_pointer()
            .hover(|s| s.bg(theme.accent.opacity(0.3)))
            .on_click(cx.listener(move |_this, _, window, cx| {
                window.dispatch_action(
                    Box::new(ShowWelcomePanel {
                        workspace_id: Some(workspace_id.clone()),
                    }),
                    cx,
                );
            }))
            .child(
                Icon::new(IconName::Plus)
                    .size_4()
                    .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("新建任务"),
            )
    }

    fn render_task_item(
        &self,
        task: &Rc<WorkspaceTask>,
        entity: Entity<Self>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let task_id = task.id.clone();
        let is_selected = self.selected_task_id.as_ref() == Some(&task_id);

        v_flex()
            .id(SharedString::from(format!("task-{}", task_id)))
            .w_full()
            .gap_0p5()
            .px_3()
            .pl(px(CHILD_INDENT + 12.0)) // Align with new task button
            .py_2()
            .cursor_pointer()
            .when(is_selected, |s| s.bg(theme.accent))
            .when(!is_selected, |s| {
                s.hover(|s| s.bg(theme.accent.opacity(0.5)))
            })
            .on_click(cx.listener({
                let task_id = task_id.clone();
                move |this, _, window, cx| {
                    this.select_task(task_id.clone(), window, cx);
                }
            }))
            .on_double_click(cx.listener({
                let task_id = task_id.clone();
                move |this, _, window, cx| {
                    log::debug!("====== >>> Double click on task");
                    this.select_task(task_id.clone(), window, cx);
                }
            }))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener({
                    // let session_id = session_id.clone();
                    move |_this, event: &MouseDownEvent, window, cx| {
                        if event.click_count == 2 {
                            // 双击：打开新的会话面板
                            // let action = match session_id.as_ref() {
                            //     Some(id) => ShowConversationPanel::with_session(id.clone()),
                            //     None => ShowConversationPanel::new(),
                            // };
                            // window.dispatch_action(Box::new(action), cx);
                        }
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener({
                    let task_id = task_id.clone();
                    move |this, event: &gpui::MouseUpEvent, window, cx| {
                        if event.click_count == 1 {
                            log::debug!("====== >>> 单击  click on task");
                        } else if event.click_count == 2 {
                            log::debug!("====== >>> 双击  click on task");
                        }
                    }
                }),
            )
            // First row: status icon + task name + mode
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .min_w_0()
                            .flex_1()
                            .child(
                                Icon::new(self.status_icon(&task.status))
                                    .size_4()
                                    .text_color(self.status_color(&task.status)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .overflow_x_hidden()
                                    .text_ellipsis()
                                    .child(task.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(task.mode.clone()),
                    ),
            )
            // Second row: agent name + last message + status badge (aligned with task name)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().size_4()) // Spacer to align with icon above
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        div()
                                            .overflow_x_hidden()
                                            .text_ellipsis()
                                            .child(task.agent_name.clone()),
                                    )
                                    .child("·")
                                    .when_some(task.last_message.clone(), |this, msg| {
                                        this.child(
                                            div().overflow_x_hidden().text_ellipsis().child(msg),
                                        )
                                    }),
                            ),
                    )
                    .child(self.render_status_badge(&task.status, cx)),
            )
            // Right-click context menu
            .context_menu(move |menu, _, _| {
                let task_id = task_id.clone();
                let entity = entity.clone();
                menu.item(
                    PopupMenuItem::new("删除任务")
                        .icon(Icon::new(crate::assets::Icon::Trash2))
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.remove_task(task_id.clone(), cx);
                            });
                        }),
                )
            })
    }

    // ========================================================================
    // Render - Timeline View
    // ========================================================================

    fn render_timeline_view(&self, cx: &Context<Self>) -> impl IntoElement {
        use chrono::{Duration, Local};

        let filtered_workspaces = self.get_filtered_workspaces(cx);
        let entity = cx.entity().clone();

        let mut all_tasks: Vec<Rc<WorkspaceTask>> = filtered_workspaces
            .iter()
            .flat_map(|w| w.tasks.clone())
            .collect();

        all_tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let now = Local::now().date_naive();

        let today: Vec<_> = all_tasks
            .iter()
            .filter(|t| t.created_at.with_timezone(&Local).date_naive() == now)
            .collect();

        let yesterday: Vec<_> = all_tasks
            .iter()
            .filter(|t| t.created_at.with_timezone(&Local).date_naive() == now - Duration::days(1))
            .collect();

        let older: Vec<_> = all_tasks
            .iter()
            .filter(|t| t.created_at.with_timezone(&Local).date_naive() < now - Duration::days(1))
            .collect();

        v_flex()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .when(!today.is_empty(), |this| {
                this.child(self.render_time_group("今天", &today, entity.clone(), cx))
            })
            .when(!yesterday.is_empty(), |this| {
                this.child(self.render_time_group("昨天", &yesterday, entity.clone(), cx))
            })
            .when(!older.is_empty(), |this| {
                this.child(self.render_time_group("更早", &older, entity.clone(), cx))
            })
    }

    fn render_time_group(
        &self,
        label: &str,
        tasks: &[&Rc<WorkspaceTask>],
        entity: Entity<Self>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .w_full()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(theme.sidebar.opacity(0.95))
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .child(
                        div()
                            .text_xs()
                            .font_medium()
                            .text_color(theme.muted_foreground)
                            .child(label.to_uppercase()),
                    ),
            )
            .children(
                tasks
                    .iter()
                    .map(|task| self.render_timeline_task_item(task, entity.clone(), cx)),
            )
    }

    fn render_timeline_task_item(
        &self,
        task: &Rc<WorkspaceTask>,
        entity: Entity<Self>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let task_id = task.id.clone();
        let is_selected = self.selected_task_id.as_ref() == Some(&task_id);

        v_flex()
            .id(SharedString::from(format!("timeline-task-{}", task_id)))
            .w_full()
            .gap_1()
            .px_3()
            .py_2p5()
            .cursor_pointer()
            .border_b_1()
            .border_color(theme.border.opacity(0.5))
            .when(is_selected, |s| s.bg(theme.accent))
            .when(!is_selected, |s| {
                s.hover(|s| s.bg(theme.accent.opacity(0.5)))
            })
            .on_click(cx.listener({
                let task_id = task_id.clone();
                move |this, _, window, cx| {
                    this.select_task(task_id.clone(), window, cx);
                }
            }))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(theme.foreground)
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .child(task.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(task.mode.clone()),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(
                        Icon::new(self.status_icon(&task.status))
                            .size_3()
                            .text_color(self.status_color(&task.status)),
                    )
                    .child(
                        div()
                            .overflow_x_hidden()
                            .text_ellipsis()
                            .child(task.agent_name.clone()),
                    )
                    .when_some(task.last_message.clone(), |this, msg| {
                        this.child("·")
                            .child(div().overflow_x_hidden().text_ellipsis().child(msg))
                    }),
            )
            .child(self.render_status_badge(&task.status, cx))
            // Right-click context menu
            .context_menu(move |menu, _, _| {
                let task_id = task_id.clone();
                let entity = entity.clone();
                menu.item(
                    PopupMenuItem::new("删除任务")
                        .icon(Icon::new(crate::assets::Icon::Trash2))
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.remove_task(task_id.clone(), cx);
                            });
                        }),
                )
            })
    }

    // ========================================================================
    // Render - Status helpers
    // ========================================================================

    fn render_status_badge(&self, status: &SessionStatus, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (label, color) = match status {
            SessionStatus::Active => ("待输入", theme.muted_foreground),
            SessionStatus::Idle => ("等待中", theme.muted_foreground),
            SessionStatus::Pending => ("进行中", theme.muted_foreground),
            SessionStatus::InProgress => ("进行中", gpui::rgb(0x22c55e).into()),
            SessionStatus::Completed => ("已完成", gpui::rgb(0x22c55e).into()),
            SessionStatus::Failed => ("失败", gpui::rgb(0xef4444).into()),
            SessionStatus::Closed => ("关闭", gpui::rgb(0xef4444).into()),
        };

        div().text_xs().text_color(color).child(label)
    }

    fn status_icon(&self, status: &SessionStatus) -> IconName {
        match status {
            SessionStatus::Active => IconName::Asterisk,
            SessionStatus::Idle => IconName::Asterisk,
            SessionStatus::Pending => IconName::Asterisk,
            SessionStatus::InProgress => IconName::Loader,
            SessionStatus::Completed => IconName::CircleCheck,
            SessionStatus::Failed => IconName::CircleX,
            SessionStatus::Closed => IconName::CircleX,
        }
    }

    fn status_color(&self, status: &SessionStatus) -> gpui::Hsla {
        match status {
            SessionStatus::Active => gpui::rgb(0x22c55e).into(),
            SessionStatus::Idle => gpui::rgb(0x22c55e).into(),
            SessionStatus::Pending => gpui::rgb(0x6b7280).into(),
            SessionStatus::InProgress => gpui::rgb(0x3b82f6).into(),
            SessionStatus::Completed => gpui::rgb(0x22c55e).into(),
            SessionStatus::Failed => gpui::rgb(0xef4444).into(),
            SessionStatus::Closed => gpui::rgb(0xef4444).into(),
        }
    }
}

impl Focusable for TaskPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TaskPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("task-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .child(self.render_header(cx))
            .child(match self.view_mode {
                ViewMode::Tree => self.render_tree_view(cx).into_any_element(),
                ViewMode::Timeline => self.render_timeline_view(cx).into_any_element(),
            })
            .child(self.render_footer(cx))
    }
}
