use std::collections::HashMap;

use gpui::{
    px, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Pixels, Render, Styled, Subscription, Window,
};

use gpui_component::{
    input::InputState,
    list::{ListDelegate, ListItem, ListState},
    select::SelectState,
    v_flex, ActiveTheme, IndexPath,
};

use crate::components::ChatInputBox;
use crate::AppState;

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

pub struct ChatInputPanel {
    focus_handle: FocusHandle,
    input_state: Entity<InputState>,
    context_list: Entity<ListState<ContextListDelegate>>,
    context_popover_open: bool,
    mode_select: Entity<SelectState<Vec<&'static str>>>,
    agent_select: Entity<SelectState<Vec<String>>>,
    has_agents: bool,
    /// Map of agent name -> session ID
    sessions: HashMap<String, String>,
    _subscriptions: Vec<Subscription>,
}

impl crate::dock_panel::DockPanel for ChatInputPanel {
    fn title() -> &'static str {
        "Chat Input"
    }

    fn description() -> &'static str {
        "A chat input box for sending messages."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }
    fn paddings() -> Pixels {
        px(0.)
    }
}

impl ChatInputPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|cx| Self::new(window, cx));

        // Subscribe to agent_select focus to refresh agents list when no agents available
        entity.update(cx, |this, cx| {
            let agent_select_focus = this.agent_select.focus_handle(cx);
            let subscription =
                cx.on_focus(&agent_select_focus, window, |this: &mut Self, window, cx| {
                    this.try_refresh_agents(window, cx);
                });
            this._subscriptions.push(subscription);
        });

        entity
    }

    fn new(window: &mut Window, cx: &mut App) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(2, 8) // Auto-grow from 2 to 8 rows
                .soft_wrap(true) // Enable word wrapping
                .placeholder("Ask, search, or make anything...")
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

        let agent_select =
            cx.new(|cx| SelectState::new(agent_list, default_agent, window, cx));

        Self {
            focus_handle: cx.focus_handle(),
            input_state,
            context_list,
            context_popover_open: false,
            mode_select,
            agent_select,
            has_agents,
            sessions: HashMap::new(),
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

    /// Send message to the selected agent
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Get the selected agent name
        let agent_name = self
            .agent_select
            .read(cx)
            .selected_value()
            .cloned();

        let agent_name = match agent_name {
            Some(name) if name != "No agents" => name,
            _ => {
                eprintln!("No agent selected");
                return;
            }
        };

        // Get the input text
        let input_text = self.input_state.read(cx).value().to_string();
        if input_text.trim().is_empty() {
            return;
        }

        // Get the agent handle
        let agent_handle = AppState::global(cx)
            .agent_manager()
            .and_then(|m| m.get(&agent_name));

        let agent_handle = match agent_handle {
            Some(handle) => handle,
            None => {
                eprintln!("Agent not found: {}", agent_name);
                return;
            }
        };

        // Check if we have an existing session for this agent
        let existing_session = self.sessions.get(&agent_name).cloned();

        // Clear the input immediately
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });

        // Spawn async task to send the message
        let sessions_update = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            use agent_client_protocol as acp;

            // Create a new session if needed
            let session_id = if let Some(sid) = existing_session {
                sid
            } else {
                let request = acp::NewSessionRequest {
                    cwd: std::env::current_dir().unwrap_or_default(),
                    mcp_servers: vec![],
                    meta: None,
                };

                match agent_handle.new_session(request).await {
                    Ok(resp) => {
                        let sid = resp.session_id.to_string();
                        println!("[{}] Created new session: {}", agent_name, sid);

                        // Store the session ID
                        let agent_name_clone = agent_name.clone();
                        let sid_clone = sid.clone();
                        cx.update(|cx| {
                            if let Some(entity) = sessions_update.upgrade() {
                                entity.update(cx, |this, _| {
                                    this.sessions.insert(agent_name_clone, sid_clone);
                                });
                            }
                        })
                        .ok();

                        sid
                    }
                    Err(e) => {
                        eprintln!("[{}] Failed to create session: {}", agent_name, e);
                        return;
                    }
                }
            };

            // Immediately publish user message to session bus for instant UI feedback
            use agent_client_protocol_schema as schema;
            use std::sync::Arc;

            // Create user message chunk using the correct ContentChunk API
            let content_block = schema::ContentBlock::from(input_text.clone());
            let content_chunk = schema::ContentChunk::new(content_block);

            let user_event = crate::session_bus::SessionUpdateEvent {
                session_id: session_id.clone(),
                update: Arc::new(schema::SessionUpdate::UserMessageChunk(content_chunk)),
            };

            // Publish to session bus
            cx.update(|cx| {
                AppState::global(cx).session_bus.publish(user_event);
            })
            .ok();
            log::info!("Published user message to session bus: {}", session_id);

            // Send the prompt
            let request = acp::PromptRequest {
                session_id: acp::SessionId::from(session_id),
                prompt: vec![input_text.into()],
                meta: None,
            };

            match agent_handle.prompt(request).await {
                Ok(_) => {
                    println!("[{}] Prompt sent successfully", agent_name);
                }
                Err(e) => {
                    eprintln!("[{}] Failed to send prompt: {}", agent_name, e);
                }
            }
        })
        .detach();
    }
}

impl Focusable for ChatInputPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatInputPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .justify_end()
            .bg(cx.theme().background)
            .child(
                ChatInputBox::new("chat-input-box", self.input_state.clone())
                    .title("Send a message")
                    .context_list(self.context_list.clone(), cx)
                    .context_popover_open(self.context_popover_open)
                    .on_context_popover_change(cx.listener(|this, open: &bool, _, cx| {
                        this.context_popover_open = *open;
                        cx.notify();
                    }))
                    .on_send(cx.listener(|this, _, window, cx| {
                        this.send_message(window, cx);
                    }))
                    .mode_select(self.mode_select.clone())
                    .agent_select(self.agent_select.clone()),
            )
    }
}
