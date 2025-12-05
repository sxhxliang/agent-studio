use agent_client_protocol_schema::{ToolCallStatus, ToolKind};
use gpui::SharedString;
use gpui_component::IconName;

// ============================================================================
// Helper Traits
// ============================================================================

pub trait ToolKindExt {
    fn icon(&self) -> IconName;
}

impl ToolKindExt for ToolKind {
    fn icon(&self) -> IconName {
        match self {
            ToolKind::Read => IconName::File,
            ToolKind::Edit => IconName::Replace,
            ToolKind::Delete => IconName::Delete,
            ToolKind::Move => IconName::ArrowRight,
            ToolKind::Search => IconName::Search,
            ToolKind::Execute => IconName::SquareTerminal,
            ToolKind::Think => IconName::Bot,
            ToolKind::Fetch => IconName::Globe,
            ToolKind::SwitchMode => IconName::ArrowRight,
            ToolKind::Other | _ => IconName::Ellipsis,
        }
    }
}

pub trait ToolCallStatusExt {
    fn icon(&self) -> IconName;
}

impl ToolCallStatusExt for ToolCallStatus {
    fn icon(&self) -> IconName {
        match self {
            ToolCallStatus::Pending => IconName::Dash,
            ToolCallStatus::InProgress => IconName::LoaderCircle,
            ToolCallStatus::Completed => IconName::CircleCheck,
            ToolCallStatus::Failed => IconName::CircleX,
            _ => IconName::Dash,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

pub fn extract_filename(uri: &str) -> String {
    uri.split('/').next_back().unwrap_or("unknown").to_string()
}

pub fn get_file_icon(mime_type: &Option<String>) -> IconName {
    if let Some(ref mime) = mime_type {
        if mime.contains("python")
            || mime.contains("javascript")
            || mime.contains("typescript")
            || mime.contains("rust")
            || mime.contains("json")
        {
            return IconName::File;
        }
    }
    IconName::File
}

// ============================================================================
// Resource Info Structure
// ============================================================================

#[derive(Clone)]
pub struct ResourceInfo {
    pub uri: SharedString,
    pub name: SharedString,
    pub mime_type: Option<SharedString>,
    pub text: Option<SharedString>,
}

impl ResourceInfo {
    pub fn from_content_block(
        content: &agent_client_protocol_schema::ContentBlock,
    ) -> Option<Self> {
        match content {
            agent_client_protocol_schema::ContentBlock::ResourceLink(link) => Some(ResourceInfo {
                uri: link.uri.clone().into(),
                name: link.name.clone().into(),
                mime_type: link.mime_type.clone().map(Into::into),
                text: None,
            }),
            // TODO: Handle Resource type when schema is clarified
            // agent_client_protocol_schema::ContentBlock::Resource(res) => { ... }
            _ => None,
        }
    }
}
