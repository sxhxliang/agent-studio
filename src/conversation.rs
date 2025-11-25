use gpui::{
    px, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
    Pixels, Render, Styled, Window,
};

use gpui_component::{scroll::ScrollbarAxis, v_flex, ActiveTheme, StyledExt};

use crate::{
    AgentMessage, AgentMessageContent, AgentMessageData, AgentTodoList, MessageContent, PlanEntry,
    PlanEntryPriority, PlanEntryStatus, ResourceContent, ToolCallContent, ToolCallData,
    ToolCallItem, ToolCallKind, ToolCallStatus, UserMessage, UserMessageData,
};

pub struct ConversationPanel {
    focus_handle: FocusHandle,
}

impl super::DockPanel for ConversationPanel {
    fn title() -> &'static str {
        "Conversation"
    }

    fn description() -> &'static str {
        "A conversation view with agent messages, user messages, tool calls, and todos."
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        Self::view(window, cx)
    }

    fn paddings() -> Pixels {
        px(0.)
    }
}

impl ConversationPanel {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(_: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for ConversationPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ConversationPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Message 1: User asks a question
        let user_msg1 = UserMessage::new(
            "msg-1",
            UserMessageData::new("sess_001")
                .add_content(MessageContent::text(
                    "Can you help me refactor this authentication code? I think there might be some security issues."
                ))
                .add_content(MessageContent::resource(
                    ResourceContent::new(
                        "file:///src/auth.rs",
                        "text/rust",
                        "pub fn authenticate(username: &str, password: &str) -> bool {\n    let stored_password = get_password_from_db(username);\n    password == stored_password\n}"
                    )
                ))
        );

        // Message 2: Agent responds with a plan
        let agent_msg1 = AgentMessage::new(
            "msg-2",
            AgentMessageData::new("sess_001")
                .with_agent_name("Claude")
                .add_chunk(AgentMessageContent::text(
                    "I'll help you refactor the authentication code. I've identified several security issues that need to be addressed. Let me create a plan for the improvements."
                ))
                .complete()
        );

        // Message 3: Todo list for the refactoring
        let todo_list = AgentTodoList::new().title("Refactoring Plan").entries(vec![
            PlanEntry::new("Replace plain text password comparison with secure hashing")
                .with_priority(PlanEntryPriority::High)
                .with_status(PlanEntryStatus::Completed),
            PlanEntry::new("Add timing attack prevention")
                .with_priority(PlanEntryPriority::High)
                .with_status(PlanEntryStatus::Completed),
            PlanEntry::new("Implement rate limiting for authentication attempts")
                .with_priority(PlanEntryPriority::High)
                .with_status(PlanEntryStatus::InProgress),
            PlanEntry::new("Add comprehensive error handling")
                .with_priority(PlanEntryPriority::Medium)
                .with_status(PlanEntryStatus::Pending),
            PlanEntry::new("Write security tests")
                .with_priority(PlanEntryPriority::Medium)
                .with_status(PlanEntryStatus::Pending),
        ]);

        // Message 4: Tool call - reading files
        let tool_call1 = ToolCallItem::new(
            "tool-1",
            ToolCallData::new("call_001", "Reading auth.rs")
                .with_kind(ToolCallKind::Read)
                .with_status(ToolCallStatus::Completed)
                .with_content(vec![ToolCallContent::new(
                    "Successfully read auth.rs (45 lines)",
                )]),
        )
        .open(false);

        // Message 5: Tool call - searching for dependencies
        let tool_call2 = ToolCallItem::new(
            "tool-2",
            ToolCallData::new("call_002", "Searching for password hashing libraries")
                .with_kind(ToolCallKind::Search)
                .with_status(ToolCallStatus::Completed)
                .with_content(vec![
                    ToolCallContent::new("Found bcrypt, argon2, and sha2 crates.\nRecommending argon2 for modern password hashing.")
                ])
        ).open(false);

        // Message 6: Agent message about changes
        let agent_msg2 = AgentMessage::new(
            "msg-6",
            AgentMessageData::new("sess_001")
                .with_agent_name("Claude")
                .add_chunk(AgentMessageContent::text(
                    "Now I'll implement the secure password hashing. I'm using Argon2, which is the recommended algorithm for password hashing as of 2023."
                ))
                .complete()
        );

        // Message 7: Tool call - editing file
        let tool_call3 = ToolCallItem::new(
            "tool-3",
            ToolCallData::new("call_003", "Editing auth.rs with secure hashing")
                .with_kind(ToolCallKind::Edit)
                .with_status(ToolCallStatus::Completed)
                .with_content(vec![
                    ToolCallContent::new("Modified auth.rs:\n- Added argon2 dependency\n- Replaced plain text comparison with verify_password()\n- Added constant-time comparison\n\n+15 lines, -3 lines")
                ])
        ).open(true);

        // Message 8: User asks follow-up question
        let user_msg2 = UserMessage::new(
            "msg-8",
            UserMessageData::new("sess_001").add_content(MessageContent::text(
                "Great! Can you also add rate limiting? I'm worried about brute force attacks.",
            )),
        );

        // Message 9: Agent acknowledges
        let agent_msg3 = AgentMessage::new(
            "msg-9",
            AgentMessageData::new("sess_001")
                .with_agent_name("Claude")
                .add_chunk(AgentMessageContent::text(
                    "Absolutely! Rate limiting is crucial for preventing brute force attacks. I'll implement a rate limiter using a token bucket algorithm."
                ))
                .complete()
        );

        // Message 10: Tool call - creating new file
        let tool_call4 = ToolCallItem::new(
            "tool-4",
            ToolCallData::new("call_004", "Creating rate_limiter.rs")
                .with_kind(ToolCallKind::Edit)
                .with_status(ToolCallStatus::Completed)
                .with_content(vec![
                    ToolCallContent::new("Created rate_limiter.rs with TokenBucket implementation:\n- Max 5 attempts per 15 minutes\n- Exponential backoff\n- IP-based tracking\n\n+87 lines")
                ])
        ).open(false);

        // Message 11: Tool call - running tests
        let tool_call5 = ToolCallItem::new(
            "tool-5",
            ToolCallData::new("call_005", "Running security tests")
                .with_kind(ToolCallKind::Execute)
                .with_status(ToolCallStatus::InProgress)
                .with_content(vec![
                    ToolCallContent::new("Running test suite...\n✓ test_password_hashing ... ok\n✓ test_timing_attack_prevention ... ok\n→ test_rate_limiting ... running")
                ])
        ).open(true);

        // Message 12: Agent provides streaming response (incomplete)
        let agent_msg4 = AgentMessage::new(
            "msg-12",
            AgentMessageData::new("sess_001")
                .with_agent_name("Claude")
                .add_chunk(AgentMessageContent::text(
                    "The tests are running. While we wait, let me explain the security improvements:\n\n1. Password Hashing: We're using Argon2id with recommended parameters"
                ))
        );

        // Message 13: User shares another file
        let user_msg3 = UserMessage::new(
            "msg-13",
            UserMessageData::new("sess_001")
                .add_content(MessageContent::text(
                    "While you're at it, can you also review this session management code?"
                ))
                .add_content(MessageContent::resource(
                    ResourceContent::new(
                        "file:///src/session.rs",
                        "text/rust",
                        "pub struct Session {\n    user_id: String,\n    token: String,\n    created_at: i64,\n}\n\nimpl Session {\n    pub fn new(user_id: String) -> Self {\n        Self {\n            user_id,\n            token: generate_token(),\n            created_at: now(),\n        }\n    }\n}"
                    )
                ))
        );

        // Message 14: Todo list updated
        let todo_list2 = AgentTodoList::new()
            .title("Security Audit Progress")
            .entries(vec![
                PlanEntry::new("Audit authentication code")
                    .with_priority(PlanEntryPriority::High)
                    .with_status(PlanEntryStatus::Completed),
                PlanEntry::new("Implement secure password hashing")
                    .with_priority(PlanEntryPriority::High)
                    .with_status(PlanEntryStatus::Completed),
                PlanEntry::new("Add rate limiting")
                    .with_priority(PlanEntryPriority::High)
                    .with_status(PlanEntryStatus::Completed),
                PlanEntry::new("Review session management")
                    .with_priority(PlanEntryPriority::High)
                    .with_status(PlanEntryStatus::InProgress),
                PlanEntry::new("Add session expiration")
                    .with_priority(PlanEntryPriority::Medium)
                    .with_status(PlanEntryStatus::Pending),
                PlanEntry::new("Implement CSRF protection")
                    .with_priority(PlanEntryPriority::Medium)
                    .with_status(PlanEntryStatus::Pending),
            ]);

        // Message 15: Tool call - analyzing session code
        let tool_call6 = ToolCallItem::new(
            "tool-6",
            ToolCallData::new("call_006", "Analyzing session.rs for vulnerabilities")
                .with_kind(ToolCallKind::Think)
                .with_status(ToolCallStatus::Completed)
                .with_content(vec![
                    ToolCallContent::new("Analysis complete:\n\nIssues found:\n1. No session expiration\n2. Token generation method not specified\n3. No secure storage mechanism\n4. Missing CSRF protection\n\nRecommendations:\n- Add expiration timestamp\n- Use cryptographically secure random tokens\n- Store sessions in secure backend\n- Implement CSRF tokens")
                ])
        ).open(true);

        v_flex()
            .p_4()
            .gap_6()
            .bg(cx.theme().background)
            // Add all messages
            .child(user_msg1)
            .child(agent_msg1)
            .child(v_flex().pl_6().child(todo_list))
            .child(v_flex().pl_6().gap_2().child(tool_call1).child(tool_call2))
            .child(agent_msg2)
            .child(v_flex().pl_6().child(tool_call3))
            .child(user_msg2)
            .child(agent_msg3)
            .child(v_flex().pl_6().gap_2().child(tool_call4).child(tool_call5))
            .child(agent_msg4)
            .child(user_msg3)
            .child(v_flex().pl_6().child(todo_list2))
            .child(v_flex().pl_6().child(tool_call6))
            .scrollable(ScrollbarAxis::Vertical)
            .size_full()
    }
}
