use std::sync::Arc;

use agent_client_protocol::{self as acp, PermissionOption, PermissionOptionKind};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

pub type PermissionResponseHandler = Arc<
    dyn Fn(String, acp::RequestPermissionResponse, &mut Context<PermissionRequest>) + Send + Sync,
>;

#[derive(Clone, Default)]
pub struct PermissionRequestOptions {
    pub on_response: Option<PermissionResponseHandler>,
}

pub fn permission_option_kind_to_icon(kind: PermissionOptionKind) -> IconName {
    match kind {
        PermissionOptionKind::AllowOnce => IconName::Check,
        PermissionOptionKind::AllowAlways => IconName::CircleCheck,
        PermissionOptionKind::RejectOnce => IconName::Minus,
        PermissionOptionKind::RejectAlways => IconName::CircleX,
        _ => IconName::CircleX,
    }
}

pub fn permission_is_allow(kind: PermissionOptionKind) -> bool {
    matches!(
        kind,
        PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
    )
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
    options: Vec<PermissionOption>,
    /// Whether the request has been responded to
    responded: bool,
    /// Response handler
    request_options: PermissionRequestOptions,
}

impl PermissionRequest {
    pub fn new(
        permission_id: String,
        session_id: String,
        tool_call: &acp::ToolCallUpdate,
        options: Vec<acp::PermissionOption>,
    ) -> Self {
        Self::with_options(
            permission_id,
            session_id,
            tool_call,
            options,
            PermissionRequestOptions::default(),
        )
    }

    pub fn with_options(
        permission_id: String,
        session_id: String,
        tool_call: &acp::ToolCallUpdate,
        options: Vec<acp::PermissionOption>,
        request_options: PermissionRequestOptions,
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
            options: options.into_iter().collect(),
            responded: false,
            request_options,
        }
    }

    pub fn on_response(mut self, handler: PermissionResponseHandler) -> Self {
        self.request_options.on_response = Some(handler);
        self
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

        let response = acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id)),
        );

        if let Some(handler) = self.request_options.on_response.as_ref() {
            handler(self.permission_id.clone(), response, cx);
        } else {
            log::warn!("PermissionRequest has no response handler");
        }

        // Mark as responded and trigger re-render
        self.responded = true;
        cx.notify();
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
                            let is_allow = permission_is_allow(option.kind);
                            Button::new(SharedString::from(format!(
                                "permission-{}-{}",
                                self.permission_id, option.option_id
                            )))
                            .label(option.name.clone())
                            .icon(permission_option_kind_to_icon(option.kind))
                            .when(is_allow, |btn| btn.primary())
                            .when(!is_allow, |btn| btn.ghost())
                            .small()
                            .on_click(cx.listener(
                                move |this, _ev, window, cx| {
                                    this.on_option_selected(option_id.to_string(), window, cx);
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
        tool_call: &acp::ToolCallUpdate,
        options: Vec<acp::PermissionOption>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let item =
                cx.new(|_| PermissionRequest::new(permission_id, session_id, tool_call, options));
            Self { item }
        })
    }

    pub fn with_options(
        permission_id: String,
        session_id: String,
        tool_call: &acp::ToolCallUpdate,
        options: Vec<acp::PermissionOption>,
        request_options: PermissionRequestOptions,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let item = cx.new(|_| {
                PermissionRequest::with_options(
                    permission_id,
                    session_id,
                    tool_call,
                    options,
                    request_options,
                )
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_allow_checks() {
        assert!(permission_is_allow(PermissionOptionKind::AllowOnce));
        assert!(permission_is_allow(PermissionOptionKind::AllowAlways));
        assert!(!permission_is_allow(PermissionOptionKind::RejectOnce));
    }
}
