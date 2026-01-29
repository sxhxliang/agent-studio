#[cfg(test)]
mod tests {
    use agent_client_protocol::{ContentBlock, ContentChunk, SessionUpdate};

    #[test]
    fn test_session_update_serialization() {
        // Create a simple SessionUpdate from text
        let content_block = ContentBlock::from("Test message".to_string());
        let chunk = ContentChunk::new(content_block);
        let update = SessionUpdate::UserMessageChunk(chunk);

        // Serialize to see expected format
        let json = serde_json::to_string_pretty(&update).unwrap();

        // Try to deserialize it back
        let parsed: SessionUpdate = serde_json::from_str(&json).unwrap();

        // Verify we can match on the variant
        assert!(matches!(parsed, SessionUpdate::UserMessageChunk(_)));
    }

    #[test]
    fn test_agent_message_chunk_serialization() {
        let content_block = ContentBlock::from("Agent response".to_string());
        let chunk = ContentChunk::new(content_block);
        let update = SessionUpdate::AgentMessageChunk(chunk);

        let json = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json).unwrap();

        assert!(matches!(parsed, SessionUpdate::AgentMessageChunk(_)));
    }

    #[test]
    fn test_agent_thought_chunk_serialization() {
        let content_block = ContentBlock::from("Thinking...".to_string());
        let chunk = ContentChunk::new(content_block);
        let update = SessionUpdate::AgentThoughtChunk(chunk);

        let json = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json).unwrap();

        assert!(matches!(parsed, SessionUpdate::AgentThoughtChunk(_)));
    }

    #[test]
    fn test_session_update_common_variants() {
        // Test that common variants serialize/deserialize correctly
        let variants: Vec<SessionUpdate> = vec![
            SessionUpdate::UserMessageChunk(ContentChunk::new(ContentBlock::from(
                "user".to_string(),
            ))),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(
                "agent".to_string(),
            ))),
            SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::from(
                "thinking".to_string(),
            ))),
        ];

        for update in variants {
            let json = serde_json::to_string(&update).unwrap();
            let _parsed: SessionUpdate = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", json, e));
        }
    }
}
