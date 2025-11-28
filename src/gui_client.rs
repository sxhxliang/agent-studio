use agent_client_protocol as acp;
use agent_client_protocol_schema as schema;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::{
    acp_client::PermissionStore,
    session_bus::{SessionUpdateBusContainer, SessionUpdateEvent},
};

/// Convert from agent_client_protocol SessionUpdate to agent_client_protocol_schema SessionUpdate
fn convert_session_update(update: &acp::SessionUpdate) -> schema::SessionUpdate {
    match update {
        acp::SessionUpdate::UserMessageChunk(chunk) => {
            schema::SessionUpdate::UserMessageChunk(convert_content_chunk(chunk))
        }
        acp::SessionUpdate::AgentMessageChunk(chunk) => {
            schema::SessionUpdate::AgentMessageChunk(convert_content_chunk(chunk))
        }
        acp::SessionUpdate::AgentThoughtChunk(chunk) => {
            schema::SessionUpdate::AgentThoughtChunk(convert_content_chunk(chunk))
        }
        acp::SessionUpdate::ToolCall(tool_call) => {
            schema::SessionUpdate::ToolCall(convert_tool_call(tool_call))
        }
        acp::SessionUpdate::ToolCallUpdate(update) => {
            schema::SessionUpdate::ToolCallUpdate(convert_tool_call_update(update))
        }
        acp::SessionUpdate::Plan(plan) => {
            schema::SessionUpdate::Plan(convert_plan(plan))
        }
        acp::SessionUpdate::CurrentModeUpdate(mode_update) => {
            schema::SessionUpdate::CurrentModeUpdate(schema::CurrentModeUpdate {
                current_mode_id: mode_update.current_mode_id.to_string(),
            })
        }
        acp::SessionUpdate::AvailableCommandsUpdate(commands_update) => {
            schema::SessionUpdate::AvailableCommandsUpdate(schema::AvailableCommandsUpdate {
                available_commands: commands_update
                    .available_commands
                    .iter()
                    .map(|cmd| schema::AvailableCommand {
                        name: cmd.name.to_string(),
                        description: cmd.description.clone().map(|d| d.to_string()),
                    })
                    .collect(),
            })
        }
    }
}

fn convert_content_chunk(chunk: &acp::ContentChunk) -> schema::ContentChunk {
    schema::ContentChunk {
        content: convert_content_block(&chunk.content),
    }
}

fn convert_content_block(block: &acp::ContentBlock) -> schema::ContentBlock {
    match block {
        acp::ContentBlock::Text(text) => {
            schema::ContentBlock::Text(schema::TextContent {
                text: text.text.to_string(),
            })
        }
        acp::ContentBlock::Image(image) => {
            schema::ContentBlock::Image(schema::ImageContent {
                data: image.data.to_string(),
                mime_type: image.mime_type.to_string(),
            })
        }
        acp::ContentBlock::Audio(audio) => {
            schema::ContentBlock::Audio(schema::AudioContent {
                data: audio.data.to_string(),
                mime_type: audio.mime_type.to_string(),
            })
        }
        acp::ContentBlock::ResourceLink(link) => {
            schema::ContentBlock::ResourceLink(schema::ResourceLink {
                uri: link.uri.to_string(),
                name: link.name.to_string(),
                mime_type: link.mime_type.clone().map(|m| m.to_string()),
            })
        }
        acp::ContentBlock::Resource(resource) => {
            schema::ContentBlock::Resource(schema::EmbeddedResource {
                resource: convert_embedded_resource_resource(&resource.resource),
            })
        }
    }
}

fn convert_embedded_resource_resource(
    resource: &acp::EmbeddedResourceResource,
) -> schema::EmbeddedResourceResource {
    match resource {
        acp::EmbeddedResourceResource::TextResourceContents(text_res) => {
            schema::EmbeddedResourceResource::TextResourceContents(schema::TextResourceContents {
                uri: text_res.uri.to_string(),
                mime_type: text_res.mime_type.clone().map(|m| m.to_string()),
                text: text_res.text.to_string(),
            })
        }
        acp::EmbeddedResourceResource::BlobResourceContents(blob_res) => {
            schema::EmbeddedResourceResource::BlobResourceContents(schema::BlobResourceContents {
                uri: blob_res.uri.to_string(),
                mime_type: blob_res.mime_type.clone().map(|m| m.to_string()),
                blob: blob_res.blob.to_string(),
            })
        }
    }
}

fn convert_tool_call(tool_call: &acp::ToolCall) -> schema::ToolCall {
    schema::ToolCall {
        tool_call_id: tool_call.id.to_string(),
        kind: convert_tool_kind(&tool_call.kind),
        title: tool_call.title.to_string(),
        status: convert_tool_call_status(&tool_call.status),
        content: tool_call
            .content
            .iter()
            .map(convert_tool_call_content)
            .collect(),
    }
}

fn convert_tool_kind(kind: &acp::ToolKind) -> schema::ToolKind {
    match kind {
        acp::ToolKind::Read => schema::ToolKind::Read,
        acp::ToolKind::Edit => schema::ToolKind::Edit,
        acp::ToolKind::Delete => schema::ToolKind::Delete,
        acp::ToolKind::Move => schema::ToolKind::Move,
        acp::ToolKind::Search => schema::ToolKind::Search,
        acp::ToolKind::Execute => schema::ToolKind::Execute,
        acp::ToolKind::Think => schema::ToolKind::Think,
        acp::ToolKind::Fetch => schema::ToolKind::Fetch,
        acp::ToolKind::SwitchMode => schema::ToolKind::SwitchMode,
        acp::ToolKind::Other => schema::ToolKind::Other,
    }
}

