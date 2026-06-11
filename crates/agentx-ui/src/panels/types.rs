use std::path::PathBuf;

use agentx_domain::{SessionId, ToolCall, WorkspaceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelPlacement {
    Center,
    Left,
    Right,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Chat { session: Option<SessionId> },
    Sessions,
    Welcome { workspace: Option<WorkspaceId> },
    Tasks,
    Settings,
    Terminal { cwd: Option<PathBuf> },
    CodeEditor { cwd: Option<PathBuf> },
    ToolCallDetail { tool_call: ToolCall },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelCommand {
    Add {
        panel: PanelKind,
        placement: PanelPlacement,
    },
    Show(PanelKind),
    Close {
        panel_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelDescriptor {
    pub id: String,
    pub kind: PanelKind,
    pub title: String,
    pub placement: PanelPlacement,
    pub closable: bool,
}

impl PanelDescriptor {
    pub fn new(
        id: impl Into<String>,
        kind: PanelKind,
        title: impl Into<String>,
        placement: PanelPlacement,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            title: title.into(),
            placement,
            closable: true,
        }
    }
}
