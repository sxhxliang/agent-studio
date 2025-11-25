# Agent Todo List Component

A generic, reusable todo list component for displaying agent plan execution progress in GPUI applications.

## Overview

The `AgentTodoList` component follows the Agent Plan specification for tracking complex multi-step tasks. It displays a list of plan entries with their execution status, priority, and completion progress.

## Features

- **Status Tracking**: Visual indicators for pending, in-progress, and completed tasks
- **Progress Display**: Header shows completion ratio (e.g., "3/3")
- **Clean UI**: Minimal, icon-based design matching the reference specifications
- **Flexible**: Both stateless (`AgentTodoList`) and stateful (`AgentTodoListView`) variants

## Components

### `PlanEntry`

Represents a single task or goal with:
- `content`: Human-readable description
- `priority`: High, Medium, or Low
- `status`: Pending, InProgress, or Completed

### `AgentTodoList`

Stateless component that renders immediately. Use when you have static data or manage state elsewhere.

### `AgentTodoListView`

Stateful GPUI view with reactive updates. Use when you need dynamic updates and notifications.

## Usage

### Basic Usage (Stateless)

```rust
use agentx::{AgentTodoList, PlanEntry, PlanEntryStatus};

// In your render function
AgentTodoList::new()
    .title("3 to-dos")
    .entries(vec![
        PlanEntry::new("Add a ModelDropdown component under dashboard-ui")
            .with_status(PlanEntryStatus::Completed),
        PlanEntry::new("Embed it into the tool header")
            .with_status(PlanEntryStatus::Completed),
        PlanEntry::new("Close popover automatically after selection")
            .with_status(PlanEntryStatus::Completed),
    ])
```

### Stateful Usage (Reactive)

```rust
use agentx::{AgentTodoListView, PlanEntry, PlanEntryStatus, PlanEntryPriority};

// Create the view
let todo_list = AgentTodoListView::new(window, cx);

// Update entries dynamically
todo_list.update(cx, |list, cx| {
    list.add_entry(
        PlanEntry::new("Analyze the existing codebase structure")
            .with_priority(PlanEntryPriority::High)
            .with_status(PlanEntryStatus::Pending),
        cx
    );
});

// Update task status
todo_list.update(cx, |list, cx| {
    list.update_status(0, PlanEntryStatus::InProgress, cx);
});

// Render in your view
todo_list.clone()
```

## Status Icons

- **Pending** (Dash icon): Task not yet started
- **InProgress** (LoaderCircle icon): Currently executing
- **Completed** (CircleCheck icon): Successfully finished

## API Reference

### PlanEntry

```rust
// Create a new entry
PlanEntry::new("Task description")

// With priority
.with_priority(PlanEntryPriority::High)

// With status
.with_status(PlanEntryStatus::InProgress)
```

### AgentTodoList

```rust
// Create stateless component
AgentTodoList::new()
    .title("Task List")
    .entries(vec![...])
    .entry(single_entry)  // Add individual entry
```

### AgentTodoListView

```rust
// Create new view
AgentTodoListView::new(window, cx)

// Create with initial entries
AgentTodoListView::with_entries(entries, window, cx)

// Update methods
.set_entries(entries, cx)
.add_entry(entry, cx)
.update_entry(index, entry, cx)
.update_status(index, status, cx)
.set_title(title, cx)
```

## Integration with Agent Plan Protocol

This component is designed to work with the JSON-RPC Agent Plan protocol:

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "plan",
      "entries": [
        {
          "content": "Analyze the existing codebase structure",
          "priority": "high",
          "status": "pending"
        }
      ]
    }
  }
}
```

You can easily convert protocol messages to `PlanEntry` objects:

```rust
fn from_protocol(entry: ProtocolPlanEntry) -> PlanEntry {
    PlanEntry::new(entry.content)
        .with_priority(match entry.priority.as_str() {
            "high" => PlanEntryPriority::High,
            "low" => PlanEntryPriority::Low,
            _ => PlanEntryPriority::Medium,
        })
        .with_status(match entry.status.as_str() {
            "in_progress" => PlanEntryStatus::InProgress,
            "completed" => PlanEntryStatus::Completed,
            _ => PlanEntryStatus::Pending,
        })
}
```

## Styling

The component automatically uses your application's active theme:
- Colors adapt to the current theme (foreground, muted, accent, green)
- Font sizes: 14px for title and tasks
- Spacing: Consistent gaps and padding
- Icons: 16px size for clear visibility

## Agent Studio: Task Panel

See the reference implementation in the codebase for a complete Agent Studio of integrating the todo list into a panel.