fn convert_tool_call_status(status: &acp::ToolCallStatus) -> schema::ToolCallStatus {
    match status {
        acp::ToolCallStatus::Pending => schema::ToolCallStatus::Pending,
        acp::ToolCallStatus::InProgress => schema::ToolCallStatus::InProgress,
        acp::ToolCallStatus::Completed => schema::ToolCallStatus::Completed,
        acp::ToolCallStatus::Failed => schema::ToolCallStatus::Failed,
    }
}

fn convert_tool_call_content(content: &acp::ToolCallContent) -> schema::ToolCallContent {
    match content {
        acp::ToolCallContent::Content(c) => {
            schema::ToolCallContent::Content(schema::Content {
                content: convert_content_block(&c.content),
            })
        }
        acp::ToolCallContent::Diff(diff) => {
            schema::ToolCallContent::Diff(schema::Diff {
                path: diff.path.clone(),
                old_text: diff.old_text.clone().map(|t| t.to_string()),
                new_text: diff.new_text.to_string(),
            })
        }
        acp::ToolCallContent::Terminal(terminal) => {
            schema::ToolCallContent::Terminal(schema::Terminal {
                terminal_id: terminal.terminal_id.to_string(),
            })
        }
    }
}

fn convert_tool_call_update(update: &acp::ToolCallUpdate) -> schema::ToolCallUpdate {
    schema::ToolCallUpdate {
        tool_call_id: update.id.to_string(),
        status: convert_tool_call_status(&update.status),
    }
}

fn convert_plan(plan: &acp::Plan) -> schema::Plan {
    schema::Plan {
        entries: plan
            .entries
            .iter()
            .map(|entry| schema::PlanEntry {
                id: entry.id.to_string(),
                content: entry.content.to_string(),
                status: convert_plan_entry_status(&entry.status),
                priority: convert_plan_entry_priority(&entry.priority),
            })
            .collect(),
    }
}

fn convert_plan_entry_status(status: &acp::PlanEntryStatus) -> schema::PlanEntryStatus {
    match status {
        acp::PlanEntryStatus::Pending => schema::PlanEntryStatus::Pending,
        acp::PlanEntryStatus::InProgress => schema::PlanEntryStatus::InProgress,
        acp::PlanEntryStatus::Completed => schema::PlanEntryStatus::Completed,
    }
}

fn convert_plan_entry_priority(priority: &acp::PlanEntryPriority) -> schema::PlanEntryPriority {
    match priority {
        acp::PlanEntryPriority::High => schema::PlanEntryPriority::High,
        acp::PlanEntryPriority::Normal => schema::PlanEntryPriority::Normal,
        acp::PlanEntryPriority::Low => schema::PlanEntryPriority::Low,
    }
}

/// GUI Client that publishes session updates to the event bus
pub struct GuiClient {
    agent_name: String,
    permission_store: Arc<PermissionStore>,
    session_bus: SessionUpdateBusContainer,
}

impl GuiClient {
    pub fn new(
        agent_name: String,
        permission_store: Arc<PermissionStore>,
        session_bus: SessionUpdateBusContainer,
    ) -> Self {
        Self {
            agent_name,
            permission_store,
            session_bus,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Client for GuiClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let id = self
            .permission_store
            .add(self.agent_name.clone(), args.session_id.to_string(), tx)
            .await;

        println!(
            "\n[PERMISSION REQUEST] Agent '{}' session '{}'",
            self.agent_name, args.session_id
        );

        if let Some(title) = &args.tool_call.fields.title {
            println!("  Action: {}", title);
        }
        if let Some(locations) = &args.tool_call.fields.locations {
            for loc in locations {
                println!("  Location: {:?}", loc.path);
            }
        }

        println!("Options:");
        for opt in &args.options {
            println!("  [{}] {}", opt.id.0, opt.name);
        }

        println!("To select an option, type: /decide {} <option_id>", id);

        rx.await.map_err(|_| {
            acp::Error::internal_error().with_data("permission request channel closed")
        })
    }

    async fn write_text_file(
        &self,
        _args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn read_text_file(
        &self,
        _args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn create_terminal(
        &self,
        _args: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse, acp::Error> {
        Err(acp::Error::method_not_found())
    }

    async fn terminal_output(
        &self,
        _args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn release_terminal(
        &self,
        _args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn wait_for_terminal_exit(
        &self,
        _args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn kill_terminal_command(
        &self,
        _args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<(), acp::Error> {
        // Publish event to the session bus
        let event = SessionUpdateEvent {
            session_id: args.session_id.to_string(),
            update: Arc::new(convert_session_update(&args.update)),
        };

        self.session_bus.publish(event);

        // Also print to console for debugging
        match &args.update {
            acp::SessionUpdate::UserMessageChunk(chunk) => {
                println!("\n[{}] User: {:?}", self.agent_name, extract_text(&chunk.content));
            }
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                println!("\n[{}] Agent: {:?}", self.agent_name, extract_text(&chunk.content));
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                println!("\n[{}] Thought: {:?}", self.agent_name, extract_text(&chunk.content));
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                println!("\n[{}] Tool: {}", self.agent_name, tool_call.title);
            }
            acp::SessionUpdate::Plan(plan) => {
                println!("\n[{}] Plan with {} entries", self.agent_name, plan.entries.len());
            }
            _ => {}
        }

        Ok(())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> acp::Result<()> {
        Ok(())
    }
}

fn extract_text(content: &acp::ContentBlock) -> String {
    match content {
        acp::ContentBlock::Text(text_content) => text_content.text.to_string(),
        acp::ContentBlock::Image(_) => "<image>".into(),
        acp::ContentBlock::Audio(_) => "<audio>".into(),
        acp::ContentBlock::ResourceLink(resource_link) => resource_link.uri.to_string(),
        acp::ContentBlock::Resource(_) => "<resource>".into(),
    }
}
