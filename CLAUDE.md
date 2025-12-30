# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AgentX (version 0.5.0) is a full-featured desktop AI agent studio built with GPUI Component. It demonstrates professional-grade desktop application patterns including:

- **Dock-based layout system** with persistent state and versioning
- **Real-time agent communication** via Agent Client Protocol (ACP)
- **Event-driven architecture** with thread-safe publish-subscribe pattern
- **Service layer** separating business logic from UI components
- **Code editor** with LSP integration and tree-sitter syntax highlighting
- **Multi-session management** with session persistence to JSONL files
- **Auto-update system** with version checking and download capabilities
- **Theme system** with light/dark mode support

The application is part of the gpui-component workspace and serves as both a working agent studio and a comprehensive example of GPUI Component patterns.

## Architecture

### High-Level Structure

```
src/
├── app/                    # Application layer
│   ├── actions.rs         # Centralized action definitions
│   ├── app_state.rs       # Global application state
│   ├── menu.rs            # Menu system
│   ├── themes.rs          # Theme management
│   └── title_bar.rs       # Custom title bar
│
├── core/                   # Core infrastructure
│   ├── agent/             # Agent process management
│   │   └── client.rs      # AgentManager, AgentHandle, GuiClient
│   ├── event_bus/         # Publish-subscribe event distribution
│   │   ├── session_bus.rs           # Session update events
│   │   ├── permission_bus.rs        # Permission request events
│   │   ├── workspace_bus.rs         # Workspace status events
│   │   ├── code_selection_bus.rs    # Code selection events
│   │   └── agent_config_bus.rs      # Agent config change events
│   ├── services/          # Business logic layer
│   │   ├── agent_service.rs         # Agent/session management
│   │   ├── message_service.rs       # Message handling & event publishing
│   │   ├── persistence_service.rs   # JSONL session persistence
│   │   ├── workspace_service.rs     # Workspace state management
│   │   ├── agent_config_service.rs  # Dynamic agent configuration
│   │   └── config_watcher.rs        # File system watching
│   ├── updater/           # Application update system
│   │   ├── checker.rs     # Check for new versions
│   │   ├── downloader.rs  # Download updates
│   │   └── version.rs     # Version parsing and comparison
│   └── config.rs          # Configuration types
│
├── panels/                 # UI panels (dockable)
│   ├── dock_panel.rs      # DockPanel trait & container
│   ├── conversation/      # ACP-enabled conversation UI
│   │   ├── panel.rs       # Main panel implementation
│   │   ├── types.rs       # Shared types
│   │   ├── components.rs  # UI components
│   │   ├── helpers.rs     # Helper functions
│   │   ├── rendered_item.rs # Rendered message items
│   │   └── mod.rs
│   ├── code_editor/       # LSP-enabled code editor
│   │   ├── panel.rs       # Editor panel
│   │   ├── lsp_store.rs   # LSP state management
│   │   ├── lsp_providers.rs # LSP providers
│   │   ├── types.rs       # Editor types
│   │   └── mod.rs
│   ├── task_panel/        # Task management
│   │   ├── panel.rs       # Task panel implementation
│   │   └── mod.rs
│   ├── welcome_panel.rs   # Welcome screen
│   ├── session_manager.rs # Multi-session manager
│   ├── settings_panel.rs  # Application settings
│   └── tool_call_detail_panel.rs  # Tool call details
│
├── components/            # Reusable UI components
│   ├── agent_message.rs   # AI agent messages with markdown
│   ├── user_message.rs    # User messages with attachments
│   ├── tool_call_item.rs  # Tool call visualization
│   ├── agent_todo_list.rs # Todo list component
│   ├── chat_input_box.rs  # Chat input with file upload
│   └── permission_request.rs  # Permission request UI
│
├── workspace/             # Workspace management
│   ├── mod.rs             # DockWorkspace with layout persistence
│   └── actions.rs         # Workspace-specific actions
│
├── schemas/               # Data models
├── utils/                 # Utility functions
├── lib.rs                 # Library entry & initialization
└── main.rs                # Application entry point
```

### Key Design Patterns

#### 1. Service Layer Pattern

Business logic is separated from UI through services in `src/core/services/`. All services are accessed through `AppState`:

