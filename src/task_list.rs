use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    div, px, Action, App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, SharedString, Styled, Subscription, Task,
    Timer, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
    list::{List, ListDelegate, ListEvent, ListState},
    popover::Popover,
    v_flex, ActiveTheme, Icon, IconName, IndexPath,
};

use agent_client_protocol_schema::{ContentBlock, SessionUpdate};

use crate::app::actions::{AddSessionToList, SelectedAgentTask};
use crate::components::TaskListItem;
use crate::task_data::{load_mock_tasks, random_status};
use crate::task_schema::{AgentTask, TaskStatus};
use crate::{AppState, CreateTaskFromWelcome, ShowConversationPanel, ShowWelcomePanel};

struct TaskListDelegate {
    industries: Vec<SharedString>,
    _agent_tasks: Vec<Rc<AgentTask>>,
    matched_agent_tasks: Vec<Vec<Rc<AgentTask>>>,
    selected_index: Option<IndexPath>,
    confirmed_index: Option<IndexPath>,
    query: SharedString,
    loading: bool,
    eof: bool,
    lazy_load: bool,
    // Track which sections are collapsed (using RefCell for interior mutability)
    collapsed_sections: Rc<RefCell<HashSet<usize>>>,
    // Store weak reference to list state to notify on collapse toggle
    list_state: Option<gpui::WeakEntity<ListState<TaskListDelegate>>>,
}

impl TaskListDelegate {
    fn is_section_collapsed(&self, section: usize) -> bool {
        self.collapsed_sections.borrow().contains(&section)
    }

    fn prepare(&mut self, query: impl Into<SharedString>) {
        self.query = query.into();
        // Clear previous data before rebuilding
        self.industries.clear();
        self.matched_agent_tasks.clear();

        let agent_tasks: Vec<Rc<AgentTask>> = self
            ._agent_tasks
            .iter()
            .filter(|agent_task| {
                agent_task
                    .name
                    .to_lowercase()
                    .contains(&self.query.to_lowercase())
            })
            .cloned()
            .collect();
        for agent_task in agent_tasks.into_iter() {
            if let Some(ix) = self
                .industries
                .iter()
                .position(|s| s.as_ref() == agent_task.task_type.as_str())
            {
                self.matched_agent_tasks[ix].push(agent_task);
            } else {
                self.industries.push(agent_task.task_type.clone().into());
                self.matched_agent_tasks.push(vec![agent_task]);
            }
        }
    }

    fn load_all_tasks(&mut self) {
        let tasks = load_mock_tasks();
        self._agent_tasks = tasks.into_iter().map(Rc::new).collect();
        self.prepare(self.query.clone());
    }

    fn extend_more(&mut self, _len: usize) {
        // For mock data, we just use the initial JSON load
        // If we want to support pagination/lazy loading, we could cycle through tasks
        // For now, just do nothing as all tasks are loaded initially
    }

    fn selected_agent_task(&self) -> Option<Rc<AgentTask>> {
        let Some(ix) = self.selected_index else {
            return None;
        };

        self.matched_agent_tasks
            .get(ix.section)
            .and_then(|c| c.get(ix.row))
            .cloned()
    }
}

impl ListDelegate for TaskListDelegate {
    type Item = TaskListItem;

    fn sections_count(&self, _: &App) -> usize {
        self.industries.len()
    }

