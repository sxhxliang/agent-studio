#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::{ContentBlock, ContentChunk, SessionUpdate};

    #[test]
    fn test_session_update_serialization() {
        // Create a simple SessionUpdate from text
        let content_block = ContentBlock::from("Test message".to_string());
        let chunk = ContentChunk::new(content_block);
        let update = SessionUpdate::UserMessageChunk(chunk);

        // Serialize to see expected format
        let json = serde_json::to_string_pretty(&update).unwrap();
        println!("\n📝 Expected JSON format for SessionUpdate:");
        println!("{}\n", json);

        // Try to deserialize it back
        let parsed: SessionUpdate = serde_json::from_str(&json).unwrap();
        println!("✅ Successfully round-tripped Session Update");
    }

    #[test]
    fn test_original_mock_conversation_json() {
        let json_str = include_str!("../mock_conversation_acp.json");
        let result = serde_json::from_str::<Vec<SessionUpdate>>(json_str);

        match &result {
            Ok(updates) => {
                println!(
                    "\n✅ Successfully parsed {} items from original JSON",
                    updates.len()
                );
                for (i, update) in updates.iter().take(3).enumerate() {
                    println!("  [{}] {:?}", i, std::mem::discriminant(update));
                }
            }
            Err(e) => {
                println!("\n❌ Failed to parse original JSON: {}", e);
            }
        }
    }
}
