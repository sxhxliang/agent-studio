//! The anti-corruption layer: pure translation between ACP schema types and
//! domain types.
//!
//! This is the *only* place ACP types are converted to or from the domain.
//! Everything inward of it works in domain terms, so ACP schema changes are
//! absorbed here instead of rippling through the application.
//!
//! Naming convention:
//! - `*_to_domain` — inbound: ACP notifications and results become domain types.
//! - `*_to_acp` — outbound: domain content becomes the ACP prompt payload.
//!
//! Inbound conversions return `Option` where the domain deliberately models
//! fewer cases than ACP (e.g. audio content, terminal tool output): a `None`
//! means "the domain does not represent this", so callers `filter_map` it away.
//! Matches on ACP enums always carry a catch-all arm because those enums are
//! `#[non_exhaustive]`.

use agent_client_protocol::schema as acp;
use agentx_domain::{
    ContentBlock, McpServerConfig, PermissionOutcome, Plan, PlanEntry, PlanEntryStatus,
    PlanPriority, ResourceContents, SessionId, SessionInit, SessionMode, SessionModel, StopReason,
    ToolCall, ToolCallContent, ToolCallStatus, ToolKind,
};

// ---------------------------------------------------------------------------
// Content blocks
// ---------------------------------------------------------------------------

/// Inbound: an ACP content block to its domain mirror, or `None` for content
/// the domain does not model (audio, and any future ACP-only kinds).
pub(crate) fn content_block_to_domain(block: acp::ContentBlock) -> Option<ContentBlock> {
    match block {
        acp::ContentBlock::Text(text) => Some(ContentBlock::Text(text.text)),
        acp::ContentBlock::Image(image) => Some(ContentBlock::Image {
            mime_type: image.mime_type,
            data: image.data,
        }),
        acp::ContentBlock::ResourceLink(link) => Some(ContentBlock::ResourceLink {
            name: link.name,
            uri: link.uri,
            mime_type: link.mime_type,
        }),
        acp::ContentBlock::Resource(resource) => {
            let contents = match resource.resource {
                acp::EmbeddedResourceResource::TextResourceContents(text) => {
                    ResourceContents::Text {
                        uri: text.uri,
                        text: text.text,
                        mime_type: text.mime_type,
                    }
                }
                acp::EmbeddedResourceResource::BlobResourceContents(blob) => {
                    ResourceContents::Blob {
                        uri: blob.uri,
                        blob: blob.blob,
                        mime_type: blob.mime_type,
                    }
                }
                _ => return None,
            };
            Some(ContentBlock::Resource(contents))
        }
        _ => None,
    }
}

/// Outbound: a domain content block to the ACP block sent in a prompt.
///
/// Total (no `Option`): every domain block has an ACP form. ACP structs are
/// `#[non_exhaustive]`, so they are built with their constructors, not literals.
pub(crate) fn content_block_to_acp(block: ContentBlock) -> acp::ContentBlock {
    match block {
        ContentBlock::Text(text) => acp::ContentBlock::Text(acp::TextContent::new(text)),
        ContentBlock::Image { mime_type, data } => {
            acp::ContentBlock::Image(acp::ImageContent::new(data, mime_type))
        }
        ContentBlock::ResourceLink {
            name,
            uri,
            mime_type,
        } => acp::ContentBlock::ResourceLink(acp::ResourceLink::new(name, uri).mime_type(mime_type)),
        ContentBlock::Resource(ResourceContents::Text {
            uri,
            text,
            mime_type,
        }) => acp::ContentBlock::Resource(acp::EmbeddedResource::new(
            acp::EmbeddedResourceResource::TextResourceContents(
                acp::TextResourceContents::new(text, uri).mime_type(mime_type),
            ),
        )),
        ContentBlock::Resource(ResourceContents::Blob {
            uri,
            blob,
            mime_type,
        }) => acp::ContentBlock::Resource(acp::EmbeddedResource::new(
            acp::EmbeddedResourceResource::BlobResourceContents(
                acp::BlobResourceContents::new(blob, uri).mime_type(mime_type),
            ),
        )),
    }
}

