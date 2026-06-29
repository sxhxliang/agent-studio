//! UI-only conversion between the domain timeline and the ACP widgets.
//!
//! The application core persists and publishes domain events. The legacy
//! conversation UI, now factored into `agentx-acp-ui`, renders ACP-shaped
//! updates. These helpers keep that adaptation local to the driving adapter.

use agent_client_protocol::schema as acp;

use agentx_domain::{
    ContentBlock, PermissionOptionKind, PermissionOutcome, PermissionRequest, Plan,
    PlanEntryStatus, PlanPriority, ResourceContents, SessionEvent, SlashCommand, ToolCall,
    ToolCallContent, ToolCallStatus, ToolKind,
};

pub(super) fn session_event_to_updates(event: &SessionEvent) -> Vec<acp::SessionUpdate> {
    match event {
        SessionEvent::UserMessage { content } => content
            .iter()
            .map(|block| {
                acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(content_block_to_acp(
                    block,
                )))
            })
            .collect(),
        SessionEvent::AgentMessage { content } => content
            .iter()
            .map(|block| {
                acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(content_block_to_acp(
                    block,
                )))
            })
            .collect(),
        SessionEvent::AgentThought { text } => {
            vec![acp::SessionUpdate::AgentThoughtChunk(
                acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(
                    text.clone(),
                ))),
            )]
        }
        SessionEvent::ToolCall(call) => vec![acp::SessionUpdate::ToolCall(tool_call_to_acp(call))],
        SessionEvent::Plan(plan) => vec![acp::SessionUpdate::Plan(plan_to_acp(plan))],
        SessionEvent::Stopped { .. } => Vec::new(),
    }
}

pub(super) fn commands_to_update(commands: &[SlashCommand]) -> acp::SessionUpdate {
    acp::SessionUpdate::AvailableCommandsUpdate(acp::AvailableCommandsUpdate::new(
        commands
            .iter()
            .map(|command| {
                acp::AvailableCommand::new(command.name.clone(), command.description.clone())
            })
            .collect(),
    ))
}

pub(super) fn permission_request_to_acp(
    request: &PermissionRequest,
) -> (acp::ToolCallUpdate, Vec<acp::PermissionOption>) {
    let fields = acp::ToolCallUpdateFields::new()
        .title(request.tool_call.title.clone())
        .kind(tool_kind_to_acp(request.tool_call.kind))
        .status(tool_status_to_acp(request.tool_call.status))
        .content(
            request
                .tool_call
                .content
                .iter()
                .map(tool_content_to_acp)
                .collect::<Vec<_>>(),
        );
    let tool_call = acp::ToolCallUpdate::new(request.tool_call.id.clone(), fields);
    let options = request
        .options
        .iter()
        .map(|option| {
            acp::PermissionOption::new(
                option.id.clone(),
                option.label.clone(),
                permission_kind_to_acp(option.kind),
            )
        })
        .collect();
    (tool_call, options)
}

pub(super) fn permission_response_to_outcome(
    response: acp::RequestPermissionResponse,
) -> PermissionOutcome {
    match response.outcome {
        acp::RequestPermissionOutcome::Selected(selected) => PermissionOutcome::Selected {
            option_id: selected.option_id.to_string(),
        },
        acp::RequestPermissionOutcome::Cancelled => PermissionOutcome::Cancelled,
        _ => PermissionOutcome::Cancelled,
    }
}

pub(super) fn acp_tool_call_to_domain(call: acp::ToolCall) -> ToolCall {
    ToolCall {
        id: call.tool_call_id.to_string(),
        title: call.title,
        kind: tool_kind_to_domain(call.kind),
        status: tool_status_to_domain(call.status),
        content: call
            .content
            .into_iter()
            .filter_map(tool_content_to_domain)
            .collect(),
    }
}

fn content_block_to_acp(block: &ContentBlock) -> acp::ContentBlock {
    match block {
        ContentBlock::Text(text) => acp::ContentBlock::Text(acp::TextContent::new(text.clone())),
        ContentBlock::Image { mime_type, data } => {
            acp::ContentBlock::Image(acp::ImageContent::new(data.clone(), mime_type.clone()))
        }
        ContentBlock::ResourceLink {
            name,
            uri,
            mime_type,
        } => acp::ContentBlock::ResourceLink(
            acp::ResourceLink::new(name.clone(), uri.clone()).mime_type(mime_type.clone()),
        ),
        ContentBlock::Resource(ResourceContents::Text {
            uri,
            text,
            mime_type,
        }) => acp::ContentBlock::Resource(acp::EmbeddedResource::new(
            acp::EmbeddedResourceResource::TextResourceContents(
                acp::TextResourceContents::new(text.clone(), uri.clone())
                    .mime_type(mime_type.clone()),
            ),
        )),
        ContentBlock::Resource(ResourceContents::Blob {
            uri,
            blob,
            mime_type,
        }) => acp::ContentBlock::Resource(acp::EmbeddedResource::new(
            acp::EmbeddedResourceResource::BlobResourceContents(
                acp::BlobResourceContents::new(blob.clone(), uri.clone())
                    .mime_type(mime_type.clone()),
            ),
        )),
    }
}

