# User Message Component

A UI component for displaying user-sent messages with support for text and resource (code file) content with collapsible display.

## Overview

The `UserMessage` component follows the Agent Session Prompt specification for displaying user messages. It supports multiple content types including plain text and file resources with syntax-highlighted code display.

## Features

- **User Identification**: Shows user avatar icon and "You" label
- **Mixed Content**: Support for text and resource content in a single message
- **Collapsible Resources**: File resources can be expanded/collapsed to show code
- **File Information**: Display filename, MIME type, and line count
- **Syntax Highlighting**: Monospace font for code display
- **Reactive Updates**: Stateful view wrapper for dynamic content updates

## Components

### `MessageContent`

Enum representing different message content types:
- `Text { text }`: Plain text content
- `Resource { resource }`: File/code resource content

### `ResourceContent`

Represents a file or code resource with:
- `uri`: Resource URI (e.g., `file:///home/user/project/main.py`)
- `mime_type`: MIME type (e.g., `text/x-python`)
- `text`: Text content of the resource

### `UserMessageData`

Complete user message with:
- `session_id`: Session identifier
- `contents`: Vector of `MessageContent` items

### `UserMessage`

Stateless component for immediate rendering.

### `UserMessageView`

Stateful GPUI view with reactive updates and resource collapse state management.

## Usage

### Basic Usage (Stateless)

```rust
use agentx::{UserMessageData, UserMessage, MessageContent, ResourceContent};

// Create message with text
let data = UserMessageData::new("sess_abc123def456")
    .add_content(MessageContent::text("Can you analyze this code for potential issues?"))
    .add_content(MessageContent::resource(
        ResourceContent::new(
            "file:///home/user/project/main.py",
            "text/x-python",
            "def process_data(items):\n    for item in items:\n        print(item)"
        )
    ));

// Render in your view
UserMessage::new("user-msg-1", data)
```

### Stateful Usage (Reactive)

```rust
use agentx::{
    UserMessageData, UserMessageView, MessageContent, ResourceContent
};

// Create the view
let data = UserMessageData::new("sess_abc123def456");
let message = UserMessageView::new(data, window, cx);

// Add text content
message.update(cx, |msg, cx| {
    msg.add_content(
        MessageContent::text("Can you analyze this code for potential issues?"),
        cx
    );
});

// Add resource content
message.update(cx, |msg, cx| {
    msg.add_content(
        MessageContent::resource(
            ResourceContent::new(
                "file:///home/user/project/main.py",
                "text/x-python",
                "def process_data(items):\n    for item in items:\n        print(item)"
            )
        ),
        cx
    );
});

// Toggle resource expansion
message.update(cx, |msg, cx| {
    msg.toggle_resource(0, cx); // Toggle first resource
});

// Render in your view
message.clone()
```

## Visual Design

### Layout Structure

```
┌─────────────────────────────────────┐
│ 👤 You                              │
│                                     │
│   Text content here...              │
│                                     │
│   ┌───────────────────────────────┐ │
│   │ 📄 main.py        12 lines ▼ │ │
│   └───────────────────────────────┘ │
│   ┌───────────────────────────────┐ │
│   │ def process_data(items):      │ │
│   │     for item in items:        │ │
│   │         print(item)           │ │
│   └───────────────────────────────┘ │
└─────────────────────────────────────┘
```

### Resource Header (Collapsed)
- File icon (left)
- Filename (center, medium weight)
- Line count (right, muted)
- Chevron down icon (right)
- Muted background with border

### Resource Content (Expanded)
- Monospace font (Monaco, Courier New)
- Secondary background
- Border with theme color
- Proper line height for readability

## Integration with Session Prompt Protocol