```rust
// Access services through global AppState
let message_service = AppState::global(cx).message_service()?;
let agent_service = AppState::global(cx).agent_service()?;
let workspace_service = AppState::global(cx).workspace_service()?;
```

**Services:**
- **AgentService**: Manages agent lifecycle and sessions (Aggregate Root pattern)
- **MessageService**: Handles message sending and event bus integration
- **PersistenceService**: Saves/loads session history to JSONL files
- **WorkspaceService**: Manages workspace state and panel visibility
- **AgentConfigService**: Dynamic agent configuration with hot-reloading
- **ConfigWatcher**: Watches config file for changes

#### 2. Event Bus Architecture

Thread-safe publish-subscribe pattern connects components across threads:

```
User Input → ChatInputBox
  ├─→ Immediate publish to session_bus (user message)
  │    └─→ ConversationPanel displays instantly
  └─→ agent_handle.prompt()
       └─→ Agent processes (separate thread)
            └─→ GuiClient.session_notification()
                 └─→ session_bus.publish()
                      └─→ ConversationPanel subscription
                           └─→ tokio::channel → cx.spawn() → cx.update()
                                └─→ Real-time UI update
```

**Event Buses:**
- **SessionUpdateBus**: Agent messages, tool calls, thinking updates
- **PermissionBus**: Permission requests from agents
- **WorkspaceBus**: Workspace status changes
- **CodeSelectionBus**: Code selection events for editor integration
- **AgentConfigBus**: Agent configuration changes

**Cross-thread Communication:**
```rust
// Subscribe to events (in UI component)
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
session_bus.subscribe(move |event| {
    let _ = tx.send((*event.update).clone());
});

cx.spawn(|mut cx| async move {
    while let Some(update) = rx.recv().await {
        cx.update(|cx| {
            entity.update(cx, |this, cx| {
                // Process update
                cx.notify();  // Trigger re-render
            });
        });
    }
}).detach();

// Publish events (from any thread)
session_bus.publish(SessionUpdateEvent {
    session_id: session_id.clone(),
    update: Arc::new(SessionUpdate::AgentMessage(...)),
});
```

#### 3. DockPanel System

All panels implement `DockPanel` trait for consistent behavior:

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

Panels are wrapped in `DockPanelContainer` and registered for serialization/deserialization.

#### 4. Message Persistence

Session updates are automatically persisted to `target/sessions/{session_id}.jsonl`:

```jsonl
{"timestamp":"2025-12-10T10:30:45Z","update":{"UserMessage":{"content":"..."}}}
{"timestamp":"2025-12-10T10:30:47Z","update":{"AgentMessage":{"content":"..."}}}
```

Persistence is handled by `PersistenceService` which subscribes to the session bus.

## Development Commands

### Build and Run

**Windows (current platform):**
```bash
# Run application
cargo run

# Run with logging
set RUST_LOG=info && cargo run

# Run from workspace root
cd ..\.. && cargo run --example agentx

# Build without running
cargo build

# Check for compilation errors (fast)
cargo check

# Release build (optimized)
cargo build --release
```

**Unix/Linux/macOS:**
```bash
# Run application
cargo run

# Run with logging
RUST_LOG=info cargo run

# Run from workspace root
cd ../.. && cargo run --example agentx

# Build without running
cargo build

# Check for compilation errors (fast)
cargo check

# Release build (optimized)
cargo build --release
```

### Development with Logging

Control log verbosity with `RUST_LOG` environment variable:

**Windows:**
```bash
# General info logging
set RUST_LOG=info && cargo run

# Debug specific modules
set RUST_LOG=info,agentx::core::services=debug && cargo run
set RUST_LOG=info,agentx::panels::conversation=debug && cargo run

# Debug event buses
set RUST_LOG=info,agentx::core::event_bus=debug && cargo run

# Combined debugging (services + panels)
set RUST_LOG=info,agentx::core=debug,agentx::panels=debug && cargo run

# Trace all updates
set RUST_LOG=trace && cargo run
```

**Unix/Linux/macOS:**
```bash
# General info logging
RUST_LOG=info cargo run

# Debug specific modules
RUST_LOG=info,agentx::core::services=debug cargo run
RUST_LOG=info,agentx::panels::conversation=debug cargo run

# Debug event buses
RUST_LOG=info,agentx::core::event_bus=debug cargo run

# Combined debugging (services + panels)
RUST_LOG=info,agentx::core=debug,agentx::panels=debug cargo run

# Trace all updates
RUST_LOG=trace cargo run
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with logging
RUST_LOG=debug cargo test

# Run specific test
cargo test test_name
```

