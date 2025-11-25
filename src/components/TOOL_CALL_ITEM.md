# Tool Call Item Component

A collapsible component for displaying agent tool call invocations with status tracking and expandable content.

## Overview

The `ToolCallItem` component follows the Agent Tool Call specification for tracking tool execution. It displays tool invocations with their status, kind, and optional content that can be expanded when available.

## Features

- **Status Icons**: Visual indicators for pending, in-progress, completed, and failed states
- **Tool Kind Icons**: Different icons for read, edit, delete, search, execute, think, fetch operations
- **Collapsible Content**: Expandable section that only shows when content is available
- **Reactive Updates**: Stateful view wrapper for dynamic updates
- **Clean UI**: Compact, icon-based design with theme integration

## Components

### `ToolCallData`

Represents a tool call invocation with:
- `tool_call_id`: Unique identifier
- `title`: Human-readable description
- `kind`: Tool category (Read, Edit, Delete, Move, Search, Execute, Think, Fetch, Other)
- `status`: Execution status (Pending, InProgress, Completed, Failed)
- `content`: Optional text content produced by the tool

### `ToolCallItem`

Stateless component for immediate rendering. Use when you have static data.

### `ToolCallItemView`

Stateful GPUI view with reactive updates. Use when you need dynamic updates.

## Usage

### Basic Usage (Stateless)

```rust
use agentx::{ToolCallData, ToolCallItem, ToolCallKind, ToolCallStatus};

// Create tool call data
let data = ToolCallData::new("call_001", "Reading configuration file")
    .with_kind(ToolCallKind::Read)
    .with_status(ToolCallStatus::Pending);

// Render in your view
ToolCallItem::new("tool-call-1", data)
    .open(false)
```

### Stateful Usage (Reactive)

```rust
use agentx::{
    ToolCallData, ToolCallItemView, ToolCallKind, ToolCallStatus, ToolCallContent
};

// Create the view
let data = ToolCallData::new("call_001", "Reading configuration file")
    .with_kind(ToolCallKind::Read)
    .with_status(ToolCallStatus::Pending);

let tool_call = ToolCallItemView::new(data, window, cx);

// Update status dynamically
tool_call.update(cx, |item, cx| {
    item.update_status(ToolCallStatus::InProgress, cx);
});

// Add content (makes it expandable)
tool_call.update(cx, |item, cx| {
    item.add_content(
        ToolCallContent::new("Found 3 configuration files..."),
        cx
    );
});

// Mark as completed
tool_call.update(cx, |item, cx| {
    item.update_status(ToolCallStatus::Completed, cx);
});

// Render in your view
tool_call.clone()
```

## Tool Kinds and Icons

- **Read** (File icon): Reading files or data
- **Edit** (Replace icon): Modifying files or content
- **Delete** (Delete icon): Removing files or data
- **Move** (ArrowRight icon): Moving or renaming files
- **Search** (Search icon): Searching for information
- **Execute** (SquareTerminal icon): Running commands or code
- **Think** (Bot icon): Internal reasoning or planning
- **Fetch** (Globe icon): Retrieving external data
- **Other** (Ellipsis icon): Other tool types

## Status Icons and Colors

- **Pending** (Dash icon, muted): Tool call not yet started
- **InProgress** (LoaderCircle icon, accent): Currently executing
- **Completed** (CircleCheck icon, green): Successfully finished
- **Failed** (CircleX icon, red): Execution failed

## Collapsible Behavior

The component uses the `Collapsible` component to show/hide content:

- Initially, only the header is shown (kind icon, title, status icon)
- When `content` is added, a chevron button appears
- Clicking the chevron toggles the expanded state
- Content is only shown when expanded and has content
- If no content exists, the chevron button is hidden

## Integration with Agent Tool Call Protocol

This component is designed to work with the JSON-RPC Agent Tool Call protocol:

### Creating a Tool Call

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "call_001",
      "title": "Reading configuration file",
      "kind": "read",
      "status": "pending"
    }
  }
}
```

### Updating a Tool Call

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "call_001",
      "status": "in_progress",
      "content": [
        {
          "type": "content",
          "content": {
            "type": "text",
            "text": "Found 3 configuration files..."
          }
        }
      ]
    }
  }
}
```

### Protocol Conversion Agent Studio

```rust
fn from_protocol(call: ProtocolToolCall) -> ToolCallData {
    ToolCallData::new(call.tool_call_id, call.title)
        .with_kind(ToolCallKind::from_str(&call.kind))
        .with_status(ToolCallStatus::from_str(&call.status))
        .with_content(
            call.content
                .into_iter()
                .map(|c| ToolCallContent::new(c.text))
                .collect()
        )
}

fn update_from_protocol(view: &Entity<ToolCallItemView>, update: ProtocolToolCallUpdate, cx: &mut App) {
    view.update(cx, |item, cx| {
        item.update_status(ToolCallStatus::from_str(&update.status), cx);

        if let Some(content) = update.content {
            item.set_content(
                content.into_iter()
                    .map(|c| ToolCallContent::new(c.text))
                    .collect(),
                cx
            );
        }
    });
}
```

## API Reference

### ToolCallData

```rust
// Create new tool call data
ToolCallData::new(tool_call_id, title)
    .with_kind(ToolCallKind::Read)
    .with_status(ToolCallStatus::Pending)
    .with_content(vec![...])

// Check if has content
.has_content() -> bool
```

### ToolCallItem

```rust
// Create stateless component
ToolCallItem::new(id, data)
    .open(false)
    .on_toggle(|ev, window, cx| { ... })
```

### ToolCallItemView

```rust
// Create new view
ToolCallItemView::new(data, window, cx)

// Update methods
.update_data(data, cx)
.update_status(status, cx)
.add_content(content, cx)
.set_content(vec![...], cx)
.toggle(cx)
.set_open(open, cx)
```

## Styling

The component automatically uses your application's active theme:
- Background: Secondary color for header
- Text: Foreground color for title, muted for content
- Icons: 16px for kind, 14px for status
- Status colors: Green (completed), Red (failed), Accent (in progress), Muted (pending)
- Border radius: Theme radius
- Padding: 8px (header), 12px left (content)

## Agent Studio: Building a Tool Call List

```rust
use gpui_component::v_flex;

v_flex()
    .gap_2()
    .child(tool_call_1.clone())
    .child(tool_call_2.clone())
    .child(tool_call_3.clone())
```

This creates a vertical list of tool calls that can be individually expanded and updated.
