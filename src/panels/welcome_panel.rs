use gpui::{
    px, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, Styled, Subscription, Window,
};

use gpui_component::{
    input::InputState,
    list::{ListDelegate, ListItem, ListState},
    select::{SelectEvent, SelectState},
    v_flex, ActiveTheme, IndexPath, StyledExt,
};

use crate::{components::ChatInputBox, AppState, CreateTaskFromWelcome, WelcomeSession};

/// Delegate for the context list in the chat input popover
struct ContextListDelegate {
    items: Vec<ContextItem>,
}

#[derive(Clone)]
struct ContextItem {
    name: &'static str,
    icon: &'static str,
}

impl ContextListDelegate {
    fn new() -> Self {
        Self {
            items: vec![
                ContextItem {
                    name: "Files",
                    icon: "file",
                },
                ContextItem {
                    name: "Folders",
                    icon: "folder",
                },
                ContextItem {
                    name: "Code",
                    icon: "code",
                },
                ContextItem {
                    name: "Git Changes",
                    icon: "git-branch",
                },
                ContextItem {
                    name: "Terminal",
                    icon: "terminal",
                },
                ContextItem {
                    name: "Problems",
                    icon: "alert-circle",
                },
                ContextItem {
                    name: "URLs",
                    icon: "link",
                },
            ],
        }
    }
}

impl ListDelegate for ContextListDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.items.len()
    }

    fn render_item(&mut self, ix: IndexPath, _: &mut Window, _: &mut gpui::Context<'_, gpui_component::list::ListState<ContextListDelegate>>) -> Option<Self::Item> {
        let item = self.items.get(ix.row)?;
        Some(ListItem::new(ix).child(item.name))
    }

    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }

    fn confirm(&mut self, _: bool, _: &mut Window, _cx: &mut Context<ListState<Self>>) {
        // Handle item selection - for now just close the popover
    }

    fn cancel(&mut self, _: &mut Window, _cx: &mut Context<ListState<Self>>) {
        // Close the popover on cancel
    }
}

/// Welcome panel displayed when creating a new task.
/// Shows a centered input form with title, instructions, and send button.
pub struct WelcomePanel {
    focus_handle: FocusHandle,
    input_state: Entity<InputState>,
    context_list: Entity<ListState<ContextListDelegate>>,
    context_popover_open: bool,
    mode_select: Entity<SelectState<Vec<&'static str>>>,
    agent_select: Entity<SelectState<Vec<String>>>,
    session_select: Entity<SelectState<Vec<String>>>,
    current_session_id: Option<String>,
    has_agents: bool,
    has_workspace: bool,
    active_workspace_name: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl crate::panels::dock_panel::DockPanel for WelcomePanel {
    fn title() -> &'static str {
        "Welcome"
    }

    fn description() -> &'static str {
        "Welcome panel for creating new tasks"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> gpui::Pixels {
        px(0.)
    }
}