This component is designed to work with the JSON-RPC Session Prompt protocol:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session/prompt",
  "params": {
    "sessionId": "sess_abc123def456",
    "prompt": [
      {
        "type": "text",
        "text": "Can you analyze this code for potential issues?"
      },
      {
        "type": "resource",
        "resource": {
          "uri": "file:///home/user/project/main.py",
          "mimeType": "text/x-python",
          "text": "def process_data(items):\n    for item in items:\n        print(item)"
        }
      }
    ]
  }
}
```

### Protocol Conversion Agent Studio

```rust
fn from_protocol(session_id: String, prompt: Vec<ProtocolPromptItem>) -> UserMessageData {
    let mut data = UserMessageData::new(session_id);

    for item in prompt {
        let content = match item.r#type.as_str() {
            "text" => MessageContent::text(item.text.unwrap_or_default()),
            "resource" => {
                if let Some(resource) = item.resource {
                    MessageContent::resource(ResourceContent::new(
                        resource.uri,
                        resource.mime_type,
                        resource.text,
                    ))
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        data = data.add_content(content);
    }

    data
}
```

## API Reference

### MessageContent

```rust
// Create text content
MessageContent::text("Your text here")

// Create resource content
MessageContent::resource(ResourceContent::new(uri, mime_type, text))

// Get content type
.content_type() -> MessageContentType
```

### ResourceContent

```rust
// Create new resource
ResourceContent::new(uri, mime_type, text)

// Get filename from URI
.filename() -> SharedString

// Get appropriate icon
.icon() -> IconName
```

### UserMessageData

```rust
// Create new message
UserMessageData::new(session_id)
    .with_contents(vec![...])
    .add_content(content)
```

### UserMessage

```rust
// Create stateless component
UserMessage::new(id, data)
    .with_resource_state(index, open)
```

### UserMessageView

```rust
// Create new view
UserMessageView::new(data, window, cx)

// Update methods
.update_data(data, cx)
.add_content(content, cx)
.toggle_resource(index, cx)
.set_resource_open(index, open, cx)
```

## Styling

The component automatically uses your application's active theme:

### Text Content
- Font size: 14px
- Line height: 22px
- Color: Foreground

### User Label
- Icon: User icon (16px, accent color)
- Text: 13px, semibold

### Resource Header
- Background: Muted
- Border: 1px, theme border color
- Font size: 13px (filename), 11px (line count)
- File icon: 16px, accent color

### Resource Content
- Background: Secondary
- Border: 1px, theme border color
- Font: Monaco, Courier New (12px monospace)
- Line height: 18px
- Color: Foreground

## Agent Studios

### Simple Text Message

```rust
let data = UserMessageData::new("sess_123")
    .add_content(MessageContent::text("Hello, can you help me?"));

UserMessage::new("msg-1", data)
```

### Message with Multiple Resources

```rust
let data = UserMessageData::new("sess_123")
    .add_content(MessageContent::text("Please review these files:"))
    .add_content(MessageContent::resource(
        ResourceContent::new(
            "file:///src/main.rs",
            "text/rust",
            "fn main() {\n    println!(\"Hello!\");\n}"
        )
    ))
    .add_content(MessageContent::resource(
        ResourceContent::new(
            "file:///src/lib.rs",
            "text/rust",
            "pub fn helper() {}"
        )
    ));

UserMessage::new("msg-2", data)
```

### Building a Conversation View

```rust
use gpui_component::v_flex;

v_flex()
    .gap_4()
    .child(user_message_1.clone())
    .child(agent_response_1.clone())
    .child(user_message_2.clone())
    .child(agent_response_2.clone())
```

## Resource State Management

The `UserMessageView` automatically tracks the open/closed state of each resource:

```rust
// Get number of resources
let resource_count = data.contents
    .iter()
    .filter(|c| matches!(c, MessageContent::Resource { .. }))
    .count();

// Toggle individual resources
message.update(cx, |msg, cx| {
    msg.toggle_resource(0, cx); // Toggle first resource
    msg.set_resource_open(1, true, cx); // Expand second resource
});
```

## MIME Type Icons

The component automatically selects icons based on MIME type:
- `text/x-python` → File icon
- `text/javascript`, `text/typescript` → File icon
- `text/rust` → File icon
- `application/json` → File icon
- Others → File icon (default)

Note: You can extend `ResourceContent::icon()` to support more specific icons based on file types.
