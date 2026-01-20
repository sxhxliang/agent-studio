# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AgentX (version 0.5.0) is a desktop AI agent studio built with Rust and GPUI Component. It provides a dock-based interface for interacting with AI agents via the Agent Client Protocol (ACP).

**Key Technologies:**
- **GPUI**: Zed's GPU-accelerated UI framework
- **gpui-component**: Component library for dock systems, menus, and UI widgets
- **Agent Client Protocol (ACP)**: Protocol for agent communication
- **Tokio**: Async runtime for agent process management

## Build and Development Commands

**Windows** (current platform):
```bash
# Run application
cargo run

# Run with logging
set RUST_LOG=info && cargo run

# Debug specific modules
set RUST_LOG=info,agentx::core::services=debug && cargo run
set RUST_LOG=info,agentx::core::event_bus=debug && cargo run

# Check for compilation errors (fast)
cargo check

# Format code
cargo fmt

# Lint
cargo clippy

# Run tests
cargo test

# Run specific test
cargo test <test_name>

# Release build
cargo build --release
```

**Unix/Linux/macOS:**
```bash
# Run application
cargo run

# Run with logging
RUST_LOG=info cargo run

# Debug specific modules
RUST_LOG=info,agentx::core::services=debug cargo run
RUST_LOG=info,agentx::core::event_bus=debug cargo run

# macOS performance profiling
MTL_HUD_ENABLED=1 cargo run
```

**Workspace Development:**
```bash
# Run from workspace root
cd ../.. && cargo run --example agentx
```

## Architecture Overview

AgentX follows a layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────┐
│  UI Layer (panels/, components/)        │  ← GPUI rendering, user interaction
├─────────────────────────────────────────┤
│  Event Bus (core/event_bus/)            │  ← Pub/sub for cross-thread updates
├─────────────────────────────────────────┤
│  Service Layer (core/services/)         │  ← Business logic
├─────────────────────────────────────────┤
│  Agent Client (core/agent/)             │  ← ACP protocol, process management
└─────────────────────────────────────────┘
```

### Core Architectural Patterns

#### 1. Event Bus System (Cross-Thread Communication)

The event bus enables thread-safe pub/sub between agent threads and UI thread:

**Event Buses** (`src/core/event_bus/`):
- `SessionUpdateBus`: Agent messages, tool calls, thinking updates
- `PermissionBus`: Permission requests from agents
- `WorkspaceBus`: Workspace status changes
- `CodeSelectionBus`: Code selection events for editor integration
- `AgentConfigBus`: Agent configuration changes

**Pattern** (Agent Thread → UI Thread):
```rust
// 1. Subscribe in UI component (runs on GPUI main thread)
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
session_bus.subscribe(move |event| {
    let _ = tx.send((*event.update).clone());
});

cx.spawn(|mut cx| async move {
    while let Some(update) = rx.recv().await {
        cx.update(|cx| {
            entity.update(cx, |this, cx| {
                // Update UI state
                cx.notify();  // Trigger re-render
            });
        });
    }
}).detach();

// 2. Publish from any thread (agent thread, service, etc.)
session_bus.publish(SessionUpdateEvent {
    session_id: session_id.clone(),
    update: Arc::new(SessionUpdate::AgentMessage(...)),
});
```

**Key Features** (Recent Enhancements):
- **Batching**: `BatchedEventCollector` groups rapid events
- **Debouncing**: `Debouncer` prevents excessive updates
- **Filtering**: Subscribe to specific sessions or all sessions
- **Metrics**: `EventBusStats` tracks subscription count and event throughput

#### 2. Service Layer Pattern

All business logic lives in services (`src/core/services/`), accessed via global `AppState`:

**Services:**
- `AgentService`: Manages agent lifecycle and sessions (Aggregate Root)
- `MessageService`: Handles message sending and event bus integration
- `PersistenceService`: Saves/loads session history to JSONL files
- `WorkspaceService`: Manages workspace state and panel visibility
- `AgentConfigService`: Dynamic agent configuration with hot-reloading
- `AiService`: AI-powered features (code comments, etc.)

**Usage Pattern:**
```rust
let message_service = AppState::global(cx).message_service()?;