fn tool_call_to_acp(call: &ToolCall) -> acp::ToolCall {
    acp::ToolCall::new(call.id.clone(), call.title.clone())
        .kind(tool_kind_to_acp(call.kind))
        .status(tool_status_to_acp(call.status))
        .content(call.content.iter().map(tool_content_to_acp).collect())
}

fn tool_content_to_acp(content: &ToolCallContent) -> acp::ToolCallContent {
    match content {
        ToolCallContent::Text(text) => {
            acp::ToolCallContent::Content(acp::Content::new(text.clone()))
        }
        ToolCallContent::Diff {
            path,
            old_text,
            new_text,
        } => {
            let diff = acp::Diff::new(path.clone(), new_text.clone()).old_text(old_text.clone());
            acp::ToolCallContent::Diff(diff)
        }
    }
}

fn tool_content_to_domain(content: acp::ToolCallContent) -> Option<ToolCallContent> {
    match content {
        acp::ToolCallContent::Content(content) => match content.content {
            acp::ContentBlock::Text(text) => Some(ToolCallContent::Text(text.text)),
            _ => None,
        },
        acp::ToolCallContent::Diff(diff) => Some(ToolCallContent::Diff {
            path: diff.path.to_string_lossy().to_string(),
            old_text: diff.old_text,
            new_text: diff.new_text,
        }),
        _ => None,
    }
}

fn plan_to_acp(plan: &Plan) -> acp::Plan {
    acp::Plan::new(
        plan.entries
            .iter()
            .map(|entry| {
                acp::PlanEntry::new(
                    entry.content.clone(),
                    plan_priority_to_acp(entry.priority),
                    plan_status_to_acp(entry.status),
                )
            })
            .collect(),
    )
}

fn plan_priority_to_acp(priority: PlanPriority) -> acp::PlanEntryPriority {
    match priority {
        PlanPriority::Low => acp::PlanEntryPriority::Low,
        PlanPriority::Medium => acp::PlanEntryPriority::Medium,
        PlanPriority::High => acp::PlanEntryPriority::High,
    }
}

fn plan_status_to_acp(status: PlanEntryStatus) -> acp::PlanEntryStatus {
    match status {
        PlanEntryStatus::Pending => acp::PlanEntryStatus::Pending,
        PlanEntryStatus::InProgress => acp::PlanEntryStatus::InProgress,
        PlanEntryStatus::Completed => acp::PlanEntryStatus::Completed,
    }
}

fn tool_kind_to_acp(kind: ToolKind) -> acp::ToolKind {
    match kind {
        ToolKind::Read => acp::ToolKind::Read,
        ToolKind::Edit => acp::ToolKind::Edit,
        ToolKind::Delete => acp::ToolKind::Delete,
        ToolKind::Move => acp::ToolKind::Move,
        ToolKind::Search => acp::ToolKind::Search,
        ToolKind::Execute => acp::ToolKind::Execute,
        ToolKind::Think => acp::ToolKind::Think,
        ToolKind::Fetch => acp::ToolKind::Fetch,
        ToolKind::Other => acp::ToolKind::Other,
    }
}

fn tool_kind_to_domain(kind: acp::ToolKind) -> ToolKind {
    match kind {
        acp::ToolKind::Read => ToolKind::Read,
        acp::ToolKind::Edit => ToolKind::Edit,
        acp::ToolKind::Delete => ToolKind::Delete,
        acp::ToolKind::Move => ToolKind::Move,
        acp::ToolKind::Search => ToolKind::Search,
        acp::ToolKind::Execute => ToolKind::Execute,
        acp::ToolKind::Think => ToolKind::Think,
        acp::ToolKind::Fetch => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

fn tool_status_to_acp(status: ToolCallStatus) -> acp::ToolCallStatus {
    match status {
        ToolCallStatus::Pending => acp::ToolCallStatus::Pending,
        ToolCallStatus::InProgress => acp::ToolCallStatus::InProgress,
        ToolCallStatus::Completed => acp::ToolCallStatus::Completed,
        ToolCallStatus::Failed => acp::ToolCallStatus::Failed,
    }
}

fn tool_status_to_domain(status: acp::ToolCallStatus) -> ToolCallStatus {
    match status {
        acp::ToolCallStatus::Pending => ToolCallStatus::Pending,
        acp::ToolCallStatus::InProgress => ToolCallStatus::InProgress,
        acp::ToolCallStatus::Completed => ToolCallStatus::Completed,
        acp::ToolCallStatus::Failed => ToolCallStatus::Failed,
        _ => ToolCallStatus::Pending,
    }
}

fn permission_kind_to_acp(kind: PermissionOptionKind) -> acp::PermissionOptionKind {
    match kind {
        PermissionOptionKind::AllowOnce => acp::PermissionOptionKind::AllowOnce,
        PermissionOptionKind::AllowAlways => acp::PermissionOptionKind::AllowAlways,
        PermissionOptionKind::RejectOnce => acp::PermissionOptionKind::RejectOnce,
        PermissionOptionKind::RejectAlways => acp::PermissionOptionKind::RejectAlways,
    }
}
