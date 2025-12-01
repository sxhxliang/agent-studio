use gpui::{
    div, prelude::FluentBuilder as _, px, App, AppContext, Context, Entity, IntoElement,
    ParentElement, Render, SharedString, Styled, Window,
};

use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex, v_flex, ActiveTheme, Icon, IconName, Sizable,
};

use crate::AppState;

/// Permission option kind determines the button style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

impl PermissionOptionKind {
    pub fn icon(&self) -> IconName {
        match self {
            PermissionOptionKind::AllowOnce => IconName::Check,
            PermissionOptionKind::AllowAlways => IconName::CircleCheck,
            PermissionOptionKind::RejectOnce => IconName::Minus,
            PermissionOptionKind::RejectAlways => IconName::CircleX,
        }
    }

    pub fn is_allow(&self) -> bool {
        matches!(
            self,
            PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
        )
    }
}

impl From<&agent_client_protocol::PermissionOptionKind> for PermissionOptionKind {
    fn from(kind: &agent_client_protocol::PermissionOptionKind) -> Self {
        match kind {
            agent_client_protocol::PermissionOptionKind::AllowOnce => {
                PermissionOptionKind::AllowOnce
            }
            agent_client_protocol::PermissionOptionKind::AllowAlways => {
                PermissionOptionKind::AllowAlways
            }
            agent_client_protocol::PermissionOptionKind::RejectOnce => {
                PermissionOptionKind::RejectOnce
            }
            agent_client_protocol::PermissionOptionKind::RejectAlways => {
                PermissionOptionKind::RejectAlways
            }
        }
    }
}

/// Permission option for display
#[derive(Clone)]
pub struct PermissionOptionData {
    pub option_id: String,
    pub name: String,
    pub kind: PermissionOptionKind,
}

impl From<agent_client_protocol::PermissionOption> for PermissionOptionData {
    fn from(option: agent_client_protocol::PermissionOption) -> Self {
        Self {
            option_id: option.id.to_string(),
            name: option.name,
            kind: PermissionOptionKind::from(&option.kind),
        }
    }
}

/// Permission request component - displays a tool call permission request with options
pub struct PermissionRequest {
    /// Unique permission ID
    permission_id: String,
    /// Session ID
    session_id: String,
    /// Tool call title
    tool_title: String,
    /// Tool call kind
    tool_kind: Option<String>,
    /// Available permission options
    options: Vec<PermissionOptionData>,
    /// Whether the request has been responded to
    responded: bool,
}

impl PermissionRequest {
    pub fn new(
        permission_id: String,
        session_id: String,
        tool_call: &agent_client_protocol::ToolCallUpdate,
        options: Vec<agent_client_protocol::PermissionOption>,
    ) -> Self {
        let tool_title = tool_call
            .fields
            .title
            .clone()
            .unwrap_or_else(|| "Tool Call".to_string());
        let tool_kind = tool_call.fields.kind.as_ref().map(|k| format!("{:?}", k));

        Self {
            permission_id,
            session_id,
            tool_title,
            tool_kind,
            options: options.into_iter().map(Into::into).collect(),
            responded: false,
        }
    }

    /// Handle user selection of a permission option
    fn on_option_selected(
        &mut self,
        option_id: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.responded {
            return; // Already responded
        }

        log::info!(
            "Permission option selected: {} for permission_id: {}",
            option_id,
            self.permission_id
        );

        // Get the PermissionStore from AppState
        let permission_store = AppState::global(cx).permission_store().cloned();

        if let Some(store) = permission_store {
            let permission_id = self.permission_id.clone();
            let response = agent_client_protocol::RequestPermissionResponse {
                outcome: agent_client_protocol::RequestPermissionOutcome::Selected {
                    option_id: option_id.clone().into(),
                },
                meta: None,
            };

            // Spawn a task to send the response
            cx.spawn(async move |_entity, _cx| {
                // Call respond on the permission store
                if let Err(e) = store.respond(&permission_id, response).await {
                    log::error!("Failed to send permission response: {}", e);
                } else {
                    log::info!("Permission response sent successfully");
                }
            })
            .detach();

            // Mark as responded and trigger re-render
            self.responded = true;
            cx.notify();
        } else {
            log::error!("PermissionStore not available in AppState");
        }
    }
}

impl Render for PermissionRequest {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let responded = self.responded;

        v_flex()
            .w_full()
            .gap_3()
            .p_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if responded {
                cx.theme().border
            } else {
                cx.theme().accent
            })
            .bg(if responded {
                cx.theme().muted
            } else {
                cx.theme().background
            })
            .child(
                // Header with icon and title
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(IconName::TriangleAlert).size(px(16.)).text_color(
                        if responded {
                            cx.theme().muted_foreground
                        } else {
                            cx.theme().accent
                        },
                    ))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(if responded {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().foreground
                            })
                            .child(if responded {
                                "Permission Request (Responded)"
                            } else {
                                "Permission Request"
                            }),
                    ),
            )
            .child(
                // Tool call information
                v_flex()
                    .gap_1()
                    .pl_6()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Tool: {}", self.tool_title)),
                    )
                    .when_some(self.tool_kind.as_ref(), |this, kind| {
                        this.child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("Kind: {}", kind)),
                        )
                    }),
            )
            .when(!responded, |this| {
                // Options - only show if not responded
                this.child(
                    h_flex()
                        .gap_2()
                        .pl_6()
                        .children(self.options.iter().map(|option| {
                            let option_id = option.option_id.clone();
                            let is_allow = option.kind.is_allow();

                            Button::new(SharedString::from(format!(
                                "permission-{}-{}",
                                self.permission_id, option.option_id
                            )))
                            .label(option.name.clone())
                            .icon(option.kind.icon())
                            .when(is_allow, |btn| btn.primary())
                            .when(!is_allow, |btn| btn.ghost())
                            .small()
                            .on_click(cx.listener(
                                move |this, _ev, window, cx| {
                                    this.on_option_selected(option_id.clone(), window, cx);
                                },
                            ))
                        })),
                )
            })
    }
}

/// View wrapper for PermissionRequest
pub struct PermissionRequestView {
    pub(crate) item: Entity<PermissionRequest>,
}

impl PermissionRequestView {
    pub fn new(
        permission_id: String,
        session_id: String,
        tool_call: &agent_client_protocol::ToolCallUpdate,
        options: Vec<agent_client_protocol::PermissionOption>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let item =
                cx.new(|_| PermissionRequest::new(permission_id, session_id, tool_call, options));
            Self { item }
        })
    }

    /// Create view directly from a PermissionRequest entity (for internal use)
    pub(crate) fn from_entity(item: Entity<PermissionRequest>) -> Self {
        Self { item }
    }

    pub fn permission_id(&self, cx: &App) -> String {
        self.item.read(cx).permission_id.clone()
    }
}

impl Render for PermissionRequestView {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.item.clone()
    }
}
