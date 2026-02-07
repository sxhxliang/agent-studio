use std::sync::Arc;

use agent_client_protocol::{
    ContentBlock, Diff, PermissionOption, PermissionOptionKind, Plan, PlanEntry, PlanEntryPriority,
    PlanEntryStatus, SessionId, ToolCall, ToolCallContent, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind,
};
use agentx_acp_ui::{
    AgentMessageData, AgentMessageOptions, AgentMessageView, AgentTodoListView, DiffSummary,
    DiffSummaryData, DiffSummaryOptions, DiffView, PermissionRequest, PermissionRequestOptions,
    ToolCallItem, ToolCallItemOptions, UserMessageData, UserMessageView,
};
use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    group_box::{GroupBox, GroupBoxVariants},
    scroll::ScrollableElement as _,
    v_flex,
};

macro_rules! section {
    ($title:expr) => {
        section($title)
    };
}

fn section(title: impl Into<SharedString>) -> GroupBox {
    GroupBox::new()
        .outline()
        .title(div().text_size(px(12.)).child(title.into()))
}

pub struct AcpUiStory {
    agent_message: Entity<AgentMessageView>,
    user_message: Entity<UserMessageView>,
    todo_list: Entity<AgentTodoListView>,
    tool_call_item: Entity<ToolCallItem>,
    diff_summary: Entity<DiffSummary>,
    permission_request: Entity<PermissionRequest>,
    diff_for_view: Diff,
}

impl AcpUiStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let session_id = SessionId::from("story-session");

        let agent_message_data = AgentMessageData::new(session_id.clone())
            .with_agent_name("AgentX")
            .add_text("Here is a **markdown** response with a list:")
            .add_text("\n- First item\n- Second item")
            .complete();

        let agent_message = AgentMessageView::with_options(
            agent_message_data,
            AgentMessageOptions::default(),
            window,
            cx,
        );

        let user_message_data = UserMessageData::new(session_id.clone())
            .add_text("Please update the config and summarize changes.")
            .add_resource_link("config.json", "file:///workspace/config.json")
            .add_resource_link("notes.txt", "file:///workspace/notes.txt");

        let user_message = UserMessageView::new(user_message_data, window, cx);

        let plan_meta = serde_json::json!({"title": "Execution Plan"});
        let mut plan = Plan::new(vec![
            PlanEntry::new(
                "Parse configuration",
                PlanEntryPriority::High,
                PlanEntryStatus::Completed,
            ),
            PlanEntry::new(
                "Apply updates",
                PlanEntryPriority::Medium,
                PlanEntryStatus::InProgress,
            ),
            PlanEntry::new(
                "Verify output",
                PlanEntryPriority::Low,
                PlanEntryStatus::Pending,
            ),
        ]);
        plan.meta = plan_meta.as_object().cloned();

        let todo_list = AgentTodoListView::with_plan(plan, window, cx);

        let diff_main = Diff::new(
            "src/main.rs",
            "fn main() {\n    println!(\"Hello, AgentX\");\n}".to_string(),
        )
        .old_text("fn main() {\n    println!(\"Hello\");\n}".to_string());

        let terminal = agent_client_protocol::Terminal::new("term-1").meta(
            serde_json::json!({"output": "cargo check\nFinished"})
                .as_object()
                .cloned(),
        );

        let mut tool_call = ToolCall::new("tool-call-1", "Edit src/main.rs");
        tool_call.kind = ToolKind::Edit;
        tool_call.status = ToolCallStatus::Completed;
        tool_call.content = vec![
            ToolCallContent::Diff(diff_main.clone()),
            ToolCallContent::from(ContentBlock::from("Applied formatting.")),
            ToolCallContent::Terminal(terminal),
        ];

        let tool_call_options = ToolCallItemOptions::default()
            .preview_max_lines(6)
            .on_open_detail(Arc::new(|tool_call, _window, _cx| {
                log::info!("Open tool call detail: {}", tool_call.tool_call_id);
            }));

        let tool_call_item = cx.new(|_| ToolCallItem::with_options(tool_call, tool_call_options));

        let diff_other = Diff::new("README.md", "# AgentX\nUpdated".to_string())
            .old_text("# AgentX".to_string());

        let mut tool_call_summary = ToolCall::new("tool-call-2", "Update README");
        tool_call_summary.kind = ToolKind::Edit;
        tool_call_summary.status = ToolCallStatus::Completed;
        tool_call_summary.content = vec![ToolCallContent::Diff(diff_other.clone())];

        let summary_data = DiffSummaryData::from_tool_calls(&[tool_call_summary]);
        let diff_summary = cx.new(|_| {
            DiffSummary::new(summary_data).with_options(DiffSummaryOptions {
                on_open_tool_call: Some(Arc::new(|tool_call, _window, _cx| {
                    log::info!("Open diff summary tool call: {}", tool_call.tool_call_id);
                })),
            })
        });

        let tool_call_update = ToolCallUpdate::new(
            "tool-call-3",
            ToolCallUpdateFields::new()
                .title("Delete generated files".to_string())
                .kind(ToolKind::Delete),
        );

        let permission_options = vec![
            PermissionOption::new("allow-once", "Allow once", PermissionOptionKind::AllowOnce),
            PermissionOption::new(
                "reject-once",
                "Reject once",
                PermissionOptionKind::RejectOnce,
            ),
        ];

        let permission_request = cx.new(|_| {
            PermissionRequest::with_options(
                "permission-1".to_string(),
                session_id.to_string(),
                &tool_call_update,
                permission_options,
                PermissionRequestOptions {
                    on_response: Some(Arc::new(|permission_id, response, _cx| {
                        log::info!("Permission {} responded with {:?}", permission_id, response);
                    })),
                },
            )
        });

        cx.new(|_| Self {
            agent_message,
            user_message,
            todo_list,
            tool_call_item,
            diff_summary,
            permission_request,
            diff_for_view: diff_main,
        })
    }
}

impl Render for AcpUiStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div().size_full().overflow_y_scrollbar().p_6().child(
                v_flex()
                    .gap_6()
                    .child(section!("Agent Message").child(self.agent_message.clone()))
                    .child(section!("User Message").child(self.user_message.clone()))
                    .child(section!("Plan / Todo List").child(self.todo_list.clone()))
                    .child(section!("Tool Call Item").child(self.tool_call_item.clone()))
                    .child(section!("Diff Summary").child(self.diff_summary.clone()))
                    .child(section!("Permission Request").child(self.permission_request.clone()))
                    .child(
                        section!("Diff View").child(
                            DiffView::new(self.diff_for_view.clone())
                                .context_lines(2)
                                .max_lines(120)
                                .render(window, cx),
                        ),
                    ),
            ),
        )
    }
}