### Performance Profiling (macOS)

```bash
# Enable Metal HUD for FPS/GPU metrics
MTL_HUD_ENABLED=1 cargo run

# Profile with samply (requires: cargo install samply)
samply record cargo run --release
```

### Code Quality

```bash
# Lint with clippy
cargo clippy

# Format code
cargo fmt

# Generate documentation
cargo doc --open
```

## GPUI Component Integration

### Initialization Pattern

AgentX extends GPUI Component initialization in `src/lib.rs`:

```rust
pub fn init(cx: &mut App) {
    // 1. Set up logging
    tracing_subscriber::registry()...

    // 2. Initialize gpui-component (required)
    gpui_component::init(cx);

    // 3. Initialize app-specific state
    AppState::init(cx);        // Global state with event buses
    themes::init(cx);          // Theme system
    menu::init(cx);            // Menu system

    // 4. Register custom panels
    register_panel(cx, PANEL_NAME, |_, _, info, window, cx| {
        // Panel factory based on saved state
    });

    // 5. Bind keybindings
    cx.bind_keys([...]);
}
```

### Root Element Requirement

The first element in a window must be `Root` from gpui-component:

```rust
cx.new(|cx| Root::new(view, window, cx))
```

This provides essential UI layers (sheets, dialogs, notifications). For custom title bars, use `DockRoot` pattern.

### Window Management

Windows are created with consistent sizing:

```rust
// Window defaults: 85% of display, max 1600x1200, min 480x320
create_new_window("Title", |window, cx| {
    // Create view
}, cx);
```

## Key Concepts

### Using the Service Layer

#### MessageService

**Send a message** (creates session if needed, publishes to event bus, sends to agent):

```rust
let message_service = AppState::global(cx).message_service()?;

cx.spawn(async move |_this, _cx| {
    match message_service.send_user_message(&agent_name, message).await {
        Ok(session_id) => log::info!("Message sent to {}", session_id),
        Err(e) => log::error!("Failed: {}", e),
    }
}).detach();
```

**Subscribe to session updates** (with automatic filtering):

```rust
let message_service = AppState::global(cx).message_service()?;

// Subscribe to specific session only
let mut rx = message_service.subscribe_session_updates(Some(session_id));

// Or subscribe to all sessions
let mut rx = message_service.subscribe_session_updates(None);

cx.spawn(async move |cx| {
    while let Some(update) = rx.recv().await {
        // Handle filtered update
    }
}).detach();
```

#### AgentService

**Get or create a session**:

```rust
let agent_service = AppState::global(cx).agent_service()?;

// Recommended: reuses existing active session
let session_id = agent_service.get_or_create_session(&agent_name).await?;

// Or explicitly create new session
let session_id = agent_service.create_session(&agent_name).await?;
```

**List all agents**:

```rust
let agents = agent_service.list_agents().await;
```

**Close a session**:

```rust
agent_service.close_session(&agent_name).await?;
```

#### PersistenceService

Session persistence is automatic via `MessageService.init_persistence()`. To load history:

```rust
let persistence_service = AppState::global(cx).persistence_service()?;
let messages = persistence_service.load_session_history(&session_id).await?;
```

### Layout Persistence

The dock layout is automatically saved to:
- `target/docks-agentx.json` (debug builds)
- `docks-agentx.json` (release builds)

Layout includes:
- Panel positions and sizes
- Active tabs
- Dock visibility
- Version number (for migration)

Saving is debounced by 10 seconds and also triggered on app quit.

### Actions System

All actions are centralized in `src/app/actions.rs`. Categories include:

**Workspace Actions:**
- `AddPanel(DockPlacement)`: Add panel to dock
- `TogglePanelVisible(SharedString)`: Show/hide panels
- `AddSessionPanel { session_id, placement }`: Create conversation panel

**Session Actions:**
- `NewSessionConversationPanel`: Create new session
- `SendMessageToSession { session_id, message }`: Send message
- `CancelSession`: Cancel active session

