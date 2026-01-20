use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Pixels,
    Render, Styled, Window, prelude::FluentBuilder, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::{
    AppState,
    core::services::{AgentSessionInfo, SessionStatus},
    panels::dock_panel::DockPanel,
};

/// Session Manager Panel - Displays and manages all agent sessions
pub struct SessionManagerPanel {
    focus_handle: FocusHandle,
    sessions_by_agent: Vec<(String, Vec<AgentSessionInfo>)>,
}

impl DockPanel for SessionManagerPanel {
    fn title() -> &'static str {
        "Session Manager"
    }

    fn title_key() -> Option<&'static str> {
        Some("session_manager.title")
    }

    fn description() -> &'static str {
        "Manage all agent sessions"
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn paddings() -> Pixels {
        px(12.)
    }
}

impl SessionManagerPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            sessions_by_agent: Vec::new(),
        };

        // Load initial session data
        panel.refresh_sessions(cx);

        panel
    }

    /// Refresh sessions from AgentService
    fn refresh_sessions(&mut self, cx: &mut Context<Self>) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::warn!("[SessionManagerPanel] AgentService not initialized");
                return;
            }
        };

        let weak_self = cx.entity().downgrade();
        cx.spawn(async move |_entity, cx| {
            // Get all agents
            let agents = agent_service.list_agents().await;

            // Group sessions by agent
            let mut sessions_by_agent = Vec::new();
            for agent_name in agents {
                let sessions = agent_service.list_sessions_for_agent(&agent_name);
                if !sessions.is_empty() {
                    sessions_by_agent.push((agent_name, sessions));
                }
            }

            _ = cx.update(|cx| {
                if let Some(this) = weak_self.upgrade() {
                    this.update(cx, |this, cx| {
                        this.sessions_by_agent = sessions_by_agent;
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    /// Create a new session for the given agent
    fn create_new_session(
        &mut self,
        agent_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("[SessionManagerPanel] AgentService not initialized");
                return;
            }
        };

        let agent_config_service = AppState::global(cx).agent_config_service().cloned();

        let weak_self = cx.entity().downgrade();
        cx.spawn_in(window, async move |_this, window| {
            let mcp_servers = if let Some(service) = agent_config_service {
                service
                    .list_mcp_servers()
                    .await
                    .into_iter()
                    .filter(|(_, config)| config.enabled)
                    .map(|(name, config)| config.to_acp_mcp_server(name))
                    .collect()
            } else {
                Vec::new()
            };
            match agent_service
                .create_session_with_mcp(&agent_name, mcp_servers)
                .await
            {
                Ok(session_id) => {
                    log::info!(
                        "[SessionManagerPanel] Created session {} for agent {}",
                        session_id,
                        agent_name
                    );
                    _ = window.update(|_window, cx| {
                        if let Some(entity) = weak_self.upgrade() {
                            entity.update(cx, |this, cx| {
                                this.refresh_sessions(cx);
                            });
                        }
                    });
                }
                Err(e) => {
                    log::error!("[SessionManagerPanel] Failed to create session: {}", e);
                }
            }
        })
        .detach();
    }

    /// Close a session
    fn close_session(
        &mut self,
        agent_name: String,
        session_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent_service = match AppState::global(cx).agent_service() {
            Some(service) => service.clone(),
            None => {
                log::error!("[SessionManagerPanel] AgentService not initialized");
                return;
            }
        };

        let weak_self = cx.entity().downgrade();
        cx.spawn_in(window, async move |_this, window| {
            match agent_service.close_session(&agent_name, &session_id).await {
                Ok(_) => {
                    log::info!(
                        "[SessionManagerPanel] Closed session {} for agent {}",
                        session_id,
                        agent_name
                    );
                    _ = window.update(|_window, cx| {
                        if let Some(entity) = weak_self.upgrade() {
                            entity.update(cx, |this, cx| {
                                this.refresh_sessions(cx);
                            });
                        }
                    });
                }
                Err(e) => {
                    log::error!("[SessionManagerPanel] Failed to close session: {}", e);
                }
            }
        })
        .detach();
    }

    /// Open a conversation panel for the given session
    fn open_session(&self, session_id: String, window: &mut Window, cx: &mut Context<Self>) {
        // Dispatch PanelAction to open the conversation panel
        window.dispatch_action(
            Box::new(crate::PanelAction::add_conversation_for_session(
                session_id,
                gpui_component::dock::DockPlacement::Center,
            )),
            cx,
        );
    }

    /// Get status badge color
    fn status_color(&self, status: &SessionStatus, cx: &App) -> gpui::Hsla {
        let theme = cx.theme();
        match status {
            SessionStatus::Active => theme.success,
            SessionStatus::Completed => theme.success,
            SessionStatus::Idle => theme.warning,
            SessionStatus::Closed => theme.muted,
            SessionStatus::Failed => theme.muted,
            SessionStatus::InProgress => theme.info,
            SessionStatus::Pending => theme.info,
        }
    }

    /// Get status text
    fn status_text(status: &SessionStatus) -> &'static str {
        match status {
            SessionStatus::Active => "Active",
            SessionStatus::Idle => "Idle",
            SessionStatus::Closed => "Closed",
            SessionStatus::InProgress => "InProgress",
            SessionStatus::Completed => "Completed",
            SessionStatus::Failed => "Failed",
            SessionStatus::Pending => "Pending",
        }
    }
}

impl Focusable for SessionManagerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SessionManagerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .gap_4()
            .bg(theme.background)
            .child(
                // Header with refresh button
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        gpui::div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child("Sessions"),
                    )
                    .child(
                        Button::new("refresh")
                            .icon(Icon::new(IconName::LoaderCircle))
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.refresh_sessions(cx);
                            })),
                    ),
            )
            .child(
                // Scrollable session list
                gpui::div().flex_1().child(
                        v_flex()
                            .w_full()
                            .gap_4()
                            .children(self.sessions_by_agent.iter().enumerate().map(|(agent_idx, (agent_name, sessions))| {
                                let agent_name_clone = agent_name.clone();

                                v_flex()
                                    .w_full()
                                    .gap_2()
                                    .p_3()
                                    .rounded(px(8.))
                                    .bg(theme.secondary)
                                    .border_1()
                                    .border_color(theme.border)
                                    .child(
                                        // Agent header with new session button
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                gpui::div()
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(theme.foreground)
                                                    .child(format!("{} ({} sessions)", agent_name, sessions.len())),
                                            )
                                            .child(
                                                Button::new(("new-session", agent_idx))
                                                    .label("New")
                                                    .icon(Icon::new(IconName::Plus))
                                                    .ghost()
                                                    .small()
                                                    .on_click({
                                                        let agent_name = agent_name_clone.clone();
                                                        cx.listener(move |this, _, window, cx| {
                                                            this.create_new_session(agent_name.clone(), window, cx);
                                                        })
                                                    }),
                                            ),
                                    )
                                    .child(
                                        // Session list
                                        v_flex()
                                            .w_full()
                                            .gap_2()
                                            .children(sessions.iter().enumerate().map(|(session_idx, session)| {
                                                let session_id = session.session_id.clone();
                                                let agent_name_for_close = agent_name_clone.clone();
                                                let session_id_for_close = session_id.clone();
                                                let session_id_for_open = session_id.clone();
                                                let status_color = self.status_color(&session.status, cx);
                                                let short_id = if session_id.len() > 12 {
                                                    &session_id[..12]
                                                } else {
                                                    &session_id
                                                };
                                                // Create unique button ID from indices
                                                let btn_id = agent_idx * 1000 + session_idx;

                                                h_flex()
                                                    .w_full()
                                                    .items_center()
                                                    .justify_between()
                                                    .p_2()
                                                    .rounded(px(6.))
                                                    .bg(theme.background)
                                                    .border_1()
                                                    .border_color(theme.border.opacity(0.5))
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .items_center()
                                                            .child(
                                                                // Status indicator
                                                                gpui::div()
                                                                    .w(px(8.))
                                                                    .h(px(8.))
                                                                    .rounded(px(4.))
                                                                    .bg(status_color),
                                                            )
                                                            .child(
                                                                v_flex()
                                                                    .gap_1()
                                                                    .child(
                                                                        gpui::div()
                                                                            .text_xs()
                                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                                            .text_color(theme.foreground)
                                                                            .child(format!("Session {}", short_id)),
                                                                    )
                                                                    .child(
                                                                        gpui::div()
                                                                            .text_xs()
                                                                            .text_color(theme.muted_foreground)
                                                                            .child(format!("{} | Last active: {}",
                                                                                Self::status_text(&session.status),
                                                                                session.last_active.format("%H:%M:%S")
                                                                            )),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .gap_1()
                                                            .child(
                                                                Button::new(("open", btn_id))
                                                                    .label("Open")
                                                                    .ghost()
                                                                    .small()
                                                                    .on_click(cx.listener(move |this, _, window, cx| {
                                                                        this.open_session(session_id_for_open.clone(), window, cx);
                                                                    })),
                                                            )
                                                            .when(session.status != SessionStatus::Closed, |this| {
                                                                this.child(
                                                                    Button::new(("close", btn_id))
                                                                        .label("Close")
                                                                        .ghost()
                                                                        .small()
                                                                        .on_click({
                                                                            let agent_name = agent_name_for_close.clone();
                                                                            let session_id = session_id_for_close.clone();
                                                                            cx.listener(move |this, _, window, cx| {
                                                                                this.close_session(agent_name.clone(), session_id.clone(), window, cx);
                                                                            })
                                                                        }),
                                                                )
                                                            }),
                                                    )
                                            })),
                                    )
                            })),
                    ),
            )
    }
}