impl WelcomePanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        // Subscribe to agent_select focus to refresh agents list when no agents available
        entity.update(cx, |this, cx| {
            let agent_select_focus = this.agent_select.focus_handle(cx);
            let subscription = cx.on_focus(
                &agent_select_focus,
                window,
                |this: &mut Self, window, cx| {
                    this.try_refresh_agents(window, cx);
                },
            );
            this._subscriptions.push(subscription);

            // Refresh sessions when agent_select loses focus (agent selection changed)
            let subscription = cx.on_focus_lost(window, |this: &mut Self, window, cx| {
                this.on_agent_changed(window, cx);
            });
            this._subscriptions.push(subscription);

            // Subscribe to session_select changes to update welcome_session
            let session_select_sub = cx.subscribe_in(
                &this.session_select,
                window,
                |this, _, _: &SelectEvent<Vec<String>>, _window, cx| {
                    this.on_session_changed(cx);
                },
            );
            this._subscriptions.push(session_select_sub);
        });

        // Load workspace info
        Self::load_workspace_info(&entity, cx);

        entity
    }

    /// Load workspace info from WorkspaceService
    fn load_workspace_info(entity: &Entity<Self>, cx: &mut App) {
        let workspace_service = match AppState::global(cx).workspace_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let weak_entity = entity.downgrade();
        cx.spawn(async move |cx| {
            // Get active workspace
            let active_workspace = workspace_service.get_active_workspace().await;

            // Update UI
            _ = cx.update(|cx| {
                if let Some(entity) = weak_entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.has_workspace = active_workspace.is_some();
                        this.active_workspace_name = active_workspace.map(|ws| ws.name);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 8) // Auto-grow from 2 to 8 rows
                .soft_wrap(true) // Enable word wrapping
                .placeholder("Describe what you'd like to build...")
        });

        let context_list =
            cx.new(|cx| ListState::new(ContextListDelegate::new(), window, cx).searchable(true));

        let mode_select = cx.new(|cx| {
            SelectState::new(
                vec!["Auto", "Ask", "Plan", "Code", "Explain"],
                Some(IndexPath::default()), // Select "Auto" by default
                window,
                cx,
            )
        });

        // Get available agents from AppState
        let agents = AppState::global(cx)
            .agent_manager()
            .map(|m| m.list_agents())
            .unwrap_or_default();

        let has_agents = !agents.is_empty();

        // Save first agent name for initializing sessions
        let first_agent = agents.first().cloned();

        // Default to first agent if available
        let default_agent = if has_agents {
            Some(IndexPath::default())
        } else {
            None
        };

        // Use placeholder if no agents available
        let agent_list = if has_agents {
            agents
        } else {
            vec!["No agents".to_string()]
        };

        let agent_select = cx.new(|cx| SelectState::new(agent_list, default_agent, window, cx));

        // Initialize session selector (initially empty)
        let session_select =
            cx.new(|cx| SelectState::new(vec!["No sessions".to_string()], None, window, cx));

        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            input_state,
            context_list,
            context_popover_open: false,
            mode_select,
            agent_select,
            session_select,
            current_session_id: None,
            has_agents,
            has_workspace: false,
            active_workspace_name: None,
            _subscriptions: Vec::new(),
        };

        // Load sessions for the initially selected agent if any
        if has_agents {
            if let Some(initial_agent) = first_agent {
                panel.refresh_sessions_for_agent(&initial_agent, window, cx);
            }
        }

        panel
    }

    /// Try to refresh agents list from AppState if we don't have agents yet
    fn try_refresh_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_agents {
            return;
        }

        let agents = AppState::global(cx)
            .agent_manager()
            .map(|m| m.list_agents())
            .unwrap_or_default();

        if agents.is_empty() {
            return;
        }

        // We now have agents, update the select
        self.has_agents = true;
        self.agent_select.update(cx, |state, cx| {
            state.set_items(agents, window, cx);
            state.set_selected_index(Some(IndexPath::default()), window, cx);
        });
        cx.notify();
    }

    /// Handle agent selection change - refresh sessions for the newly selected agent
    fn on_agent_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != "No agents" => name,
            _ => {
                // No valid agent selected, clear sessions
                self.session_select.update(cx, |state, cx| {
                    state.set_items(vec!["No sessions".to_string()], window, cx);
                    state.set_selected_index(None, window, cx);
                });
                self.current_session_id = None;
                AppState::global_mut(cx).clear_welcome_session();
                cx.notify();
                return;
            }
        };

        // Refresh sessions for the newly selected agent
        self.refresh_sessions_for_agent(&agent_name, window, cx);
    }

    /// Handle session selection change - update welcome_session
    fn on_session_changed(&mut self, cx: &mut Context<Self>) {
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != "No agents" => name,
            _ => return,
        };

        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        // Get the selected session index
        let selected_index = match self.session_select.read(cx).selected_index(cx) {
            Some(idx) => idx.row,
            None => return,
        };

        // Get all sessions for this agent
        let sessions = agent_service.list_sessions_for_agent(&agent_name);

        // Get the selected session
        if let Some(selected_session) = sessions.get(selected_index) {
            self.current_session_id = Some(selected_session.session_id.clone());

            // Update welcome session
            AppState::global_mut(cx).set_welcome_session(WelcomeSession {
                session_id: selected_session.session_id.clone(),
                agent_name: agent_name.clone(),
            });

            log::info!(
                "[WelcomePanel] Session changed to: {} for agent: {}",
                selected_session.session_id,
                agent_name
            );
        }
    }

    /// Refresh sessions for the currently selected agent
    fn refresh_sessions_for_agent(
        &mut self,
        agent_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let sessions = agent_service.list_sessions_for_agent(agent_name);

        if sessions.is_empty() {
            // No sessions for this agent
            self.session_select.update(cx, |state, cx| {
                state.set_items(vec!["No sessions".to_string()], window, cx);
                state.set_selected_index(None, window, cx);
            });
            self.current_session_id = None;

            // Clear welcome session when no sessions available
            AppState::global_mut(cx).clear_welcome_session();
        } else {
            // Display sessions (show first 8 chars of session ID)
            let session_display: Vec<String> = sessions
                .iter()
                .map(|s| {
                    let short_id = if s.session_id.len() > 8 {
                        &s.session_id[..8]
                    } else {
                        &s.session_id
                    };
                    format!("Session {}", short_id)
                })
                .collect();

            self.session_select.update(cx, |state, cx| {
                state.set_items(session_display, window, cx);
                state.set_selected_index(Some(IndexPath::default()), window, cx);
            });

            // Set current session to the first one
            if let Some(first_session) = sessions.first() {
                self.current_session_id = Some(first_session.session_id.clone());

                // Store as welcome session for CreateTaskFromWelcome action
                AppState::global_mut(cx).set_welcome_session(WelcomeSession {
                    session_id: first_session.session_id.clone(),
                    agent_name: agent_name.to_string(),
                });
            }
        }

        cx.notify();
    }

    /// Create a new session for the currently selected agent
    fn create_new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let agent_name = match self.agent_select.read(cx).selected_value().cloned() {
            Some(name) if name != "No agents" => name,
            _ => return,
        };

        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => return,
        };

        let weak_self = cx.entity().downgrade();
        let agent_name_for_session = agent_name.clone();
        cx.spawn_in(window, async move |_this, window| {
            match agent_service.create_session(&agent_name).await {
                Ok(session_id) => {
                    log::info!("[WelcomePanel] Created new session: {}", session_id);
                    _ = window.update(|window, cx| {
                        // Store as welcome session immediately
                        AppState::global_mut(cx).set_welcome_session(WelcomeSession {
                            session_id: session_id.clone(),
                            agent_name: agent_name_for_session.clone(),
                        });

                        // Update UI
                        if let Some(this) = weak_self.upgrade() {
                            this.update(cx, |this, cx| {
                                this.current_session_id = Some(session_id.clone());
                                this.refresh_sessions_for_agent(
                                    &agent_name_for_session,
                                    window,
                                    cx,
                                );
                            });
                        }
                    });
                }
                Err(e) => {
                    log::error!("[WelcomePanel] Failed to create session: {}", e);
                }
            }
        })
        .detach();
    }

    /// Handles sending the task based on the current input, mode, and agent selections.
    fn handle_send_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Check if workspace exists
        if !self.has_workspace {
            log::warn!("[WelcomePanel] Cannot create task: No workspace available");
            // TODO: Show user-facing notification/toast
            return;
        }

        let task_name = self.input_state.read(cx).text().to_string();

        if !task_name.is_empty() {
            let mode = self
                .mode_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or("Auto")
                .to_string();

            let agent_name = self
                .agent_select
                .read(cx)
                .selected_value()
                .cloned()
                .unwrap_or_else(|| "test-agent".to_string());

            let agent_name = if agent_name == "No agents" {
                "test-agent".to_string()
            } else {
                agent_name
            };

            // Clear the input immediately
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });

            // Dispatch CreateTaskFromWelcome action
            let action = CreateTaskFromWelcome {
                task_input: task_name.clone(),
                agent_name: agent_name.clone(),
                mode,
            };

            window.dispatch_action(Box::new(action), cx);
        }
    }
}

