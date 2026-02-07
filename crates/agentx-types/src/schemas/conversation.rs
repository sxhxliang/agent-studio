use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ConversationItem {
    UserMessage {
        id: String,
        data: UserMessageDataSchema,
    },
    AgentMessage {
        id: String,
        data: AgentMessageDataSchema,
    },
    /// Plan item following ACP's SessionUpdate::Plan format
    Plan(PlanSchema),
    ToolCallGroup {
        items: Vec<ToolCallItemSchema>,
    },
}

/// User message data schema aligned with ACP's PromptRequest format
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserMessageDataSchema {
    pub session_id: String,
    /// Content blocks following ACP ContentBlock structure
    pub prompt: Vec<ContentBlockSchema>,
}

/// Content block schema aligned with ACP's ContentBlock enum
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockSchema {
    Text(TextContentSchema),
    Image(ImageContentSchema),
    ResourceLink(ResourceLinkSchema),
    Resource(EmbeddedResourceSchema),
}

/// Text content schema
#[derive(Debug, Deserialize, Clone)]
pub struct TextContentSchema {
    pub text: String,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Image content schema
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageContentSchema {
    pub data: String,
    pub mime_type: String,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Resource link schema (reference to a resource without embedding content)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLinkSchema {
    pub name: String,
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Embedded resource schema (contains the actual content)
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResourceSchema {
    pub resource: ResourceContentsSchema,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Resource contents schema (text or blob)
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResourceContentsSchema {
    TextResourceContents(TextResourceContentsSchema),
    BlobResourceContents(BlobResourceContentsSchema),
}

/// Text resource contents schema
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextResourceContentsSchema {
    pub uri: String,
    pub text: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Blob resource contents schema
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContentsSchema {
    pub uri: String,
    pub blob: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Agent message data schema aligned with ACP's ContentChunk format
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDataSchema {
    pub session_id: String,
    /// Content chunks following ACP ContentChunk structure
    pub chunks: Vec<ContentChunkSchema>,
    /// Extended metadata (agent_name, is_complete stored in _meta)
    #[serde(rename = "_meta")]
    pub meta: Option<AgentMessageMetaSchema>,
}

/// Content chunk schema aligned with ACP's ContentChunk
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContentChunkSchema {
    /// Content block following ACP's ContentBlock structure
    pub content: ContentBlockSchema,
    /// Extension point for implementations
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Extended metadata for agent messages
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageMetaSchema {
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub is_complete: bool,
}

/// Plan schema aligned with ACP's Plan structure from SessionUpdate::Plan
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanSchema {
    /// The list of tasks to be accomplished
    pub entries: Vec<PlanEntrySchema>,
    /// Extension point for implementations (can contain title, etc.)
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Plan entry schema aligned with ACP's PlanEntry
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanEntrySchema {
    /// Human-readable description of what this task aims to accomplish
    pub content: String,
    /// The relative importance of this task (high, medium, low)
    pub priority: String,
    /// Current execution status (pending, in_progress, completed)
    pub status: String,
    /// Extension point for implementations
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Tool call item schema following ACP's ToolCall format
#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallItemSchema {
    pub id: String,
    pub data: ToolCallSchema,
    pub open: bool,
}

/// Tool call schema aligned with ACP's ToolCall structure
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallSchema {
    /// Unique identifier for this tool call
    pub tool_call_id: String,
    /// Human-readable title describing what the tool is doing
    pub title: String,
    /// The category of tool being invoked (read, edit, search, etc.)
    #[serde(default)]
    pub kind: Option<String>,
    /// Current execution status (pending, in_progress, completed, failed)
    #[serde(default)]
    pub status: Option<String>,
    /// Content produced by the tool call
    #[serde(default)]
    pub content: Vec<ToolCallContentItemSchema>,
    /// Extension point for implementations
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

/// Tool call content item schema (simplified for mock data)
#[derive(Debug, Deserialize, Clone)]
pub struct ToolCallContentItemSchema {
    pub text: String,
}