**Agent Management:**
- `AddAgent`, `UpdateAgent`, `RemoveAgent`: Manage agent configs
- `RestartAgent`: Restart agent process
- `ReloadAgentConfig`: Hot-reload configuration

**UI Actions:**
- `SelectFont`, `SelectLocale`, `SelectRadius`, `SelectScrollbarShow`
- `ShowWelcomePanel`, `ShowConversationPanel`, `ShowToolCallDetail`

### Update System

AgentX includes automatic update checking:

```rust
use crate::core::updater::{UpdateManager, UpdateCheckResult};

let manager = UpdateManager::new()?;

// Check for updates
match manager.check_for_updates().await {
    UpdateCheckResult::UpdateAvailable(info) => {
        println!("New version: {}", info.version);

        // Download update
        let path = manager.download_update(&info, Some(progress_callback)).await?;
    }
    UpdateCheckResult::UpToDate => println!("Already up to date"),
    UpdateCheckResult::Error(e) => eprintln!("Check failed: {}", e),
}
```

## Creating Custom Panels

### Step 1: Implement DockPanel Trait

Create a new file in `src/panels/`:

```rust
// src/panels/my_panel.rs
use gpui::*;
use crate::panels::dock_panel::DockPanel;

pub struct MyPanel {
    focus_handle: FocusHandle,
    // ... your state
}

impl DockPanel for MyPanel {
    fn title() -> &'static str { "My Panel" }

    fn description() -> &'static str { "Description for panel dropdown" }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }

    // Optional overrides
    fn closable() -> bool { true }
    fn zoomable() -> bool { true }
    fn paddings() -> Pixels { px(12.) }
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
        v_flex()
            .size_full()
            .child("My Panel Content")
    }
}

impl Focusable for MyPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
```

### Step 2: Register Panel

In `src/lib.rs`, add to `create_panel_view()` match statement:

```rust
"MyPanel" => {
    let view = MyPanel::new_view(window, cx);
    Some(view.into())
}
```

### Step 3: Export from Module

In `src/panels/mod.rs`:

```rust
mod my_panel;
pub use my_panel::MyPanel;
```

In `src/lib.rs` exports:

```rust
pub use panels::MyPanel;
```

### Step 4: Add to Default Layout (Optional)

In `src/workspace/mod.rs`, add to `init_default_layout()`:

```rust
dock_area.push_panel_to_stack(
    DockPanelContainer::panel::<MyPanel>(window, cx).into(),
    DockPlacement::Left,
);
```

### For Large Panels: Use Subdirectory Structure

The codebase follows this pattern for complex panels:

**Conversation Panel** (`src/panels/conversation/`):
```
src/panels/conversation/
├── mod.rs           # Module exports
├── panel.rs         # ConversationPanel implementation
├── types.rs         # SessionUpdate, Message types
├── components.rs    # UI subcomponents
├── helpers.rs       # Utility functions
└── rendered_item.rs # Message rendering logic
```

**Code Editor Panel** (`src/panels/code_editor/`):
```
src/panels/code_editor/
├── mod.rs           # Module exports
├── panel.rs         # CodeEditorPanel implementation
├── lsp_store.rs     # LSP state management
├── lsp_providers.rs # LSP provider implementations
└── types.rs         # Editor-specific types
```

**Task Panel** (`src/panels/task_panel/`):
```
src/panels/task_panel/
├── mod.rs           # Module exports
└── panel.rs         # TaskPanel implementation
```

## Coding Conventions

### GPUI Patterns

**Entity Creation:**
- Use `cx.new()` for creating entities (not `cx.build()`)
- Prefer `Entity<T>` over raw views for state management
- Use GPUI's reactive patterns: subscriptions, notifications, actions

**Entity Lifecycle (Critical):**

❌ **WRONG** - Creating entities in `render()`:
```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let widget = cx.new(|cx| Widget::new(...)); // Dies after render!
    v_flex().child(widget)
}
```

✅ **CORRECT** - Creating entities in constructor:
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

**Why**: Entities created in `render()` are dropped immediately after the method returns, breaking event handlers.

### UI Conventions

- **Mouse cursor**: Use `default` not `pointer` for buttons (desktop convention)
- **Component size**: Default to `md` size (consistent with macOS/Windows)
- **Sizing**: Use `px()` for pixels, `rems()` for font-relative sizing
- **Layout**: Use flexbox: `v_flex()`, `h_flex()` with `.gap()`, `.p()` modifiers