    fn items_count(&self, section: usize, _: &App) -> usize {
        // Return 0 items if the section is collapsed
        if self.is_section_collapsed(section) {
            0
        } else {
            self.matched_agent_tasks[section].len()
        }
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.prepare(query.to_owned());
        Task::ready(())
    }

    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        println!("Confirmed with secondary: {}", secondary);
        window.dispatch_action(Box::new(SelectedAgentTask), cx);
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_section_header(
        &self,
        section: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement> {
        let Some(task_type) = self.industries.get(section) else {
            return None;
        };

        let is_collapsed = self.is_section_collapsed(section);
        let collapsed_sections = self.collapsed_sections.clone();
        let list_state = self.list_state.clone();

        // Use ChevronRight when collapsed, ChevronDown when expanded
        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .pb_1()
                .px_2()
                .gap_2()
                .text_sm()
                .rounded(cx.theme().radius)
                // Left side: collapsible section header
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .text_color(cx.theme().muted_foreground)
                        .cursor_default()
                        .hover(|style| style.bg(cx.theme().secondary))
                        .rounded(cx.theme().radius)
                        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                            // Toggle the collapsed state
                            let mut collapsed = collapsed_sections.borrow_mut();
                            if collapsed.contains(&section) {
                                collapsed.remove(&section);
                            } else {
                                collapsed.insert(section);
                            }
                            drop(collapsed); // Release the borrow before updating

                            // Notify the list state to re-render
                            if let Some(list_state) = list_state.as_ref() {
                                _ = list_state.update(cx, |_, cx| {
                                    cx.notify();
                                });
                            }
                        })
                        .child(Icon::new(chevron_icon).size(px(14.)))
                        .child(Icon::new(IconName::Folder))
                        .child(task_type.clone()),
                )
                // Right side: add task button
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(20.))
                        .h(px(20.))
                        .rounded(px(4.))
                        .cursor_default()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|style| {
                            style
                                .bg(cx.theme().accent)
                                .text_color(cx.theme().accent_foreground)
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _window, _cx| {
                            println!("Add new task to section: {}", section);
                            // TODO: Implement add task functionality
                        })
                        .child(Icon::new(IconName::Plus).size(px(14.))),
                ),
        )
    }

    fn render_item(&self, ix: IndexPath, _: &mut Window, _: &mut App) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index || Some(ix) == self.confirmed_index;
        if let Some(agent_task) = self.matched_agent_tasks[ix.section].get(ix.row) {
            return Some(TaskListItem::new(ix, agent_task.clone(), selected));
        }

        None
    }

    fn render_empty(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // Check if we have sections but all are collapsed
        let has_collapsed_sections = !self.industries.is_empty()
            && self.industries.len() == self.collapsed_sections.borrow().len();

        if has_collapsed_sections {
            // Render section headers so user can expand them
            let collapsed_sections = self.collapsed_sections.clone();
            let list_state = self.list_state.clone();

            v_flex()
                .w_full()
                .gap_1()
                .children(
                    self.industries
                        .iter()
                        .enumerate()
                        .map(|(section, task_type)| {
                            let collapsed_sections = collapsed_sections.clone();
                            let list_state = list_state.clone();
                            let task_type = task_type.clone();

                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .pb_1()
                                .px_2()
                                .gap_2()
                                .text_sm()
                                .rounded(cx.theme().radius)
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .flex_1()
                                        .text_color(cx.theme().muted_foreground)
                                        .cursor_default()
                                        .hover(|style| style.bg(cx.theme().secondary))
                                        .rounded(cx.theme().radius)
                                        .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                            // Expand the section
                                            collapsed_sections.borrow_mut().remove(&section);

                                            if let Some(list_state) = list_state.as_ref() {
                                                _ = list_state.update(cx, |_, cx| {
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child(Icon::new(IconName::ChevronRight).size(px(14.)))
                                        .child(Icon::new(IconName::Folder))
                                        .child(task_type),
                                )
                        }),
                )
                .into_any_element()
        } else {
            // Default empty state
            h_flex()
                .size_full()
                .justify_center()
                .text_color(cx.theme().muted_foreground.opacity(0.6))
                .child(Icon::new(IconName::Inbox).size_12())
                .into_any_element()
        }
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn is_eof(&self, _: &App) -> bool {
        return !self.loading && !self.eof;
    }

    fn load_more_threshold(&self) -> usize {
        150
    }

    fn load_more(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        if !self.lazy_load {
            return;
        }

        cx.spawn_in(window, async move |view, window| {
            // Simulate network request, delay 1s to load data.
            Timer::after(Duration::from_secs(1)).await;

            _ = view.update_in(window, move |view, window, cx| {
                let query = view.delegate().query.clone();
                view.delegate_mut().extend_more(200);
                _ = view.delegate_mut().perform_search(&query, window, cx);
                view.delegate_mut().eof = view.delegate()._agent_tasks.len() >= 6000;
            });
        })
        .detach();
    }
}

pub struct ListTaskPanel {
    focus_handle: FocusHandle,
    task_list: Entity<ListState<TaskListDelegate>>,
    selected_agent_task: Option<Rc<AgentTask>>,
    _subscriptions: Vec<Subscription>,
}

impl crate::dock_panel::DockPanel for ListTaskPanel {
    fn title() -> &'static str {
        "Tasks"
    }

    fn description() -> &'static str {
        "A list displays a series of items."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
}

impl ListTaskPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        // Subscribe to session bus for all session updates
        Self::subscribe_to_session_updates(&entity, cx);

        entity
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut delegate = TaskListDelegate {
            industries: vec![],
            matched_agent_tasks: vec![],
            _agent_tasks: vec![],
            selected_index: Some(IndexPath::default()),
            confirmed_index: None,
            query: "".into(),
            loading: false,
            eof: false,
            lazy_load: false,
            collapsed_sections: Rc::new(RefCell::new(HashSet::new())),
            list_state: None,
        };
        delegate.load_all_tasks();

        let task_list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

        // Set the weak reference to the list state in the delegate
        task_list.update(cx, |list, _| {
            list.delegate_mut().list_state = Some(task_list.downgrade());
        });

        let _subscriptions = vec![cx.subscribe_in(
            &task_list,
            window,
            |_this, _, ev: &ListEvent, window, cx| match ev {
                ListEvent::Select(ix) => {
                    println!("List Selected: {:?}", ix);
                }
                ListEvent::Confirm(ix) => {
                    println!("List Confirmed: {:?}", ix);
                    // Dispatch action to show conversation panel
                    window.dispatch_action(Box::new(ShowConversationPanel), cx);
                }
                ListEvent::Cancel => {
                    println!("List Cancelled");
                }
            },
        )];

        // Spawn a background task to randomly update task status for demo
        cx.spawn(async move |this, cx| {
            this.update(cx, |this, cx| {
                this.task_list.update(cx, |picker, _| {
                    picker
                        .delegate_mut()
                        ._agent_tasks
                        .iter_mut()
                        .for_each(|agent_task| {
                            // Clone the task and update its status
                            let mut updated_task = (**agent_task).clone();
                            updated_task.status = random_status();
                            *agent_task = Rc::new(updated_task.prepare());
                        });
                    picker.delegate_mut().prepare("");
                });
                cx.notify();
            })
            .ok();
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            task_list,
            selected_agent_task: None,
            _subscriptions,
        }
    }

    /// Subscribe to session bus to update task subtitles with message previews
    fn subscribe_to_session_updates(entity: &Entity<Self>, cx: &mut App) {
        let weak_entity = entity.downgrade();
        let session_bus = AppState::global(cx).session_bus.clone();

        // Create channel for cross-thread communication
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, SessionUpdate)>();

        // Subscribe to all session updates
        session_bus.subscribe(move |event| {
            let _ = tx.send((event.session_id.clone(), (*event.update).clone()));
        });

        // Spawn background task to receive updates and update task subtitles
        cx.spawn(async move |cx| {
            while let Some((session_id, update)) = rx.recv().await {
                let weak = weak_entity.clone();
                let _ = cx.update(|cx| {
                    if let Some(entity) = weak.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.handle_session_update(session_id, update, cx);
                        });
                    }
                });
            }
        })
        .detach();

        log::info!("ListTaskPanel subscribed to session bus");
    }

    /// Handle session updates and update task subtitles
    fn handle_session_update(
        &mut self,
        session_id: String,
        update: SessionUpdate,
        cx: &mut Context<Self>,
    ) {
        // Extract text from the update
        let text = match &update {
            SessionUpdate::UserMessageChunk(chunk) => {
                Self::extract_text_from_content(&chunk.content)
            }
            SessionUpdate::AgentMessageChunk(chunk) => {
                Self::extract_text_from_content(&chunk.content)
            }
            _ => return, // Ignore other update types
        };

        if text.is_empty() {
            return;
        }

        // Update the task with matching session_id
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            let mut found = false;

            // Find and update the task
            for task in delegate._agent_tasks.iter_mut() {
                if task.session_id.as_ref() == Some(&session_id) {
                    let mut updated_task = (**task).clone();
                    // Truncate text to ~50 characters for subtitle
                    let preview = if text.len() > 50 {
                        format!("{}...", &text[..50])
                    } else {
                        text.clone()
                    };
                    updated_task.update_subtitle(preview);
                    *task = Rc::new(updated_task);
                    found = true;
                    break;
                }
            }

            if found {
                delegate.prepare("");
                cx.notify();
                log::debug!("Updated task subtitle for session: {}", session_id);
            }
        });
    }

    /// Extract text from ContentBlock
    fn extract_text_from_content(content: &ContentBlock) -> String {
        match content {
            ContentBlock::Text(text_content) => text_content.text.clone(),
            _ => String::new(),
        }
    }

    fn selected_agent_task(
        &mut self,
        _: &SelectedAgentTask,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = self.task_list.read(cx);
        if let Some(agent_task) = picker.delegate().selected_agent_task() {
            self.selected_agent_task = Some(agent_task);
        }
    }

    /// Handle action to create a new task from the welcome panel
    fn on_create_task_from_welcome(
        &mut self,
        action: &CreateTaskFromWelcome,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task_name = action.task_input.clone();

        // Create a new task with InProgress status
        let new_task = AgentTask {
            name: task_name,
            task_type: "Conversation".to_string(),
            add_new_code_lines: 0,
            delete_code_lines: 0,
            status: TaskStatus::InProgress,
            session_id: None,
            subtitle: None,
            change_timestamp: 0,
            change_timestamp_str: "".into(),
            add_new_code_lines_str: "+0".into(),
            delete_code_lines_str: "-0".into(),
        }
        .prepare();

        // Add task to the beginning of the list
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            delegate._agent_tasks.insert(0, Rc::new(new_task));
            delegate.prepare("");
            cx.notify();
        });
    }

    /// Handle action to add a new session to the list
    fn on_add_session_to_list(
        &mut self,
        action: &AddSessionToList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!(
            "Received AddSessionToList action: session_id={}, task_name={}",
            action.session_id,
            action.task_name
        );

        let task_name = action.task_name.clone();
        let session_id = action.session_id.clone();

        // Create a new task for this session
        let new_task = AgentTask::new_for_session(task_name, session_id.clone());

        // Add task to the beginning of the list in the "Default" section
        self.task_list.update(cx, |list, cx| {
            let delegate = list.delegate_mut();
            delegate._agent_tasks.insert(0, Rc::new(new_task));
            delegate.prepare("");
            cx.notify();
        });

        log::info!("Added session to list: {}", session_id);
    }

    /// Handle click on "New Task" button - shows the welcome panel
    fn on_new_task_click(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Ensure this panel has focus before dispatching action
        window.focus(&self.focus_handle);
        window.dispatch_action(Box::new(ShowWelcomePanel), cx);
    }
}

