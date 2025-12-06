/// Helper functions for ConversationPanel
use regex::Regex;

/// Extract text content from XML-like tags using regex based on ToolKind
/// For example: "```\n<tool_use_error>File does not exist.</tool_use_error>\n```"
/// Returns: "File does not exist."
///
/// This function decides whether to extract XML content based on the tool type:
/// - For Execute, Other, and similar types: Extract XML content
/// - For other types: Return original text
pub fn extract_xml_content(text: &str, tool_kind: &agent_client_protocol::ToolKind) -> String {
    // Decide whether to extract XML based on tool kind
    let should_extract = matches!(
        tool_kind,
        agent_client_protocol::ToolKind::Execute
            | agent_client_protocol::ToolKind::Other
            | agent_client_protocol::ToolKind::Read
    );

    if !should_extract {
        // For other tool types, return the text as-is (just strip code fences)
        return text
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string();
    }

    // Pattern to match XML-like tags: <tag_name>content</tag_name>
    // This captures the content between any XML tags
    let re = Regex::new(r"<[^>]+>([^<]*)</[^>]+>").unwrap();

    let mut result = String::new();
    for cap in re.captures_iter(text) {
        if let Some(content) = cap.get(1) {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(content.as_str());
        }
    }

    // If no XML tags found, return the original text (stripped of markdown code fences)
    if result.is_empty() {
        text.trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        result
    }
}

/// Get a unique ElementId from a string identifier
pub fn get_element_id(id: &str) -> gpui::ElementId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    gpui::ElementId::from(("item", hasher.finish()))
}

/// Extract text from ContentBlock for display
pub fn extract_text_from_content(content: &agent_client_protocol::ContentBlock) -> String {
    use agent_client_protocol::{ContentBlock, EmbeddedResourceResource};

    match content {
        ContentBlock::Text(text_content) => text_content.text.clone(),
        ContentBlock::Image(img) => {
            format!("[Image: {}]", img.mime_type)
        }
        ContentBlock::Audio(audio) => {
            format!("[Audio: {}]", audio.mime_type)
        }
        ContentBlock::ResourceLink(link) => {
            format!("[Resource: {}]", link.name)
        }
        ContentBlock::Resource(resource) => match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(text_res) => {
                format!(
                    "[Resource: {}]\n{}",
                    text_res.uri,
                    &text_res.text[..text_res.text.len().min(200)]
                )
            }
            EmbeddedResourceResource::BlobResourceContents(blob_res) => {
                format!("[Binary Resource: {}]", blob_res.uri)
            }
            _ => "[Unknown Resource]".to_string(),
        },
        _ => "[Unknown Content]".to_string(),
    }
}

/// Get a human-readable type name for SessionUpdate (for logging)
pub fn session_update_type_name(update: &agent_client_protocol::SessionUpdate) -> &'static str {
    use agent_client_protocol::SessionUpdate;

    match update {
        SessionUpdate::UserMessageChunk(_) => "UserMessageChunk",
        SessionUpdate::AgentMessageChunk(_) => "AgentMessageChunk",
        SessionUpdate::AgentThoughtChunk(_) => "AgentThoughtChunk",
        SessionUpdate::ToolCall(_) => "ToolCall",
        SessionUpdate::ToolCallUpdate(_) => "ToolCallUpdate",
        SessionUpdate::Plan(_) => "Plan",
        SessionUpdate::AvailableCommandsUpdate(_) => "AvailableCommandsUpdate",
        SessionUpdate::CurrentModeUpdate(_) => "CurrentModeUpdate",
        _ => "Unknown/Future SessionUpdate Type",
    }
}