// Send message (async operation)
cx.spawn(async move |_this, _cx| {
    match message_service.send_user_message(&agent_name, message).await {
        Ok(session_id) => log::info!("Message sent to {}", session_id),
        Err(e) => log::error!("Failed: {}", e),
    }
}).detach();

// Subscribe to session updates with filtering
let mut rx = message_service.subscribe_session_updates(Some(session_id));
cx.spawn(async move |cx| {
    while let Some(update) = rx.recv().await {
        // Handle update
    }
}).detach();
```

#### 3. DockPanel System

All panels implement `DockPanel` trait for consistent docking behavior:

```rust
pub trait DockPanel: 'static + Sized {
    fn title() -> &'static str;
    fn description() -> &'static str;
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render>;

    // Optional customization
    fn closable() -> bool { true }
    fn zoomable() -> bool { true }
    fn paddings() -> Pixels { px(16.) }
}
```

**Panels** (`src/panels/`):
- `ConversationPanel`: Chat interface with ACP agents
- `CodeEditorPanel`: LSP-enabled code editor
- `TaskPanel`: Task/todo management
- `SessionManagerPanel`: Multi-session switching
- `SettingsPanel`: Application settings
- `TerminalPanel`: Embedded terminal
- `ToolCallDetailPanel`: Tool call detail viewer
- `WelcomePanel`: Welcome screen

#### 4. Entity Lifecycle (CRITICAL)

**GPUI Entity Rule**: Entities created in `render()` are dropped after the method returns.

❌ **WRONG**:
```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let widget = cx.new(|cx| Widget::new(...)); // Dies after render!
    v_flex().child(widget)
}
```

✅ **CORRECT**:
```rust
struct MyPanel {
    widget: Entity<Widget>,  // Stored in struct
}

impl MyPanel {
    fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            widget: cx.new(|cx| Widget::new(...)),  // Lives with panel
        }
    }
}

fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex().child(self.widget.clone())  // Reference stored entity
}
```

## Key Subsystems

### Agent Management

**Flow**: `main.rs` → `AgentManager::initialize()` → spawns agent processes → `GuiClient` callbacks → event bus

**Agent Configuration** (`config.json`):
- Located in user data directory (Windows: `%APPDATA%\agentx\config.json`)
- Supports hot-reloading via `ConfigWatcher`
- Command-line override: `agentx --config /path/to/config.json`

**Session Lifecycle**:
```rust
let agent_service = AppState::global(cx).agent_service()?;

// Get or create session (reuses existing)
let session_id = agent_service.get_or_create_session(&agent_name).await?;

// Send message
let message_service = AppState::global(cx).message_service()?;
message_service.send_user_message(&agent_name, message).await?;

// Close session
agent_service.close_session(&agent_name).await?;
```

### Layout Persistence

**Location**:
- Debug: `target/docks-agentx.json`
- Release: `docks-agentx.json`

**Features**:
- Auto-saves layout (debounced 10 seconds)
- Saves on app quit
- Includes panel positions, sizes, active tabs
- Version tracking for migration

### Session Persistence

**Location**: `target/sessions/{session_id}.jsonl` (debug) or `sessions/` (release)

**Format** (one JSON per line):
```jsonl
{"timestamp":"2025-12-10T10:30:45Z","update":{"UserMessage":{"content":"..."}}}
{"timestamp":"2025-12-10T10:30:47Z","update":{"AgentMessage":{"content":"..."}}}
```

**Automatic**: `PersistenceService` subscribes to session bus and saves in real-time.

### Update System

**Auto-update checking** (`src/core/updater/`):
```rust
let manager = UpdateManager::new()?;

match manager.check_for_updates().await {
    UpdateCheckResult::UpdateAvailable(info) => {
        // Download update
        let path = manager.download_update(&info, Some(progress_callback)).await?;
    }
    UpdateCheckResult::UpToDate => {},
    UpdateCheckResult::Error(e) => {},
}
```

## Adding New Panels

### Step 1: Implement DockPanel

Create `src/panels/my_panel.rs`:
```rust
use gpui::*;
use crate::panels::dock_panel::DockPanel;

pub struct MyPanel {
    focus_handle: FocusHandle,
}

