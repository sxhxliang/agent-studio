use std::sync::Arc;

use gpui::{App, AppContext, Entity, Global, SharedString};

use crate::{
    acp_client::{AgentManager, PermissionStore},
    permission_bus::PermissionBusContainer,
    session_bus::SessionUpdateBusContainer,
};

/// Welcome session info - stores the session created when user selects an agent
#[derive(Clone, Debug)]
pub struct WelcomeSession {
    pub session_id: String,
    pub agent_name: String,
}

pub struct AppState {
    pub invisible_panels: Entity<Vec<SharedString>>,
    agent_manager: Option<Arc<AgentManager>>,
    permission_store: Option<Arc<PermissionStore>>,
    pub session_bus: SessionUpdateBusContainer,
    pub permission_bus: PermissionBusContainer,
    /// Current welcome session - created when user selects an agent
    welcome_session: Option<WelcomeSession>,
}

impl AppState {
    pub fn init(cx: &mut App) {
        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
            agent_manager: None,
            permission_store: None,
            session_bus: SessionUpdateBusContainer::new(),
            permission_bus: PermissionBusContainer::new(),
            welcome_session: None,
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    /// Set the AgentManager after async initialization
    pub fn set_agent_manager(&mut self, manager: Arc<AgentManager>) {
        log::info!(
            "Setting AgentManager with {} agents",
            manager.list_agents().len()
        );
        self.agent_manager = Some(manager);
    }

    /// Set the PermissionStore
    pub fn set_permission_store(&mut self, store: Arc<PermissionStore>) {
        log::info!("Setting PermissionStore");
        self.permission_store = Some(store);
    }

    /// Get a reference to the AgentManager if initialized
    pub fn agent_manager(&self) -> Option<&Arc<AgentManager>> {
        self.agent_manager.as_ref()
    }

    /// Get the PermissionStore if set
    pub fn permission_store(&self) -> Option<&Arc<PermissionStore>> {
        self.permission_store.as_ref()
    }

    /// Set the welcome session
    pub fn set_welcome_session(&mut self, session: WelcomeSession) {
        log::info!(
            "Setting welcome session: session_id={}, agent={}",
            session.session_id,
            session.agent_name
        );
        self.welcome_session = Some(session);
    }

    /// Get the welcome session
    pub fn welcome_session(&self) -> Option<&WelcomeSession> {
        self.welcome_session.as_ref()
    }

    /// Clear the welcome session
    pub fn clear_welcome_session(&mut self) {
        log::info!("Clearing welcome session");
        self.welcome_session = None;
    }
}
impl Global for AppState {}
