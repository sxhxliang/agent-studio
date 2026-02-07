# agentx-acp-ui

ACP (Agent Client Protocol) message UI components used by AgentX.

## Components

- `AgentMessage`, `AgentMessageView`
- `UserMessage`, `UserMessageView`
- `AgentTodoList`, `AgentTodoListView`
- `ToolCallItem`, `ToolCallItemView`
- `DiffView`, `DiffSummary`
- `PermissionRequest`, `PermissionRequestView`

## Usage

### Agent Message

```rust
use std::sync::Arc;

use agentx_acp_ui::{AgentMessage, AgentMessageData};
use gpui_component::Icon;

let icon_provider = Arc::new(|name: &str| Icon::new(get_agent_icon(name)));
let data = AgentMessageData::new("session-1").add_text("Hello");
let view = AgentMessage::new("agent-message", data).icon_provider(icon_provider);
```

### Tool Call Item

```rust
use std::sync::Arc;

use agentx_acp_ui::{ToolCallItem, ToolCallItemOptions};

let options = ToolCallItemOptions::default().on_open_detail(Arc::new(|tool_call, window, cx| {
    let action = PanelAction::show_tool_call_detail(
        tool_call.tool_call_id.to_string(),
        tool_call,
    );
    window.dispatch_action(Box::new(action), cx);
}));

let item = ToolCallItem::with_options(tool_call, options);
```

### Permission Request

```rust
use std::sync::Arc;

use agentx_acp_ui::{PermissionRequest, PermissionRequestOptions};

let options = PermissionRequestOptions {
    on_response: Some(Arc::new(|permission_id, response, cx| {
        let store = permission_store.clone();
        cx.spawn(async move |_entity, _cx| {
            let _ = store.respond(&permission_id, response).await;
        })
        .detach();
    })),
};

let request = PermissionRequest::with_options(
    permission_id,
    session_id,
    &tool_call_update,
    permission_options,
    options,
);
```

## Story

See `crates/agentx-acp-ui/stories/acp_ui_story.rs` for a visual component showcase.
