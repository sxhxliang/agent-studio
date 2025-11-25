# Agent Message Component

A UI component for displaying agent-sent messages with support for streaming content updates.

## Overview

The `AgentMessage` component follows the Agent Message Chunk specification for displaying messages from the AI agent. It supports streaming updates where content arrives in chunks, making it ideal for real-time conversational interfaces.

## Features

- **Agent Identification**: Shows agent bot icon and customizable agent name
- **Streaming Support**: Add content chunks progressively as they arrive
- **Status Indicator**: Shows loading icon when message is incomplete
- **Text Aggregation**: Automatically combines multiple chunks into coherent text
- **Reactive Updates**: Stateful view wrapper for dynamic content updates
- **Clean UI**: Minimal design focused on readability

## Components

### `AgentMessageContent`

Represents a single content chunk:
- `content_type`: Type of content (Text, Image, Other)
- `text`: Text content

### `AgentMessageData`

Complete agent message with:
- `session_id`: Session identifier
- `chunks`: Vector of content chunks (supports streaming)
- `agent_name`: Optional custom agent name (defaults to "Agent")
- `is_complete`: Whether the message is fully received

### `AgentMessage`

Stateless component for immediate rendering.

### `AgentMessageView`

Stateful GPUI view with reactive updates, perfect for streaming scenarios.

## Usage

### Basic Usage (Complete Message)

```rust
use agentx::{AgentMessageData, AgentMessage, AgentMessageContent};

// Create a complete message
let data = AgentMessageData::new("sess_abc123def456")
    .with_agent_name("Assistant")
    .add_chunk(AgentMessageContent::text(
        "I'll analyze your code for potential issues. Let me examine it..."
    ))
    .complete();

// Render in your view
AgentMessage::new("agent-msg-1", data)
```

### Streaming Usage (Progressive Updates)

```rust
use agentx::{AgentMessageData, AgentMessageView, AgentMessageContent};

// Create initial empty message
let data = AgentMessageData::new("sess_abc123def456")
    .with_agent_name("Assistant");
let message = AgentMessageView::new(data, window, cx);

// Simulate streaming: Add chunks as they arrive
message.update(cx, |msg, cx| {
    msg.append_text("I'll analyze ", cx);
});

message.update(cx, |msg, cx| {
    msg.append_text("your code ", cx);
});

message.update(cx, |msg, cx| {
    msg.append_text("for potential issues...", cx);
});

// Mark as complete when done
message.update(cx, |msg, cx| {
    msg.mark_complete(cx);
});

// Render in your view
message.clone()
```

### Alternative: Add Complete Chunks

```rust
// Add chunks individually
message.update(cx, |msg, cx| {
    msg.add_chunk(
        AgentMessageContent::text("I'll analyze your code "),
        cx
    );
});

message.update(cx, |msg, cx| {
    msg.add_chunk(
        AgentMessageContent::text("for potential issues..."),
        cx
    );
});
```

## Visual Design

### Layout Structure

```
┌─────────────────────────────────────┐
│ 🤖 Assistant ⭘                      │ (⭘ = loading indicator when incomplete)
│                                     │
│   I'll analyze your code for        │
│   potential issues. Let me          │
│   examine it...                     │
└─────────────────────────────────────┘
```

### Complete Message
- Bot icon (left)
- Agent name (customizable, default "Agent")
- No loading indicator

### Incomplete Message
- Bot icon (left)
- Agent name
- Loading indicator (rotating circle icon)
- Content updates in real-time

## Integration with Session Update Protocol

This component is designed to work with the JSON-RPC Session Update protocol for streaming messages:

### Receiving Message Chunks

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": {
        "type": "text",
        "text": "I'll analyze your code for potential issues. Let me examine it..."
      }
    }
  }
}
```

### Protocol Integration Agent Studio

```rust
use agentx::{AgentMessageView, AgentMessageContent};

// Store message view in your state
struct ConversationState {
    current_message: Option<Entity<AgentMessageView>>,
}

// Handle incoming chunks
fn handle_message_chunk(
    state: &mut ConversationState,
    session_id: String,
    chunk: ProtocolContent,
    window: &mut Window,
    cx: &mut App,
) {
    // Get or create message view
    let message = if let Some(msg) = &state.current_message {
        msg.clone()
    } else {
        let data = AgentMessageData::new(session_id);
        let msg = AgentMessageView::new(data, window, cx);
        state.current_message = Some(msg.clone());
        msg
    };

    // Append the chunk
    message.update(cx, |msg, cx| {
        if chunk.r#type == "text" {
            msg.append_text(chunk.text, cx);
        }
    });
}

// Handle message completion
fn handle_message_complete(
    state: &mut ConversationState,
    cx: &mut App,
) {
    if let Some(message) = &state.current_message {
        message.update(cx, |msg, cx| {
            msg.mark_complete(cx);
        });
        state.current_message = None;
    }
}
```

## API Reference

### AgentMessageContent

```rust
// Create text content
AgentMessageContent::text("Your text here")

// Set content type
.with_type(AgentContentType::Text)
```

### AgentMessageData

```rust
// Create new message
AgentMessageData::new(session_id)
    .with_agent_name("Assistant")
    .with_chunks(vec![...])
    .add_chunk(content)
    .complete()