impl DockPanel for MyPanel {
    fn title() -> &'static str { "My Panel" }
    fn description() -> &'static str { "Panel description" }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl MyPanel {
    fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for MyPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().child("Panel content")
    }
}

impl Focusable for MyPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
```

### Step 2: Register Panel

In `src/lib.rs`, add to `create_panel_view()`:
```rust
"MyPanel" => {
    let view = MyPanel::new_view(window, cx);
    Some(view.into())
}
```

### Step 3: Export

In `src/panels/mod.rs`:
```rust
mod my_panel;
pub use my_panel::MyPanel;
```

### Step 4: Add to Default Layout (Optional)

In `src/workspace/mod.rs`, update `init_default_layout()`:
```rust
dock_area.push_panel_to_stack(
    DockPanelContainer::panel::<MyPanel>(window, cx).into(),
    DockPlacement::Left,
);
```

## Important Conventions

### Code Organization

Follow this structure for complex panels:
```
src/panels/my_panel/
├── mod.rs           # Module exports
├── panel.rs         # Main panel implementation
├── types.rs         # Panel-specific types
├── components.rs    # UI subcomponents
└── helpers.rs       # Utility functions
```

Examples: `conversation/`, `code_editor/`, `task_panel/`

### Import Organization

Group imports with blank lines:
```rust
// External crates (alphabetical)
use anyhow::Context as _;
use gpui::{App, Context, Entity};

// Local parent module imports
use crate::panels::ConversationPanel;

// Sibling module imports
use super::app_state::AppState;
```

### Error Handling

Use `anyhow` with context:
```rust
let data = load_data()
    .await
    .context("Failed to load data")?;
```

### UI Patterns

- **Sizing**: Use `px()` for pixels, `rems()` for font-relative
- **Layout**: Use `v_flex()`, `h_flex()` with `.gap()`, `.p()` modifiers
- **Mouse cursor**: Use `default` not `pointer` for buttons (desktop convention)
- **Component size**: Default to `md` size

### Async Operations

- Use `tokio` for async runtime
- Spawn with `cx.spawn(...).detach()` for fire-and-forget
- Bridge agent threads to UI with `tokio::sync::mpsc::unbounded_channel` + `cx.spawn()`

## Configuration Files

**User Data Directories**:
- macOS: `~/.agentx/`
- Windows: `%APPDATA%\agentx\`
- Linux: `~/.config/agentx/`

**Files**:
- `config.json`: Agent server configurations
- `docks-agentx.json`: Layout state
- `sessions/{session_id}.jsonl`: Session history
- `state.json`: Application state
- `workspace-config.json`: Workspace configuration

## Internationalization

**System**: `rust-i18n` crate
**Locale files**: `locales/en.yml`, `locales/zh-CN.yml`
**Usage**: `t!("key")` macro for translated strings
**Settings**: Locale selection in Settings panel

## Testing

Tests colocated in `#[cfg(test)] mod tests` blocks:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test implementation
    }
}
```

## Debugging

**Module-specific logging**:
```bash
# Windows
set RUST_LOG=info,agentx::core::services=debug && cargo run
set RUST_LOG=info,agentx::panels::conversation=debug && cargo run

# Unix/Linux/macOS
RUST_LOG=info,agentx::core::services=debug cargo run
```

**Key log messages**:
- `"Published user message to session bus"` - ChatInputBox
- `"Subscribed to session bus"` - ConversationPanel
- `"Session update sent to channel"` - Event bus
- `"Agent spawned successfully"` - AgentManager
- `"Session created"` - AgentService

## Additional Guidelines

**See AGENTS.md** for:
- Detailed code style guidelines
- Git/PR conventions
- Security considerations
- Clippy exceptions
- Testing guidelines

**Performance** (macOS only):
```bash
MTL_HUD_ENABLED=1 cargo run  # Show FPS/GPU metrics
samply record cargo run --release  # Profile with samply
```

## Workspace Context

This project is part of the `gpui-component` workspace at `../gpui-component/`:
- `crates/ui`: Core component library
- `crates/story`: Component gallery
- `crates/macros`: Procedural macros
- `examples/`: Other GPUI examples

Run full component gallery:
```bash
cd ../.. && cargo run
```
