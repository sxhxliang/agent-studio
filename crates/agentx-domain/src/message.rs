//! Message content: the semantic building blocks of prompts and replies.
//!
//! These are domain-owned mirrors of the ACP content model. The ACP adapter
//! ([`agentx-acp`]) maps protocol types to and from these; the domain never
//! references ACP types directly (the anti-corruption layer).

use serde::{Deserialize, Serialize};

/// Who authored a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Agent,
}

/// A single block of content within a message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentBlock {
    Text(String),
    Image { mime_type: String, data: String },
    ResourceLink {
        name: String,
        uri: String,
        mime_type: Option<String>,
    },
    Resource(ResourceContents),
}

/// The contents of an embedded resource (inline text or base64 blob).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceContents {
    Text {
        uri: String,
        text: String,
        mime_type: Option<String>,
    },
    Blob {
        uri: String,
        blob: String,
        mime_type: Option<String>,
    },
}

impl ContentBlock {
    pub fn text(value: impl Into<String>) -> Self {
        ContentBlock::Text(value.into())
    }

    /// Best-effort plain-text view of a block, used for previews and search.
    pub fn as_plain_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text(text) => Some(text),
            ContentBlock::Resource(ResourceContents::Text { text, .. }) => Some(text),
            _ => None,
        }
    }
}

/// A complete message (as opposed to an incremental streamed chunk).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(content: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    pub fn agent(content: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Agent,
            content,
        }
    }

    /// Concatenated plain text of every text-bearing block.
    pub fn plain_text(&self) -> String {
        self.content
            .iter()
            .filter_map(ContentBlock::as_plain_text)
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_skips_non_text_blocks() {
        let message = Message::user(vec![
            ContentBlock::text("hello "),
            ContentBlock::Image {
                mime_type: "image/png".into(),
                data: "...".into(),
            },
            ContentBlock::text("world"),
        ]);
        assert_eq!(message.plain_text(), "hello world");
    }
}