// Get full combined text
.full_text() -> SharedString
```

### AgentMessage

```rust
// Create stateless component
AgentMessage::new(id, data)
```

### AgentMessageView

```rust
// Create new view
AgentMessageView::new(data, window, cx)

// Update methods
.update_data(data, cx)
.add_chunk(chunk, cx)
.append_text(text, cx)          // Append to last chunk or create new
.mark_complete(cx)
.set_agent_name(name, cx)
.clear(cx)                       // Clear all chunks

// Query methods
.get_text(cx) -> SharedString
.is_complete(cx) -> bool
```

## Streaming Patterns

### Pattern 1: Character-by-Character Streaming

```rust
// Simulate character-by-character streaming
for char in "Hello, world!".chars() {
    message.update(cx, |msg, cx| {
        msg.append_text(char.to_string(), cx);
    });
    // Add small delay in real scenario
}
```

### Pattern 2: Word-by-Word Streaming

```rust
// Simulate word-by-word streaming
for word in "Hello world from agent".split_whitespace() {
    message.update(cx, |msg, cx| {
        msg.append_text(format!("{} ", word), cx);
    });
}
```

### Pattern 3: Chunk-Based Streaming

```rust
// Receive chunks from server
fn on_chunk_received(chunk: String, message: &Entity<AgentMessageView>, cx: &mut App) {
    message.update(cx, |msg, cx| {
        msg.append_text(chunk, cx);
    });
}
```

## Styling

The component automatically uses your application's active theme:

### Agent Label
- Icon: Bot icon (16px, accent color)
- Text: 13px, semibold
- Loading icon: LoaderCircle (12px, muted foreground) when incomplete

### Message Content
- Font size: 14px
- Line height: 22px
- Color: Foreground
- Left padding: 24px (pl_6)

## Agent Studios

### Simple Complete Message

```rust
let data = AgentMessageData::new("sess_123")
    .add_chunk(AgentMessageContent::text("Hello! How can I help you?"))
    .complete();

AgentMessage::new("msg-1", data)
```

### Streaming Message with Status

```rust
// Create message
let message = AgentMessageView::new(
    AgentMessageData::new("sess_123"),
    window,
    cx
);

// Start streaming
message.update(cx, |msg, cx| {
    msg.append_text("Analyzing", cx);
});

// Continue streaming
message.update(cx, |msg, cx| {
    msg.append_text(" your code...", cx);
});

// Complete
message.update(cx, |msg, cx| {
    msg.mark_complete(cx);
});
```

### Building a Conversation View

```rust
use gpui_component::v_flex;

v_flex()
    .gap_4()
    .child(user_message_1.clone())
    .child(agent_message_1.clone())  // Complete message
    .child(user_message_2.clone())
    .child(agent_message_2.clone())  // Streaming message (incomplete)
```

## State Management

The `AgentMessageView` manages the streaming state automatically:

```rust
// Check if message is still receiving chunks
if message.read(cx).is_complete {
    // Message is complete
} else {
    // Message is still streaming
}

// Get current text at any time
let current_text = message.update(cx, |msg, cx| {
    msg.get_text(cx)
});
```

## Comparison with UserMessage

| Feature | UserMessage | AgentMessage |
|---------|-------------|--------------|
| Icon | User | Bot |
| Name | "You" | Customizable (default "Agent") |
| Streaming | No | Yes |
| Status Indicator | No | Yes (loading icon) |
| Content Types | Text, Resource | Text (extensible) |
| Collapsible | Yes (resources) | No |

## Advanced Usage

### Custom Agent Names

```rust
message.update(cx, |msg, cx| {
    msg.set_agent_name("Claude", cx);
});
```

### Clearing Message Content

```rust
// Clear and reuse message view
message.update(cx, |msg, cx| {
    msg.clear(cx);
});
```

### Multiple Content Types

```rust
// Though currently text-focused, the structure supports extension
let chunk1 = AgentMessageContent::text("Here's the analysis:")
    .with_type(AgentContentType::Text);

let chunk2 = AgentMessageContent {
    content_type: AgentContentType::Image,
    text: "data:image/png;base64,...".into(),
};

data = data.add_chunk(chunk1).add_chunk(chunk2);
```

## Performance Considerations

### Efficient Streaming

The `append_text()` method is optimized for streaming:
- Appends to the last text chunk instead of creating new chunks
- Reduces memory allocations
- Maintains single notification per update

### Memory Management

```rust
// For long streaming sessions, consider limiting chunk count
if message.read(cx).chunks.len() > 100 {
    // Merge chunks or use full_text() to create single chunk
    let full = message.update(cx, |msg, cx| msg.get_text(cx));
    message.update(cx, |msg, cx| {
        msg.clear(cx);
        msg.append_text(full, cx);
    });
}
```

## Error Handling

```rust
// Handle streaming errors
fn handle_stream_error(message: &Entity<AgentMessageView>, error: &str, cx: &mut App) {
    message.update(cx, |msg, cx| {
        msg.append_text(format!("\n\n[Error: {}]", error), cx);
        msg.mark_complete(cx);
    });
}
```