/// Outbound: a whole prompt body.
pub(crate) fn content_blocks_to_acp(blocks: Vec<ContentBlock>) -> Vec<acp::ContentBlock> {
    blocks.into_iter().map(content_block_to_acp).collect()
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

/// Inbound: a complete ACP tool call to its domain snapshot.
pub(crate) fn tool_call_to_domain(call: acp::ToolCall) -> ToolCall {
    ToolCall {
        id: call.tool_call_id.to_string(),
        title: call.title,
        kind: tool_kind_to_domain(call.kind),
        status: tool_call_status_to_domain(call.status),
        content: call
            .content
            .into_iter()
            .filter_map(tool_call_content_to_domain)
            .collect(),
    }
}

pub(crate) fn tool_kind_to_domain(kind: acp::ToolKind) -> ToolKind {
    match kind {
        acp::ToolKind::Read => ToolKind::Read,
        acp::ToolKind::Edit => ToolKind::Edit,
        acp::ToolKind::Delete => ToolKind::Delete,
        acp::ToolKind::Move => ToolKind::Move,
        acp::ToolKind::Search => ToolKind::Search,
        acp::ToolKind::Execute => ToolKind::Execute,
        acp::ToolKind::Think => ToolKind::Think,
        acp::ToolKind::Fetch => ToolKind::Fetch,
        // `SwitchMode` is a UI affordance the domain does not categorize.
        _ => ToolKind::Other,
    }
}

pub(crate) fn tool_call_status_to_domain(status: acp::ToolCallStatus) -> ToolCallStatus {
    match status {
        acp::ToolCallStatus::Pending => ToolCallStatus::Pending,
        acp::ToolCallStatus::InProgress => ToolCallStatus::InProgress,
        acp::ToolCallStatus::Completed => ToolCallStatus::Completed,
        acp::ToolCallStatus::Failed => ToolCallStatus::Failed,
        _ => ToolCallStatus::Pending,
    }
}

/// Inbound: a single piece of tool-call content. The domain models text and
/// diffs; embedded terminals and non-text blocks have no domain form.
pub(crate) fn tool_call_content_to_domain(
    content: acp::ToolCallContent,
) -> Option<ToolCallContent> {
    match content {
        acp::ToolCallContent::Content(block) => content_block_to_domain(block.content)
            .and_then(|block| block.as_plain_text().map(|text| text.to_string()))
            .map(ToolCallContent::Text),
        acp::ToolCallContent::Diff(diff) => Some(ToolCallContent::Diff {
            path: diff.path.to_string_lossy().into_owned(),
            old_text: diff.old_text,
            new_text: diff.new_text,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Plans
// ---------------------------------------------------------------------------

/// Inbound: an ACP plan to the domain plan. ACP sends the full entry list on
/// every update, so this is a plain element-wise map with no merging.
pub(crate) fn plan_to_domain(plan: acp::Plan) -> Plan {
    Plan {
        entries: plan.entries.into_iter().map(plan_entry_to_domain).collect(),
    }
}

fn plan_entry_to_domain(entry: acp::PlanEntry) -> PlanEntry {
    PlanEntry {
        content: entry.content,
        priority: plan_priority_to_domain(entry.priority),
        status: plan_entry_status_to_domain(entry.status),
    }
}

fn plan_priority_to_domain(priority: acp::PlanEntryPriority) -> PlanPriority {
    match priority {
        acp::PlanEntryPriority::High => PlanPriority::High,
        acp::PlanEntryPriority::Medium => PlanPriority::Medium,
        acp::PlanEntryPriority::Low => PlanPriority::Low,
        _ => PlanPriority::Medium,
    }
}

fn plan_entry_status_to_domain(status: acp::PlanEntryStatus) -> PlanEntryStatus {
    match status {
        acp::PlanEntryStatus::Pending => PlanEntryStatus::Pending,
        acp::PlanEntryStatus::InProgress => PlanEntryStatus::InProgress,
        acp::PlanEntryStatus::Completed => PlanEntryStatus::Completed,
        _ => PlanEntryStatus::Pending,
    }
}

// ---------------------------------------------------------------------------
// Stop reason
// ---------------------------------------------------------------------------

pub(crate) fn stop_reason_to_domain(reason: acp::StopReason) -> StopReason {
    match reason {
        acp::StopReason::EndTurn => StopReason::EndTurn,
        acp::StopReason::MaxTokens => StopReason::MaxTokens,
        // ACP calls this `MaxTurnRequests`; the domain calls it `MaxTurns`.
        acp::StopReason::MaxTurnRequests => StopReason::MaxTurns,
        acp::StopReason::Refusal => StopReason::Refusal,
        acp::StopReason::Cancelled => StopReason::Cancelled,
        _ => StopReason::EndTurn,
    }
}

// ---------------------------------------------------------------------------
// Permission outcome
// ---------------------------------------------------------------------------

/// Outbound: the user's domain decision becomes the ACP permission response the
/// agent is waiting on. `Cancelled` is also the outcome ACP mandates when a turn
/// is cancelled with pending permission requests.
pub(crate) fn permission_outcome_to_acp(
    outcome: PermissionOutcome,
) -> acp::RequestPermissionResponse {
    let outcome = match outcome {
        PermissionOutcome::Selected { option_id } => {
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id))
        }
        PermissionOutcome::Cancelled => acp::RequestPermissionOutcome::Cancelled,
    };
    acp::RequestPermissionResponse::new(outcome)
}

// ---------------------------------------------------------------------------
// MCP servers
// ---------------------------------------------------------------------------

/// Outbound: the enabled MCP servers to advertise when opening a session.
/// Disabled entries are dropped; each enabled one becomes a stdio server.
pub(crate) fn mcp_servers_to_acp(servers: &[McpServerConfig]) -> Vec<acp::McpServer> {
    servers
        .iter()
        .filter(|server| server.enabled)
        .map(mcp_server_to_acp)
        .collect()
}

fn mcp_server_to_acp(server: &McpServerConfig) -> acp::McpServer {
    let env = server
        .env
        .iter()
        .map(|(name, value)| acp::EnvVariable::new(name.clone(), value.clone()))
        .collect();
    acp::McpServer::Stdio(
        acp::McpServerStdio::new(server.name.clone(), server.command.clone())
            .args(server.args.clone())
            .env(env),
    )
}

// ---------------------------------------------------------------------------
// Session capabilities (modes, models)
// ---------------------------------------------------------------------------

/// Inbound: assemble the [`SessionInit`] returned from an ACP session response.
/// Modes come from the agent's mode state; models from the `Model`-category
/// config option. Slash commands are deliberately empty — ACP delivers those
/// later via a notification, a stream the domain does not model yet.
pub(crate) fn session_init_from(
    session_id: SessionId,
    modes: Option<acp::SessionModeState>,
    config_options: Option<Vec<acp::SessionConfigOption>>,
) -> SessionInit {
    let (modes, current_mode) = match modes {
        Some(state) => (
            state
                .available_modes
                .into_iter()
                .map(|mode| SessionMode {
                    id: mode.id.to_string(),
                    name: mode.name,
                })
                .collect(),
            Some(state.current_mode_id.to_string()),
        ),
        None => (Vec::new(), None),
    };
    let (models, current_model) = models_from(config_options.as_deref());
    SessionInit {
        session_id,
        modes,
        current_mode,
        models,
        current_model,
        commands: Vec::new(),
    }
}

/// The id of the session's model selector, if it advertises one — the supervisor
/// keeps it so a later `set_model` can target the right config option.
pub(crate) fn model_config_id(config_options: Option<&[acp::SessionConfigOption]>) -> Option<String> {
    Some(model_select(config_options?)?.0.id.to_string())
}

/// Find the `Model`-category select option and its select payload.
fn model_select(
    options: &[acp::SessionConfigOption],
) -> Option<(&acp::SessionConfigOption, &acp::SessionConfigSelect)> {
    options.iter().find_map(|option| {
        if option.category != Some(acp::SessionConfigOptionCategory::Model) {
            return None;
        }
        match &option.kind {
            acp::SessionConfigKind::Select(select) => Some((option, select)),
            _ => None,
        }
    })
}

fn models_from(
    config_options: Option<&[acp::SessionConfigOption]>,
) -> (Vec<SessionModel>, Option<String>) {
    let Some((_, select)) = config_options.and_then(model_select) else {
        return (Vec::new(), None);
    };
    let models = match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(list) => {
            list.iter().map(model_from_option).collect()
        }
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .map(model_from_option)
            .collect(),
        _ => Vec::new(),
    };
    (models, Some(select.current_value.to_string()))
}

fn model_from_option(option: &acp::SessionConfigSelectOption) -> SessionModel {
    SessionModel {
        id: option.value.to_string(),
        name: option.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- content: domain -> acp -> domain round-trips the modeled fields ----

    #[test]
    fn text_block_round_trips() {
        let domain = ContentBlock::text("hello");
        assert_eq!(
            content_block_to_domain(content_block_to_acp(domain.clone())),
            Some(domain)
        );
    }

    #[test]
    fn image_block_round_trips() {
        let domain = ContentBlock::Image {
            mime_type: "image/png".into(),
            data: "base64".into(),
        };
        assert_eq!(
            content_block_to_domain(content_block_to_acp(domain.clone())),
            Some(domain)
        );
    }

    #[test]
    fn resource_link_round_trips_with_mime() {
        let domain = ContentBlock::ResourceLink {
            name: "readme".into(),
            uri: "file:///readme.md".into(),
            mime_type: Some("text/markdown".into()),
        };
        assert_eq!(
            content_block_to_domain(content_block_to_acp(domain.clone())),
            Some(domain)
        );
    }

    #[test]
    fn text_resource_round_trips() {
        let domain = ContentBlock::Resource(ResourceContents::Text {
            uri: "file:///a.txt".into(),
            text: "body".into(),
            mime_type: None,
        });
        assert_eq!(
            content_block_to_domain(content_block_to_acp(domain.clone())),
            Some(domain)
        );
    }

    #[test]
    fn blob_resource_round_trips() {
        let domain = ContentBlock::Resource(ResourceContents::Blob {
            uri: "file:///a.bin".into(),
            blob: "AAAA".into(),
            mime_type: Some("application/octet-stream".into()),
        });
        assert_eq!(
            content_block_to_domain(content_block_to_acp(domain.clone())),
            Some(domain)
        );
    }

    #[test]
    fn audio_content_has_no_domain_form() {
        let audio = acp::ContentBlock::Audio(acp::AudioContent::new("data", "audio/mp3"));
        assert_eq!(content_block_to_domain(audio), None);
    }

    // ---- tool calls ----

    #[test]
    fn tool_call_maps_id_title_kind_and_status() {
        let call = acp::ToolCall::new("call-1", "Read file")
            .kind(acp::ToolKind::Read)
            .status(acp::ToolCallStatus::InProgress);
        let domain = tool_call_to_domain(call);
        assert_eq!(domain.id, "call-1");
        assert_eq!(domain.title, "Read file");
        assert_eq!(domain.kind, ToolKind::Read);
        assert_eq!(domain.status, ToolCallStatus::InProgress);
    }

    #[test]
    fn switch_mode_kind_collapses_to_other() {
        assert_eq!(
            tool_kind_to_domain(acp::ToolKind::SwitchMode),
            ToolKind::Other
        );
    }

    #[test]
    fn diff_content_maps_to_domain_diff() {
        let call = acp::ToolCall::new("c", "edit").content(vec![acp::ToolCallContent::Diff(
            acp::Diff::new("/tmp/file.rs", "new").old_text("old"),
        )]);
        let domain = tool_call_to_domain(call);
        assert_eq!(
            domain.content,
            vec![ToolCallContent::Diff {
                path: "/tmp/file.rs".into(),
                old_text: Some("old".into()),
                new_text: "new".into(),
            }]
        );
    }

    #[test]
    fn text_content_maps_to_domain_text() {
        let call = acp::ToolCall::new("c", "run")
            .content(vec![acp::ToolCallContent::Content(acp::Content::new(
                "output line",
            ))]);
        let domain = tool_call_to_domain(call);
        assert_eq!(
            domain.content,
            vec![ToolCallContent::Text("output line".into())]
        );
    }

    #[test]
    fn terminal_content_is_dropped() {
        let call = acp::ToolCall::new("c", "run").content(vec![acp::ToolCallContent::Terminal(
            acp::Terminal::new("term-1"),
        )]);
        assert!(tool_call_to_domain(call).content.is_empty());
    }

    // ---- plans ----

    #[test]
    fn plan_entries_map_priority_and_status() {
        let plan = acp::Plan::new(vec![acp::PlanEntry::new(
            "do a thing",
            acp::PlanEntryPriority::High,
            acp::PlanEntryStatus::InProgress,
        )]);
        let domain = plan_to_domain(plan);
        assert_eq!(domain.entries.len(), 1);
        assert_eq!(domain.entries[0].content, "do a thing");
        assert_eq!(domain.entries[0].priority, PlanPriority::High);
        assert_eq!(domain.entries[0].status, PlanEntryStatus::InProgress);
    }

    // ---- stop reason ----

    #[test]
    fn max_turn_requests_maps_to_domain_max_turns() {
        assert_eq!(
            stop_reason_to_domain(acp::StopReason::MaxTurnRequests),
            StopReason::MaxTurns
        );
    }

    #[test]
    fn common_stop_reasons_map_directly() {
        assert_eq!(
            stop_reason_to_domain(acp::StopReason::EndTurn),
            StopReason::EndTurn
        );
        assert_eq!(
            stop_reason_to_domain(acp::StopReason::Cancelled),
            StopReason::Cancelled
        );
    }

    // ---- permission outcome ----

    #[test]
    fn selected_outcome_carries_the_option_id() {
        let response = permission_outcome_to_acp(PermissionOutcome::Selected {
            option_id: "allow-once".into(),
        });
        match response.outcome {
            acp::RequestPermissionOutcome::Selected(selected) => {
                assert_eq!(selected.option_id.to_string(), "allow-once");
            }
            other => panic!("expected Selected, got {other:?}"),
        }
    }

    #[test]
    fn cancelled_outcome_maps_to_acp_cancelled() {
        let response = permission_outcome_to_acp(PermissionOutcome::Cancelled);
        assert!(matches!(
            response.outcome,
            acp::RequestPermissionOutcome::Cancelled
        ));
    }

    // ---- mcp servers ----

    #[test]
    fn only_enabled_servers_map_to_stdio() {
        use std::collections::HashMap;
        let servers = vec![
            McpServerConfig {
                name: "fs".into(),
                enabled: true,
                command: "npx".into(),
                args: vec!["server".into()],
                env: HashMap::from([("API_KEY".to_string(), "secret".to_string())]),
            },
            McpServerConfig {
                name: "disabled".into(),
                enabled: false,
                command: "nope".into(),
                args: vec![],
                env: HashMap::new(),
            },
        ];
        let mapped = mcp_servers_to_acp(&servers);
        assert_eq!(mapped.len(), 1);
        let acp::McpServer::Stdio(stdio) = &mapped[0] else {
            panic!("expected a stdio server");
        };
        assert_eq!(stdio.name, "fs");
        assert_eq!(stdio.command, std::path::PathBuf::from("npx"));
        assert_eq!(stdio.args, vec!["server".to_string()]);
        assert_eq!(stdio.env.len(), 1);
        assert_eq!(stdio.env[0].name, "API_KEY");
        assert_eq!(stdio.env[0].value, "secret");
    }

    // ---- session capabilities ----

    fn model_option() -> acp::SessionConfigOption {
        acp::SessionConfigOption::select(
            "model-config",
            "Model",
            "gpt-5",
            vec![
                acp::SessionConfigSelectOption::new("gpt-5", "GPT-5"),
                acp::SessionConfigSelectOption::new("o3", "o3"),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model)
    }

    #[test]
    fn session_init_maps_modes_and_models() {
        let modes = acp::SessionModeState::new(
            "code",
            vec![
                acp::SessionMode::new("code", "Code"),
                acp::SessionMode::new("ask", "Ask"),
            ],
        );
        let init =
            session_init_from(SessionId::from("s1"), Some(modes), Some(vec![model_option()]));
        assert_eq!(init.session_id, SessionId::from("s1"));
        assert_eq!(init.modes.len(), 2);
        assert_eq!(init.current_mode, Some("code".to_string()));
        assert_eq!(init.models.len(), 2);
        assert_eq!(init.current_model, Some("gpt-5".to_string()));
        // Slash commands arrive via a notification, not the session response.
        assert!(init.commands.is_empty());
    }

    #[test]
    fn session_init_is_empty_without_caps() {
        let init = session_init_from(SessionId::from("s1"), None, None);
        assert!(init.modes.is_empty());
        assert_eq!(init.current_mode, None);
        assert!(init.models.is_empty());
        assert_eq!(init.current_model, None);
    }

    #[test]
    fn model_config_id_finds_only_the_model_selector() {
        let options = vec![model_option()];
        assert_eq!(model_config_id(Some(&options)), Some("model-config".to_string()));
        assert_eq!(model_config_id(Some(&[])), None);
        assert_eq!(model_config_id(None), None);
    }
}
