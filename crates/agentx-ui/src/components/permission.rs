use gpui::*;
use gpui_component::{
    Icon, IconName, Sizable as _, Theme,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};

use agentx_domain::{PermissionOptionKind, PermissionRequest};

pub fn permission_option_kind_to_icon(kind: PermissionOptionKind) -> IconName {
    match kind {
        PermissionOptionKind::AllowOnce => IconName::Check,
        PermissionOptionKind::AllowAlways => IconName::CircleCheck,
        PermissionOptionKind::RejectOnce => IconName::Minus,
        PermissionOptionKind::RejectAlways => IconName::CircleX,
    }
}

pub fn permission_is_allow(kind: PermissionOptionKind) -> bool {
    matches!(
        kind,
        PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
    )
}

pub fn render_permission_request_card(
    request: &PermissionRequest,
    option_buttons: Vec<AnyElement>,
    deny_button: AnyElement,
    theme: &Theme,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_3()
        .p_3()
        .rounded(px(8.))
        .border_1()
        .border_color(theme.accent)
        .bg(theme.background)
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(16.))
                        .text_color(theme.accent),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child("Permission Request"),
                ),
        )
        .child(
            v_flex()
                .gap_1()
                .pl_6()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(format!("Tool: {}", request.tool_call.title)),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .child(format!("Kind: {:?}", request.tool_call.kind)),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .pl_6()
                .children(option_buttons)
                .child(deny_button),
        )
        .into_any_element()
}

pub fn permission_option_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: PermissionOptionKind,
) -> Button {
    let button = Button::new(id)
        .label(label)
        .icon(permission_option_kind_to_icon(kind))
        .small();

    if permission_is_allow(kind) {
        button.primary()
    } else {
        button.ghost()
    }
}