impl Focusable for WelcomePanel {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WelcomePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(800.)) // Maximum width for better readability
                    .gap_4()
                    .child(
                        // Welcome title and subtitle
                        v_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .px(px(32.))
                            .child(
                                gpui::div()
                                    .text_2xl()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child("Welcome to Agent Studio"),
                            )
                            .child(
                                gpui::div()
                                    .text_base()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        if self.has_workspace {
                                            if let Some(workspace_name) = &self.active_workspace_name {
                                                format!("Current workspace: {} - Start by describing what you'd like to build", workspace_name)
                                            } else {
                                                "Start by describing what you'd like to build".to_string()
                                            }
                                        } else {
                                            "Please add a workspace first by clicking 'Add repository' in the left panel".to_string()
                                        }
                                    ),
                            ),
                    )
                    .child(
                        // Chat input with title and send handler
                        ChatInputBox::new("welcome-chat-input", self.input_state.clone())
                            // .title("New Task")
                            .context_list(self.context_list.clone(), cx)
                            .context_popover_open(self.context_popover_open)
                            .on_context_popover_change(cx.listener(|this, open: &bool, _, cx| {
                                this.context_popover_open = *open;
                                cx.notify();
                            }))
                            .mode_select(self.mode_select.clone())
                            .agent_select(self.agent_select.clone())
                            .session_select(self.session_select.clone())
                            .on_new_session(cx.listener(|this, _, window, cx| {
                                this.create_new_session(window, cx);
                            }))
                            .on_send(cx.listener(|this, _, window, cx| {
                                this.handle_send_task(window, cx);
                            })),
                    ),
            )
    }
}
