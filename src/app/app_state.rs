use gpui::{App, AppContext, Entity, Global, SharedString};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{
    core::agent::{AgentManager, PermissionStore},
    core::config::DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES,
    core::event_bus::EventHub,
    core::services::{
        AgentConfigService, AgentService, AiService, MessageService, PersistenceService,
        WorkspaceService,
    },
};

use super::service_registry::ServiceRegistry;

/// Welcome session info - stores the session created when user selects an agent
#[derive(Clone, Debug)]
pub struct WelcomeSession {
    pub session_id: String,
    pub agent_name: String,
}

pub struct AppState {
    // UI state (GPUI entities)
    pub invisible_panels: Entity<Vec<SharedString>>,
    pub selected_tool_call: Entity<Option<agent_client_protocol::ToolCall>>,

    // Infrastructure
    agent_manager: Option<Arc<AgentManager>>,
    permission_store: Option<Arc<PermissionStore>>,

    /// Service registry — Clone + Send, can be captured in async closures
    pub services: ServiceRegistry,

    // Configuration
    config_path: Option<PathBuf>,
    current_working_dir: PathBuf,
    tool_call_preview_max_lines: usize,

    // Temporary UI state
    welcome_session: Option<WelcomeSession>,
    app_title: SharedString,
}

impl AppState {
    pub fn init(cx: &mut App) {
        // Initialize WorkspaceService with config path
        let config_path = crate::core::config_manager::get_workspace_config_path();

        // Create shared event hub
        let event_hub = EventHub::new();

        // Create service registry
        let mut services = ServiceRegistry::new(event_hub.clone());

        // Create workspace service and set its bus
        let mut workspace_service = WorkspaceService::new(config_path);
        workspace_service.set_event_hub(event_hub.clone());
        services.set_workspace_service(Arc::new(workspace_service));

        let sessions_dir = crate::core::config_manager::get_sessions_dir();
        services.set_persistence_service(Arc::new(PersistenceService::new(sessions_dir)));

        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
            agent_manager: None,
            permission_store: None,
            services,
            welcome_session: None,
            config_path: None,
            current_working_dir: Self::resolve_initial_working_dir(),
            tool_call_preview_max_lines: DEFAULT_TOOL_CALL_PREVIEW_MAX_LINES,
            selected_tool_call: cx.new(|_| None),
            app_title: SharedString::from(""),
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    fn resolve_initial_working_dir() -> PathBuf {
        if let Ok(cwd) = std::env::current_dir() {
            if Self::is_safe_working_dir(&cwd) {
                return cwd;
            }
        }

        let data_dir = crate::core::config_manager::user_data_dir_or_temp();
        if Self::is_safe_working_dir(&data_dir) {
            return data_dir;
        }

        if let Some(home) = dirs::home_dir() {
            if Self::is_safe_working_dir(&home) {
                return home;
            }
        }

        data_dir
    }

    fn is_safe_working_dir(path: &Path) -> bool {
        if !path.is_dir() {
            return false;
        }

        if path.parent().is_none() {
            return false;
        }

        #[cfg(target_os = "macos")]
        {
            let path_str = path.to_string_lossy();
            if path_str.contains(".app/Contents") {
                return false;
            }
        }

        true
    }

    /// Set the AgentManager after async initialization
    pub fn set_agent_manager(
        &mut self,
        manager: Arc<AgentManager>,
        initial_config: crate::core::config::Config,
    ) {
        log::info!("Setting AgentManager");

        // Ensure persistence service exists
        let sessions_dir = crate::core::config_manager::get_sessions_dir();
        let persistence_service = match self.services.persistence_service() {
            Ok(ps) => ps.clone(),
            Err(_) => {
                let ps = Arc::new(PersistenceService::new(sessions_dir));
                self.services.set_persistence_service(ps.clone());
                ps
            }
        };

        let event_hub = self.services.event_hub.clone();

        // Initialize services when agent_manager is set
        let mut agent_service = AgentService::new(manager.clone());
        agent_service.set_event_hub(event_hub.clone());
        let agent_service = Arc::new(agent_service);

        let message_service = Arc::new(MessageService::new(
            event_hub.clone(),
            agent_service.clone(),
            persistence_service,
        ));

        // Initialize AgentConfigService if config_path is set
        if let Some(config_path) = &self.config_path {
            let mut service = AgentConfigService::new(
                initial_config.clone(),
                config_path.clone(),
                manager.clone(),
                event_hub.clone(),
            );
            service.set_agent_service(agent_service.clone());
            self.services.set_agent_config_service(Arc::new(service));
        } else {
            log::warn!("Config path not set, AgentConfigService will not be initialized");
        }

        // Initialize AI Service from config
        if !initial_config.models.is_empty() {
            log::info!(
                "Initializing AI Service with {} models",
                initial_config.models.len()
            );
            self.services.set_ai_service(Arc::new(AiService::new(
                initial_config.models.clone(),
                initial_config.system_prompts.clone(),
            )));
        } else {
            log::warn!("No AI models configured in config.json");
        }

        self.agent_manager = Some(manager);
        self.services.set_agent_service(agent_service);
        self.services.set_message_service(message_service);
        self.tool_call_preview_max_lines = initial_config.tool_call_preview_max_lines;

        log::info!(
            "Initialized service layer (AgentService, MessageService, PersistenceService, AgentConfigService, AiService)"
        );
    }

    /// Set the config path for AgentConfigService
    pub fn set_config_path(&mut self, path: PathBuf) {
        self.config_path = Some(path);
    }

    /// Set the PermissionStore
    pub fn set_permission_store(&mut self, store: Arc<PermissionStore>) {
        log::info!("Setting PermissionStore");
        self.permission_store = Some(store);
    }

    pub fn set_app_title(&mut self, title: SharedString) {
        self.app_title = title;
    }

    pub fn app_title(&self) -> &SharedString {
        &self.app_title
    }

    /// Get a reference to the AgentManager if initialized
    pub fn agent_manager(&self) -> Option<&Arc<AgentManager>> {
        self.agent_manager.as_ref()
    }

    /// Get the PermissionStore if set
    pub fn permission_store(&self) -> Option<&Arc<PermissionStore>> {
        self.permission_store.as_ref()
    }

    /// Get the event hub
    pub fn event_hub(&self) -> &EventHub {
        &self.services.event_hub
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

    // --- Backward-compatible service accessors (delegate to ServiceRegistry) ---

    pub fn agent_service(&self) -> Option<&Arc<AgentService>> {
        self.services.agent_service().ok()
    }

    pub fn message_service(&self) -> Option<&Arc<MessageService>> {
        self.services.message_service().ok()
    }

    pub fn persistence_service(&self) -> Option<&Arc<PersistenceService>> {
        self.services.persistence_service().ok()
    }

    pub fn workspace_service(&self) -> Option<&Arc<WorkspaceService>> {
        self.services.workspace_service().ok()
    }

    pub fn agent_config_service(&self) -> Option<&Arc<AgentConfigService>> {
        self.services.agent_config_service().ok()
    }

    pub fn ai_service(&self) -> Option<&Arc<AiService>> {
        self.services.ai_service().ok()
    }

    /// Get the current working directory
    pub fn current_working_dir(&self) -> &PathBuf {
        &self.current_working_dir
    }

    /// Set the current working directory
    pub fn set_current_working_dir(&mut self, path: PathBuf) {
        log::info!("Setting current working directory: {:?}", path);
        self.current_working_dir = path;
    }

    /// Get the tool call preview line limit
    pub fn tool_call_preview_max_lines(&self) -> usize {
        self.tool_call_preview_max_lines
    }
}
impl Global for AppState {}