### Component Design

- Follow builder pattern: `.label()`, `.icon()`, `.ghost()`, `.on_click()`
- Keep components stateless when possible (implement `RenderOnce`)
- For stateful components, use `Entity<T>` and implement `Render`
- Implement `Focusable` for interactive panels

### Code Organization

- **Reusable UI components** → `src/components/`
- **Dockable panels** → `src/panels/`
- **Business logic** → `src/core/services/`
- **Actions** → `src/app/actions.rs` (centralized)
- **Event buses** → `src/core/event_bus/`

Large files should be modularized into subdirectories with focused modules.

## Configuration

### Agent Configuration

Create `config.json` in the project root:

```json
{
  "agent_servers": [
    {
      "name": "my-agent",
      "command": "/path/to/agent/executable",
      "args": ["--arg1", "value1"]
    }
  ]
}
```

Configuration supports hot-reloading via `ConfigWatcher` - changes are automatically detected and agents are restarted.

### Settings

Override default settings via command-line:

```bash
cargo run -- --config /path/to/config.json
```

**Available Settings** (defined in `src/core/config.rs`):
- `config_path`: Path to agent configuration file
- Agent server configurations
- UI preferences (accessed via settings panel)

### Data Storage

Runtime data is stored in the `target/` directory (debug) or project root (release):

- `docks-agentx.json`: Layout state
- `sessions/{session_id}.jsonl`: Session history (one JSON per line)
- `state.json`: Application state
- `workspace-config.json`: Workspace configuration

## Workspace Context

This Agent Studio is part of a Cargo workspace at `../../`:

- `crates/ui`: Core gpui-component library
- `crates/story`: Story framework and component gallery
- `crates/macros`: Procedural macros
- `crates/assets`: Asset handling
- `examples/agentx`: This Agent Studio application
- `examples/hello_world`, `examples/input`, etc.: Other examples

Run the complete component gallery from workspace root:

**Windows:**
```bash
cd ..\.. && cargo run
```

**Unix/Linux/macOS:**
```bash
cd ../.. && cargo run
```

## Platform-Specific Considerations

