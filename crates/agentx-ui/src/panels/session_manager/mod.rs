//! The session manager — live sessions grouped by agent, in the left dock.
//!
//! A container over [`SessionService`] (session lifecycle) and [`AgentRegistry`]
//! (the configured agents), plus the [`ChatView`] it opens sessions into. Like
//! the chat panel, `mod.rs` holds state + intents and [`view`] does the render.
//!
//! Note: the rewrite binds a window to one agent, and persisted timelines do not
//! record their agent, so this groups the *live* sessions of this run; the flat
//! on-disk history stays in the sibling `SessionsPanel`.

mod view;

use std::sync::Arc;

use gpui::*;

use agentx_app::SessionService;
use agentx_domain::{AgentId, AgentRegistry, AgentStatus, SessionId, SessionStatus};

use crate::chat::ChatView;

/// One session row within an agent group.
pub(crate) struct SessionRowVm {
    pub id: SessionId,
    pub status: SessionStatus,
}

/// An agent and its live sessions.
pub(crate) struct AgentGroupVm {
    pub agent: AgentId,
    pub status: AgentStatus,
    pub sessions: Vec<SessionRowVm>,
}

pub struct SessionManagerPanel {
    service: Arc<SessionService>,
    registry: Arc<dyn AgentRegistry>,
    chat: Entity<ChatView>,
    groups: Vec<AgentGroupVm>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl SessionManagerPanel {
    pub fn new(
        service: Arc<SessionService>,
        registry: Arc<dyn AgentRegistry>,
        chat: Entity<ChatView>,
        cx: &mut Context<Self>,
    ) -> Self {
        // The chat is the source of truth for session activity; re-pull the live
        // set whenever it changes (new turn, status change, new session).
        let subscriptions = vec![cx.observe(&chat, |this, _, cx| {
            this.refresh(cx);
            cx.notify();
        })];
        let panel = Self {
            service,
            registry,
            chat,
            groups: Vec::new(),
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        panel.refresh(cx);
        panel
    }

    /// Reload the live-session set and regroup it by agent.
    fn refresh(&self, cx: &mut Context<Self>) {
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let live = service.live_sessions().await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.rebuild_groups(live);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn rebuild_groups(&mut self, live: Vec<(AgentId, SessionId, SessionStatus)>) {
        // One group per configured agent, then drop each live session into its
        // agent's group (creating a group if the agent isn't configured).
        let mut groups: Vec<AgentGroupVm> = self
            .registry
            .agents()
            .into_iter()
            .map(|descriptor| AgentGroupVm {
                agent: descriptor.id,
                status: descriptor.status,
                sessions: Vec::new(),
            })
            .collect();
        for (agent, id, status) in live {
            if let Some(group) = groups.iter_mut().find(|group| group.agent == agent) {
                group.sessions.push(SessionRowVm { id, status });
            } else {
                groups.push(AgentGroupVm {
                    agent,
                    status: AgentStatus::Ready,
                    sessions: vec![SessionRowVm { id, status }],
                });
            }
        }
        self.groups = groups;
    }

    /// Browse a session in the chat (read-only history / live).
    fn open(&mut self, id: SessionId, cx: &mut Context<Self>) {
        self.chat.update(cx, |chat, cx| chat.view_session(id, cx));
    }

    /// Start a new session (for the chat's bound agent) and refresh.
    fn start_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat
            .update(cx, |chat, cx| chat.start_new_session(window, cx));
        self.refresh(cx);
    }

    /// Close a session (terminal; keeps its persisted timeline) and refresh.
    fn close(&mut self, id: SessionId, cx: &mut Context<Self>) {
        let service = self.service.clone();
        cx.spawn(async move |this, cx| {
            let _ = service.close_session(&id).await;
            let _ = cx.update(|cx| {
                if let Some(view) = this.upgrade() {
                    view.update(cx, |this, cx| {
                        this.refresh(cx);
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }
}
