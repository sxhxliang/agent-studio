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
use agent_client_protocol as acp;

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

    fn render_item(&self, ix: IndexPath, _: &mut Window, _: &mut App) -> Option<Self::Item> {
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
    has_agents: bool,
    _subscriptions: Vec<Subscription>,
}

impl crate::dock_panel::DockPanel for WelcomePanel {
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

            // Subscribe to agent_select selection changes to create session
            let agent_select_sub = cx.subscribe_in(
                &this.agent_select,
                window,
                |this, _, _: &SelectEvent<Vec<String>>, window, cx| {
                    this.on_agent_selected(window, cx);
                },
            );
            this._subscriptions.push(agent_select_sub);
        });

        entity
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

        Self {
            focus_handle: cx.focus_handle(),
            input_state,
            context_list,
            context_popover_open: false,
            mode_select,
            agent_select,
            has_agents,
            _subscriptions: Vec::new(),
        }
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

    /// Called when agent is selected - creates a new session
    fn on_agent_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Skip if no agents available
        if !self.has_agents {
            return;
        }

        let agent_name = self
            .agent_select
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_else(|| "test-agent".to_string());

        // Skip placeholder
        if agent_name == "No agents" {
            return;
        }

        // Get agent handle
        let agent_handle = match AppState::global(cx)
            .agent_manager()
            .and_then(|m| m.get(&agent_name))
        {
            Some(handle) => handle,
            None => {
                log::warn!("Agent not found: {}", agent_name);
                return;
            }
        };

        log::info!("Agent selected: {}, creating session...", agent_name);

        // Create session asynchronously
        cx.spawn_in(window, async move |_this, window| {
            let new_session_req = acp::NewSessionRequest {
                cwd: std::env::current_dir().unwrap_or_default(),
                mcp_servers: vec![],
                meta: None,
            };

            match agent_handle.new_session(new_session_req).await {
                Ok(resp) => {
                    let session_id = resp.session_id.to_string();
                    log::info!("Session created: {} for agent: {}", session_id, agent_name);

                    // Store session in AppState
                    _ = window.update(move |_, cx| {
                        AppState::global_mut(cx).set_welcome_session(WelcomeSession {
                            session_id,
                            agent_name,
                        });
                    });
                }
                Err(e) => {
                    log::error!("Failed to create session for agent {}: {}", agent_name, e);
                }
            }
        })
        .detach();
    }

    /// Handles sending the task based on the current input, mode, and agent selections.
    fn handle_send_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

            // Dispatch CreateTaskFromWelcome action
            let action = CreateTaskFromWelcome {
                task_input: task_name.clone(),
                agent_name,
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
                                    .child("Start by describing what you'd like to build"),
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
                            .on_send(cx.listener(|this, _, window, cx| {
                                this.handle_send_task(window, cx);
                            })),
                    ),
            )
    }
}