impl Focusable for ListTaskPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ListTaskPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let lazy_load = self.task_list.read(cx).delegate().lazy_load;

        v_flex()
            .child(
                Button::new("btn-new-task")
                    .label("New Task")
                    .primary()
                    .icon(Icon::new(IconName::Plus))
                    .on_click(cx.listener(Self::on_new_task_click)),
            )
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::selected_agent_task))
            .on_action(cx.listener(Self::on_create_task_from_welcome))
            .on_action(cx.listener(Self::on_add_session_to_list))
            .size_full()
            .gap_4()
            .child(
                List::new(&self.task_list)
                    .p(px(8.))
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius),
            )
            // Bottom action buttons with popover
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        Popover::new("add-repository-popover")
                            .trigger(
                                Button::new("btn-add-repository")
                                    .label("Add repository")
                                    .icon(Icon::new(IconName::Plus))
                                    .ghost(),
                            )
                            .content(|_state, _window, cx| {
                                let popover_entity = cx.entity();
                                v_flex()
                                    .gap_1()
                                    .min_w(px(200.))
                                    .child(
                                        // Open project button
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .px_3()
                                            .py_2()
                                            .rounded(cx.theme().radius)
                                            .cursor_default()
                                            .hover(|style| style.bg(cx.theme().secondary))
                                            .on_mouse_down(MouseButton::Left, {
                                                let popover = popover_entity.clone();
                                                move |_, window, cx| {
                                                    // Close the popover first
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });

                                                    // Then spawn the folder picker
                                                    cx.spawn(async move |_cx| {
                                                        let folder = rfd::AsyncFileDialog::new()
                                                            .set_title("Select Project Folder")
                                                            .pick_folder()
                                                            .await;

                                                        if let Some(folder) = folder {
                                                            let path = folder.path();
                                                            println!("Selected folder: {}", path.display());
                                                            log::info!("Selected project folder: {}", path.display());
                                                        } else {
                                                            println!("No folder selected");
                                                            log::info!("Folder selection cancelled");
                                                        }
                                                    })
                                                    .detach();
                                                }
                                            })
                                            .child(Icon::new(IconName::Folder).size(px(16.)))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().foreground)
                                                    .child("Open project"),
                                            ),
                                    )
                                    .child(
                                        // Clone from URL button
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .px_3()
                                            .py_2()
                                            .rounded(cx.theme().radius)
                                            .cursor_default()
                                            .hover(|style| style.bg(cx.theme().secondary))
                                            .on_mouse_down(MouseButton::Left, {
                                                let popover = popover_entity.clone();
                                                move |_, window, cx| {
                                                    // Close the popover
                                                    popover.update(cx, |state, cx| {
                                                        state.dismiss(window, cx);
                                                    });

                                                    println!("Clone from URL clicked");
                                                    // TODO: Implement clone from URL functionality
                                                }
                                            })
                                            .child(Icon::new(IconName::Globe).size(px(16.)))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().foreground)
                                                    .child("Clone from URL"),
                                            ),
                                    )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("btn-notifications")
                                    .icon(Icon::new(IconName::Bell))
                                    .ghost()
                                    .on_click(|_, _, _| {
                                        println!("Notifications clicked");
                                        // TODO: Implement notifications functionality
                                    }),
                            )
                            .child(
                                Button::new("btn-settings")
                                    .icon(Icon::new(IconName::Settings))
                                    .ghost()
                                    .on_click(|_, _, _| {
                                        println!("Settings clicked");
                                        // TODO: Implement settings functionality
                                    }),
                            ),
                    ),
            )
    }
}