### Windows
- Use `set RUST_LOG=... && cargo run` for environment variables
- Path separators use backslash (`\`) but Rust handles both
- Agent executables should have `.exe` extension in config.json
- Layout files stored in `target\` directory (debug builds)

### macOS
- MTL_HUD_ENABLED available for GPU performance metrics
- Performance profiling with `samply` works best on macOS
- Use forward slash (`/`) for paths
- Metal backend provides optimal performance

### Linux
- Vulkan backend used for rendering
- Ensure graphics drivers are up to date
- May require additional system dependencies for UI rendering

## Internationalization (i18n)

AgentX uses the `rust-i18n` crate for multilingual support:

- Translation files are managed in `src/i18n.rs`
- Locale selection available in Settings panel
- UI components support dynamic language switching
- Use `t!("key")` macro for translated strings in code

## Asset Management

Assets are embedded at compile time using `rust-embed`:

**Location**: `src/assets.rs` and `assets/` directory

**Structure**:
```
assets/
├── icons/        # General UI icons (SVG)
├── icons2/       # Additional icon set (SVG)
└── logo/         # Application logo assets (SVG)
```

**Usage**:
```rust
use crate::Assets;

// Initialize asset source in lib.rs
cx.asset_source().clone().add_source(
    std::sync::Arc::new(Box::new(Assets))
);

// Load assets via IconNamed trait
Icon::Claude.named()  // Returns SharedString for icon path
```

Assets are loaded at runtime from the embedded binary, eliminating the need for external asset files in distribution.

## Dependencies

Key dependencies from `Cargo.toml`:

**Core Framework:**
- `gpui = "0.2.2"`: Core GPUI framework (Git dependency from Zed)
- `gpui-component = "0.5.0"`: UI component library (Git dependency from LongBridge)

**Agent Communication:**
- `agent-client-protocol = "0.9.0"`: ACP protocol
- `tokio = "1.48.0"`: Async runtime (with process, fs, io-util)
- `tokio-util = "0.7.17"`: Stream compatibility

**HTTP Client (Embedded in `src/reqwest_client/`):**
- Custom reqwest wrapper with TLS support
- Platform-specific certificate verification
- Used by the auto-update system

**Language Support:**
- `tree-sitter-navi = "0.2.2"`: Syntax highlighting
- `lsp-types = "0.97.0"`: Language Server Protocol
- `color-lsp = "0.2.0"`: Color support

**Utilities:**
- `serde`, `serde_json`: Serialization
- `uuid = "1.11"`: Unique IDs
- `chrono = "0.4"`: Date/time
- `tracing`, `tracing-subscriber`: Logging
- `reqwest_client`: HTTP requests (for updates)

## Important Patterns

### Threading Model

- **Agent I/O threads**: Run agent processes, GuiClient callbacks
- **GPUI main thread**: All UI rendering and entity updates
- **Bridge**: `tokio::sync::mpsc::unbounded_channel` + `cx.spawn()`

Never call GPUI APIs directly from agent threads. Always use channels + `cx.spawn()`.

### Error Handling

Services use `anyhow::Result` for unified error handling:

```rust
use anyhow::{Context, Result};

async fn my_operation() -> Result<()> {
    let data = load_data()
        .await
        .context("Failed to load data")?;
    Ok(())
}
```

### State Management

- **Global state**: Use `AppState::global(cx)` for cross-component state
- **Panel state**: Store in panel struct, serializable for persistence
- **Local UI state**: Use GPUI's reactive state management

### When to Use Event Bus vs Direct Calls

**Use Event Bus for:**
- Real-time UI updates from agent threads
- Cross-component communication without tight coupling
- Session-scoped events
- Broadcasts to multiple subscribers

**Use Direct Calls for:**
- Synchronous operations
- Single-target communication
- Local component interactions

### Component Communication Patterns

**Pattern 1: Parent → Child (Props)**
```rust
// Pass data down via constructor or method calls
let child = cx.new(|cx| ChildComponent::new(data, cx));
```

**Pattern 2: Child → Parent (Callbacks)**
```rust
// Use callbacks for upward communication
pub struct ChatInputBox {
    on_send: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}

// Parent provides callback
ChatInputBox::new(id, cx)
    .on_send(|_event, _window, cx| {
        // Handle send action
    })
```

**Pattern 3: Cross-Component (Event Bus)**
```rust
// Subscribe in one component
let session_bus = AppState::global(cx).session_bus.clone();
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
session_bus.lock().unwrap().subscribe(move |event| {
    let _ = tx.send((*event.update).clone());
});

// Publish from another component/thread
session_bus.lock().unwrap().publish(SessionUpdateEvent { ... });
```

**Pattern 4: Global State (AppState)**
```rust
// Access shared state from anywhere
let app_state = AppState::global(cx);
let agent_service = app_state.agent_service()?;
```

## Debugging Tips

### Enable Detailed Logging

**Windows:**
```bash
# Session bus events
set RUST_LOG=info,agentx::core::event_bus::session_bus=debug && cargo run

# Service layer
set RUST_LOG=info,agentx::core::services=debug && cargo run

# Specific panel
set RUST_LOG=info,agentx::panels::conversation=debug && cargo run

# Everything in core
set RUST_LOG=info,agentx::core=trace && cargo run
```

**Unix/Linux/macOS:**
```bash
# Session bus events
RUST_LOG=info,agentx::core::event_bus::session_bus=debug cargo run

# Service layer
RUST_LOG=info,agentx::core::services=debug cargo run

# Specific panel
RUST_LOG=info,agentx::panels::conversation=debug cargo run

# Everything in core
RUST_LOG=info,agentx::core=trace cargo run
```

### Key Log Messages to Look For

- `"Published user message to session bus"` - ChatInputBox
- `"Subscribed to session bus"` - ConversationPanel
- `"Session update sent to channel"` - Event bus callback
- `"Rendered session update"` - Panel re-render
- `"Agent spawned successfully"` - AgentManager
- `"Session created"` - AgentService
- `"Persisted message to JSONL"` - PersistenceService

### Common Issues

**Panel not updating**: Check event bus subscription is active and `cx.notify()` is called.

**Entity event handlers not working**: Ensure entities are stored in struct, not created in `render()`.

**Agent not responding**: Check agent process logs, verify config.json paths and permissions.

**Layout not saving**: Ensure target directory has write permissions and check for serialization errors.
