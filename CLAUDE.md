# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the `agentx` Agent Studio application, part of the gpui-component workspace. It demonstrates building a full-featured desktop application with GPUI Component, showcasing:

- A dock-based layout system with multiple panels (left, right, bottom, center)
- Custom title bar with menu integration and panel management
- Code editor with LSP support (diagnostics, completion, hover, code actions)
- AI conversation UI components (agent messages, user messages, tool calls, todo lists)
- Task list panel with collapsible sections and mock data loading
- Chat input panel with context controls
- Persistent layout state management with versioning
- Theme support and customization

## Architecture

### Application Structure

- **Main Entry**: `src/main.rs` initializes the app, loads config, and spawns the AgentManager
- **DockWorkspace** (`src/workspace/mod.rs`): The root container managing the dock area, title bar, and layout persistence
- **Panels Module** (`src/panels/`): All panel implementations organized in dedicated directory
- **Core Infrastructure** (`src/core/`): Agent client, event buses, and configuration management
- **Dock System**: Uses `DockArea` from gpui-component for flexible panel layout
- **App Module** (`src/app/`): Contains modular application components (actions, menus, themes, title bar)

### Directory Structure (Post-Refactoring)

```
src/
├── app/                   # Application-level modules
│   ├── actions.rs        # Centralized action definitions
│   ├── app_state.rs      # Global application state
│   ├── menu.rs           # Menu system
│   ├── themes.rs         # Theme management
│   └── title_bar.rs      # Custom title bar
│
├── panels/                # All panel implementations
│   ├── dock_panel.rs     # DockPanel trait and container
│   ├── code_editor.rs    # Code editor with LSP
│   ├── conversation.rs   # Mock conversation panel
│   ├── conversation_acp/ # ACP-enabled conversation (modularized)
│   │   ├── panel.rs      # Main panel logic (1215 lines)
│   │   ├── types.rs      # Reusable types and traits (94 lines)
│   │   └── mod.rs        # Module exports
│   ├── task_list.rs      # Task list panel
│   ├── chat_input.rs     # Chat input panel
│   ├── welcome_panel.rs  # Welcome screen
│   └── settings_window.rs # Settings UI
│
├── core/                  # Core infrastructure
│   ├── agent/            # Agent client management
│   │   ├── client.rs     # AgentManager, AgentHandle
│   │   └── mod.rs
│   ├── event_bus/        # Event distribution system
│   │   ├── session_bus.rs      # Session updates
│   │   ├── permission_bus.rs   # Permission requests
│   │   └── mod.rs
│   ├── services/         # Business logic services (NEW)
│   │   ├── agent_service.rs    # Agent and session management
│   │   ├── message_service.rs  # Message sending and subscription
│   │   └── mod.rs
│   ├── config.rs         # Configuration types
│   └── mod.rs
│
├── components/            # Reusable UI components
│   ├── agent_message.rs
│   ├── user_message.rs
│   ├── tool_call_item.rs
│   ├── agent_todo_list.rs
│   └── ...
│
├── workspace/             # Workspace management
│   ├── mod.rs            # DockWorkspace implementation
│   └── actions.rs        # Workspace action handlers
│
├── schemas/               # Data models
│   ├── conversation_schema.rs
│   └── task_schema.rs
│
├── utils/                 # Utility functions
├── lib.rs                # Library entry point
└── main.rs               # Application entry point
```

### Key Components

1. **DockWorkspace** (`src/workspace/mod.rs`):
   - Manages the main dock area with version-controlled layout persistence
   - Saves layout state to `target/docks-agentx.json` (debug) or `docks-agentx.json` (release)
   - Handles layout loading, saving (debounced by 10 seconds), and version migration
   - Provides actions for adding panels and toggling visibility via dropdown menu in title bar
   - Handles session-based panel creation via `AddSessionPanel` action

2. **Panel System** (`src/panels/dock_panel.rs`):
   - `DockPanelContainer`: Wrapper for panels implementing the `Panel` trait from gpui-component
   - `DockPanel`: Custom trait that panels implement to define title, description, behavior
   - `panel<S: DockPanel>()`: Factory method to create panels of any DockPanel type
   - `panel_for_session()`: Specialized method to create session-specific ConversationPanel instances
   - Panel registration happens in `init()` via `register_panel()` with deserialization from saved state
   - All panels are registered under the name `"DockPanelContainer"` with state determining the actual panel type

3. **App Module** (`src/app/`):
   - **actions.rs**: Centralized action definitions with comprehensive documentation (workspace, task list, UI settings, themes, menus)
   - **app_state.rs**: Global application state, including service layer references
   - **menu.rs**: Application menu setup and handling
   - **themes.rs**: Theme configuration and switching
   - **title_bar.rs**: Custom application title bar component
   - **app_menus.rs**: Menu construction and organization

4. **Service Layer** (`src/core/services/`) - **NEW as of 2025-12-01**:
   - **AgentService** (`agent_service.rs`): Manages agents and their sessions using the Aggregate Root pattern
     - `list_agents()`: List all available agents
     - `create_session(agent_name)`: Create a new session for an agent
     - `get_or_create_session(agent_name)`: Get existing or create new session (recommended)
     - `get_active_session(agent_name)`: Get the active session ID
     - `send_prompt(agent_name, session_id, prompt)`: Send a prompt to an agent
     - `close_session(agent_name)`: Close an agent's session
   - **MessageService** (`message_service.rs`): Handles message sending and event bus interaction
     - `send_user_message(agent_name, message)`: Complete flow - creates/reuses session, publishes to event bus, sends prompt
     - `publish_user_message(session_id, message)`: Publish user message to event bus for immediate UI feedback
     - `subscribe_session_updates(session_id)`: Subscribe to session updates with automatic filtering
   - **Architecture Benefits**:
     - Separates business logic from UI components
     - Eliminates ~150 lines of duplicate code across components
     - Centralizes session management (one active session per agent)
     - Simplifies testing (services can be tested independently)
     - Unified error handling with `anyhow::Result`

5. **Conversation UI Components** (`src/components/`):
   - **AgentMessage** (`agent_message.rs`): Displays AI agent responses with markdown support and streaming capability
   - **UserMessage** (`user_message.rs`): Shows user messages with text and file/resource attachments
   - **ToolCallItem** (`tool_call_item.rs`): Renders tool calls with status badges (pending, running, success, error)
   - **AgentTodoList** (`agent_todo_list.rs`): Interactive todo list with status tracking (pending, in_progress, completed)
   - **ChatInputBox** (`chat_input_box.rs`): Reusable input component with send functionality
   - **TaskListItem** (`task_list_item.rs`): Individual task item display component
   - All components follow a builder pattern for configuration

5. **Panel Implementations** (`src/panels/`):
   - **ConversationPanel** (`conversation.rs`): Mock conversation UI showcasing all message types
   - **ConversationPanel** (`conversation_acp/`): **ACP-enabled conversation panel** with real-time event bus integration
     - Modularized into `panel.rs` (main logic), `types.rs` (reusable helpers), and `mod.rs`
     - Uses **MessageService** for unified message sending (session creation, event publishing, prompt sending)
   - **CodeEditorPanel** (`code_editor/`): High-performance code editor with LSP integration and tree-sitter
     - Modularized into subdirectory with separate modules for LSP providers, storage, and panel logic
   - **ListTaskPanel** (`task_list/`): Task list with collapsible sections
     - Modularized into subdirectory with separate types, delegate, and panel logic
   - **WelcomePanel** (`welcome_panel.rs`): Welcome screen for new sessions

6. **Core Infrastructure** (`src/core/`):
   - **Agent Module** (`agent/client.rs`): `AgentManager` and `AgentHandle` for spawning and managing agent processes
   - **Event Bus** (`event_bus/`): Thread-safe publish-subscribe system
     - `session_bus.rs`: Session update distribution
     - `permission_bus.rs`: Permission request handling
   - **Configuration** (`config.rs`): Agent and application configuration types

### Layout Persistence

The dock layout system uses versioned states:
- Current version: 5 (defined in `MAIN_DOCK_AREA` in `src/workspace/mod.rs`)
- When version mismatch detected, prompts user to reset to default layout
- Layout automatically saved 10 seconds after changes (debounced)
- Layout saved on app quit via `on_app_quit` hook
- State includes panel positions, sizes, active tabs, and visibility

## Development Commands

### Build and Run

```bash
# Run from the agentx directory
cargo run

# Run with info logging
RUST_LOG=info cargo run

# Or from the workspace root (parent directory of agent-studio)
cd ../.. && cargo run --example agentx

# Run the full component gallery (workspace root)
cd ../.. && cargo run
```

### Build Only

```bash
cargo build

# Check for compilation errors without building binaries
cargo check
```

### Development with Performance Profiling (macOS)

```bash
# Enable Metal HUD to see FPS and GPU metrics
MTL_HUD_ENABLED=1 cargo run

# Profile with samply (requires: cargo install samply)
samply record cargo run
```

### Logging

The application uses `tracing` for logging. Control log levels via `RUST_LOG`:

```bash
# Enable trace logging for gpui-component
RUST_LOG=gpui_component=trace cargo run

# Enable debug logging for everything
RUST_LOG=debug cargo run
```

## GPUI Component Integration

### Initialization Pattern

Always call `gpui_component::init(cx)` before using any GPUI Component features. This Agent Studio extends initialization with custom setup:

```rust
pub fn init(cx: &mut App) {
    // Set up logging first
    tracing_subscriber::registry()...

    // Initialize gpui-component (required)
    gpui_component::init(cx);

    // Initialize app-specific state and modules
    AppState::init(cx);
    themes::init(cx);
    editor::init();
    menu::init(cx);

    // Bind keybindings
    cx.bind_keys([...]);

    // Register custom panels
    register_panel(cx, PANEL_NAME, |_, _, info, window, cx| {
        // Panel factory logic
    });
}
```

### Root Element Requirement

The first level element in a window must be a `Root` from gpui-component:

```rust
cx.new(|cx| Root::new(view, window, cx))
```

This provides essential UI layers (sheets, dialogs, notifications). For custom title bars, use `DockRoot` pattern (see `src/lib.rs:167`).

### Creating Custom Panels

To add a new panel type:

1. Create your panel file in `src/panels/` directory

2. Implement the `DockPanel` trait (defined in `src/panels/dock_panel.rs`):
   - `klass()`: Returns the panel type name (auto-implemented from type name)
   - `title()`: Panel display name (static)
   - `description()`: Panel description (static)
   - `new_view()`: Create the panel view entity (returns `Entity<impl Render>`)
   - Optional: `closable()`, `zoomable()`, `title_bg()`, `paddings()`, `on_active()`

3. Add to the match statement in `create_panel_view()` in `src/lib.rs` to handle panel creation

4. Add to default layout in `reset_default_layout()` or `init_default_layout()` in `src/workspace/mod.rs`

5. Export from `src/panels/mod.rs`:
   ```rust
   mod my_panel;
   pub use my_panel::MyPanel;
   ```

Example panel structure:
```rust
// src/panels/my_panel.rs
use gpui::*;
use crate::panels::dock_panel::DockPanel;

pub struct MyPanel {
    focus_handle: FocusHandle,
}

impl DockPanel for MyPanel {
    fn title() -> &'static str { "My Panel" }
    fn description() -> &'static str { "Description here" }
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render> {
        cx.new(|cx| Self::new(window, cx))
    }
}
```

**For large panels**, consider creating a subdirectory:
```
src/panels/my_panel/
├── mod.rs       # Export MyPanel
├── panel.rs     # Main implementation
├── types.rs     # Shared types
└── helpers.rs   # Utility functions
```

Note: The `klass()` method is auto-implemented and will extract "MyPanel" from the full type name.

## Key Concepts

### Dock Placement

Panels can be added to four dock areas: `Center`, `Left`, `Right`, `Bottom`

Dock areas are collapsible (except Center) and support resizing.

### Window Management

- Window bounds are centered and sized to 85% of display (max 1600x1200)
- Minimum window size: 480x320 pixels
- Custom titlebar on macOS/Windows via `TitleBar::title_bar_options()`
- Client decorations on Linux with transparent background

### State Management

- **Global state** via `AppState` for tracking invisible panels
- **Panel state** serialization via `dump()` and deserialization via panel registry
- **Layout state** includes panel positions, sizes, active tabs, and version
- **Mock data** loaded from `mock_tasks.json` for the task list panel

### Message Components Architecture

The conversation UI uses a builder pattern with type-safe components:

- **UserMessage**: `MessageContent::text()` and `MessageContent::resource()` for attachments
- **AgentMessage**: Supports streaming via `add_chunk()`, completed state, thinking indicator
- **ToolCallItem**: Status progression (pending → running → success/error)
- **AgentTodoList**: Entries with priority (high/normal/low) and status tracking

All components are exported from `src/components/mod.rs` for easy reuse.

### Actions System

The application uses a centralized action system defined in `src/app/actions.rs`:

**Action Categories:**
1. **Workspace Actions** - Panel management and dock operations
   - `AddPanel(DockPlacement)`: Add panel to specific dock area
   - `TogglePanelVisible(SharedString)`: Show/hide panels
   - `AddSessionPanel { session_id, placement }`: Create session-specific conversation panels
   - `ToggleDockToggleButton`: Toggle dock button visibility

2. **Task List Actions** - Task and session management
   - `SelectedAgentTask`: Handle task selection
   - `AddSessionToList { session_id, task_name }`: Add new sessions to task list

3. **UI Settings Actions** - Interface customization
   - `SelectScrollbarShow(ScrollbarShow)`: Change scrollbar display mode
   - `SelectLocale(SharedString)`: Switch interface language
   - `SelectFont(usize)`: Change editor/UI font
   - `SelectRadius(usize)`: Adjust component border radius

4. **General Application Actions** - Core app operations
   - `CreateTaskFromWelcome(SharedString)`: Create tasks from welcome panel
   - `About`, `Open`, `Quit`, `CloseWindow`: Standard app operations
   - `ToggleSearch`, `Tab`, `TabPrev`: Navigation
   - `ShowWelcomePanel`, `ShowConversationPanel`: Panel navigation

5. **Theme Actions** - Appearance customization
   - `SwitchTheme(SharedString)`: Change color theme
   - `SwitchThemeMode(ThemeMode)`: Toggle light/dark mode

All actions are fully documented with Chinese and English comments explaining their purpose and parameters.

### Service Layer Usage (NEW - 2025-12-01)

The application now uses a service layer to separate business logic from UI components. Services are accessed through `AppState`:

#### Using MessageService

**Send a message** (automatically creates/reuses session, publishes to event bus, sends prompt):
```rust
// In a UI component
let message_service = AppState::global(cx)
    .message_service()
    .expect("MessageService not initialized");

cx.spawn(async move |_this, _cx| {
    match message_service.send_user_message(&agent_name, message).await {
        Ok(session_id) => {
            log::info!("Message sent successfully to session {}", session_id);
        }
        Err(e) => {
            log::error!("Failed to send message: {}", e);
        }
    }
}).detach();
```

**Send a message to an existing session** (when you need to ensure panel is subscribed first):
```rust
// Recommended pattern for creating panels:
// 1. Get or create session
let agent_service = AppState::global(cx).agent_service().unwrap();
let session_id = agent_service.get_or_create_session(&agent_name).await?;

// 2. Create panel (panel subscribes to session)
let conversation_panel = DockPanelContainer::panel_for_session(session_id.clone(), window, cx);

// 3. Send message to session (panel will receive it)
let message_service = AppState::global(cx).message_service().unwrap();
message_service.send_message_to_session(&agent_name, &session_id, message).await?;
```

**Subscribe to session updates** (automatic filtering):
```rust
let message_service = AppState::global(cx).message_service().unwrap();

// Subscribe to a specific session (automatic filtering)
let mut rx = message_service.subscribe_session_updates(Some(session_id));

// Or subscribe to all sessions
let mut rx = message_service.subscribe_session_updates(None);

cx.spawn(async move |cx| {
    while let Some(update) = rx.recv().await {
        // Handle update (already filtered by session_id if specified)
        // Process the update...
    }
}).detach();
```

#### Using AgentService

**Get or create a session**:
```rust
let agent_service = AppState::global(cx).agent_service().unwrap();

// Recommended: automatically reuses existing active session
let session_id = agent_service.get_or_create_session(&agent_name).await?;

// Or explicitly create a new session
let session_id = agent_service.create_session(&agent_name).await?;
```

**Check for active session**:
```rust
if let Some(session_id) = agent_service.get_active_session(&agent_name) {
    log::info!("Agent {} has active session: {}", agent_name, session_id);
}
```

**Service Initialization**: Services are automatically initialized in `AppState::set_agent_manager()` when the AgentManager is ready.

### Event Bus Architecture (SessionUpdateBus)

The application uses a centralized event bus for real-time message distribution between components:

#### Core Components

1. **SessionUpdateBus** (`src/core/event_bus/session_bus.rs`)
   - Thread-safe publish-subscribe pattern
   - `SessionUpdateEvent`: Contains `session_id` and `SessionUpdate` data
   - `subscribe()`: Register callbacks for events
   - `publish()`: Broadcast events to all subscribers
   - Wrapped in `SessionUpdateBusContainer` (Arc<Mutex<>>) for cross-thread safety

2. **GuiClient** (`src/core/agent/client.rs`)
   - Implements `acp::Client` trait
   - Receives agent notifications via `session_notification()`
   - **Publishes** to session bus when agent sends updates
   - Used by `AgentManager` to bridge agent I/O threads to GPUI main thread

3. **ConversationPanel** (`src/panels/conversation_acp/panel.rs`)
   - **Subscribes** to session bus on initialization
   - Uses `tokio::sync::mpsc::unbounded_channel` for cross-thread communication
   - Real-time rendering: subscription callback → channel → `cx.spawn()` → `cx.update()` → `cx.notify()`
   - Zero-delay updates (no polling required)

4. **ChatInputPanel** (`src/panels/chat_input.rs`)
   - Publishes user messages to session bus immediately
   - Provides instant visual feedback before agent response
   - Uses unique `chunk_id` with UUID to identify local messages

#### Message Flow

```
User Input → ChatInputPanel
  ├─→ Immediate publish to session_bus (user message)
  │    └─→ ConversationPanel displays instantly
  └─→ agent_handle.prompt()
       └─→ Agent processes
            └─→ GuiClient.session_notification()
                 └─→ session_bus.publish()
                      └─→ ConversationPanel subscription
                           └─→ channel.send()
                                └─→ cx.spawn() background task
                                     └─→ cx.update() + cx.notify()
                                          └─→ Real-time render
```

#### Key Implementation Details

- **Cross-thread safety**: Agent I/O threads → GPUI main thread via channels
- **No polling**: Events trigger immediate renders through `cx.notify()`
- **Session isolation**: Each session has a unique ID for message routing
- **Scalability**: Unbounded channel prevents blocking on UI updates

#### Usage Example

```rust
// Subscribe to session bus (in ConversationPanel)
let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
session_bus.subscribe(move |event| {
    let _ = tx.send((*event.update).clone());
});

cx.spawn(|mut cx| async move {
    while let Some(update) = rx.recv().await {
        cx.update(|cx| {
            entity.update(cx, |this, cx| {
                // Process update and trigger render
                cx.notify();
            });
        });
    }
}).detach();

// Publish to session bus (in ChatInputPanel or GuiClient)
let event = SessionUpdateEvent {
    session_id: session_id.clone(),
    update: Arc::new(SessionUpdate::UserMessageChunk(...)),
};
session_bus.publish(event);
```

## Testing

Run the complete story gallery from workspace root:

```bash
cd ../.. && cargo run
```

This displays all GPUI components in a comprehensive gallery interface.

The Agent Studio itself serves as a test bed for:
- Dock layout persistence and restoration
- Panel lifecycle management
- Custom UI components (messages, todos, tool calls)
- LSP integration in code editor
- Theme switching and customization

## Workspace Structure

This Agent Studio is part of a Cargo workspace at `../../`:

- `crates/ui`: Core gpui-component library
- `crates/story`: Story framework and component gallery
- `crates/macros`: Procedural macros for GPUI components
- `crates/assets`: Asset handling and management
- `examples/agentx`: This Agent Studio application
- `examples/hello_world`, `examples/input`, etc.: Other examples
- `crates/ui/src/icon.rs`: IconName definitions for the Icon component
- `crates/story/src/*.rs`: Component examples and documentation

### Important Files in agentx

**Core Entry Points:**
- `src/main.rs`: Application entry, loads config, initializes AgentManager, spawns workspace
- `src/lib.rs`: Core initialization, panel registration, DockRoot, AppState with event buses

**Workspace & Layout:**
- `src/workspace/mod.rs`: DockWorkspace implementation, layout persistence, panel management
- `src/workspace/actions.rs`: Workspace action handlers

**Panels** (all in `src/panels/`):
- `dock_panel.rs`: DockPanel trait, DockPanelContainer, panel factory methods
- `conversation_acp/`: ACP-enabled conversation panel (modularized)
  - `panel.rs`: Main panel implementation (1215 lines)
  - `types.rs`: Reusable helper traits and types (94 lines)
  - `mod.rs`: Module exports
- `code_editor.rs`: Code editor with LSP integration (1052 lines)
- `task_list.rs`: Task list panel with collapsible sections (797 lines)
- `conversation.rs`: Mock conversation panel (for demonstration)
- `chat_input.rs`: Chat input panel, publishes to session bus
- `welcome_panel.rs`: Welcome screen for new sessions
- `settings_window.rs`: Settings UI

**Core Infrastructure** (all in `src/core/`):
- `agent/client.rs`: AgentManager, AgentHandle, GuiClient, PermissionStore
- `event_bus/session_bus.rs`: Session update event bus
- `event_bus/permission_bus.rs`: Permission request event bus
- `config.rs`: Configuration types (AgentProcessConfig, Config, Settings)

**Application Modules** (all in `src/app/`):
- `actions.rs`: **Centralized action definitions** for all app operations
- `app_state.rs`: Global application state with event bus containers
- `menu.rs`: Application menu setup and handlers
- `themes.rs`: Theme configuration and switching
- `title_bar.rs`: Custom application title bar
- `app_menus.rs`: Menu construction

**UI Components** (all in `src/components/`):
- `agent_message.rs`: AI agent message display
- `user_message.rs`: User message display with attachments
- `tool_call_item.rs`: Tool call display with status
- `agent_todo_list.rs`: Todo list component
- `chat_input_box.rs`: Reusable input component
- `task_list_item.rs`: Task item display
- `permission_request.rs`: Permission request UI

**Data & Schemas:**
- `src/schemas/`: Schema definitions for conversations and tasks
- `mock_tasks.json`: Mock task data for the task list panel
- `mock_conversation_acp.json`: Mock conversation data for testing
- `config.json`: Agent configuration file 

## Dependencies

Key dependencies defined in `Cargo.toml`:

### Core Framework
- `gpui = "0.2.2"`: Core GPUI framework for UI rendering
- `gpui-component`: UI component library (workspace member)
- `gpui-component-assets`: Asset integration (workspace member)

### Language Support
- `tree-sitter-navi = "0.2.2"`: Syntax highlighting for the code editor
- `lsp-types`: Language Server Protocol type definitions
- `color-lsp = "0.2.0"`: LSP implementation for color support

### Utilities
- `serde`, `serde_json`: Serialization for layout persistence and mock data
- `rand = "0.8"`: Random number generation for UI demos
- `autocorrect = "2.14.2"`: Text correction utilities
- `chrono = "0.4"`: Date and time handling
- `smol`: Async runtime utilities
- `tracing`, `tracing-subscriber`: Logging and diagnostics

### Workspace Dependencies

All workspace-level dependencies are defined in the root `Cargo.toml` and shared across examples.

### AgentX-specific Dependencies

- `uuid = { version = "1.11", features = ["v4"] }`: For generating unique message chunk IDs
- `tokio = { version = "1.48.0", features = ["rt", "rt-multi-thread", "process"] }`: Async runtime for agent processes
- `tokio-util = { version = "0.7.17", features = ["compat"] }`: Tokio utilities for stream compatibility
- `agent-client-protocol = "0.7.0"`: ACP protocol types for agent communication
- `agent-client-protocol-schema = "0.7.0"`: Schema definitions for session updates

## Event Bus Best Practices

### When to Use the Session Bus

1. **Real-time UI updates** - Agent responses, tool calls, status changes
2. **Cross-component communication** - Chat input → Conversation panel
3. **Session-scoped events** - Messages tied to specific agent sessions

### When NOT to Use the Session Bus

1. **Global UI state** - Use AppState or GPUI global state instead
2. **Synchronous operations** - Direct function calls are simpler
3. **Local component state** - Use Entity state management

### Threading Model

- **Agent I/O threads**: Run agent processes, GuiClient callbacks
- **GPUI main thread**: All UI rendering and entity updates
- **Bridge**: `tokio::sync::mpsc::unbounded_channel` + `cx.spawn()`

### Debugging Tips

Enable debug logging to trace message flow:
```bash
# General info logging
RUST_LOG=info cargo run

# Core infrastructure
RUST_LOG=info,agentx::core::agent=debug cargo run

# Specific panels
RUST_LOG=info,agentx::panels::conversation_acp=debug cargo run

# Event buses
RUST_LOG=info,agentx::core::event_bus=debug cargo run

# Combined debugging
RUST_LOG=info,agentx::core=debug,agentx::panels::conversation_acp=debug cargo run
```

Key log points:
- `"Published user message to session bus"` - ChatInputPanel
- `"Subscribed to session bus with channel-based updates"` - ConversationPanel
- `"Session update sent to channel"` - Subscription callback
- `"Rendered session update"` - Entity update + render

## Coding Style and Conventions

### GPUI Patterns
- Use `cx.new()` for creating entities (not `cx.build()` or direct construction)
- Prefer `Entity<T>` over raw views for state management and lifecycle control
- Use GPUI's reactive patterns: subscriptions, notifications, actions for communication
- Implement `Focusable` trait for interactive panels to support focus management

### UI Conventions
- Mouse cursor: use `default` not `pointer` for buttons (desktop convention, not web)
- Default component size: `md` for most components (consistent with macOS/Windows)
- Use `px()` for pixel values, `rems()` for font-relative sizing
- Apply responsive layout with flexbox: `v_flex()`, `h_flex()`

### Component Design
- Follow existing patterns for component creation and layout
- Use builder pattern for component configuration (e.g., `.label()`, `.icon()`, `.ghost()`)
- Keep components stateless when possible (implement `RenderOnce`)
- For stateful components, use `Entity<T>` and implement `Render`

### Architecture Guidelines
- Separate UI components from business logic
- Use the `DockPanel` trait for all dockable panels
- Keep panel state serializable for layout persistence
- Export reusable components from appropriate module files

### Code Organization

**Post-Refactoring Structure (2025-12-01):**

The codebase has been reorganized for better maintainability:

- **Place reusable UI components in `src/components/`**
- **Keep all panel implementations in `src/panels/`** directory
  - Large panels can be modularized into subdirectories (e.g., `conversation_acp/`)
- **Use `mod.rs` files to re-export public APIs**
- **Group related functionality in submodules:**
  - `src/app/` - Application-level modules (actions, menus, themes, state)
  - `src/core/` - Core infrastructure (agents, event buses, config)
  - `src/workspace/` - Workspace and dock management
- **All GPUI actions should be defined in `src/app/actions.rs`** with proper documentation
- **Use the `DockPanel` trait** for all dockable panels - implement only required methods unless customization needed

**Benefits of Current Structure:**
- ✅ Root directory reduced by 62% (16+ files → 6 files)
- ✅ Clear module boundaries and responsibilities
- ✅ Easier to navigate and maintain
- ✅ Better support for modular refactoring

### Entity Lifecycle Management

**Critical Pattern for Interactive Components:**

When using components like `Collapsible` or any stateful interactive UI elements:

❌ **WRONG** - Creating entities in `render()`:
```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let collapsible = cx.new(|cx| Collapsible::new(...)); // ❌ Dies after render!
    v_flex().child(collapsible)
}
```

✅ **CORRECT** - Creating entities in `new()`:
```rust
struct MyPanel {
    collapsible: Entity<Collapsible>, // ✅ Stored in struct
}

impl MyPanel {
    fn new(window: &mut Window, cx: &mut App) -> Self {
        Self {
            collapsible: cx.new(|cx| Collapsible::new(...)), // ✅ Lives with panel
        }
    }
}

fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex().child(self.collapsible.clone()) // ✅ Reference stored entity
}
```

**Why:** Entities created in `render()` are dropped immediately after the method returns, causing event handlers to fail. Store them in the parent struct to maintain their lifecycle.

**See also:** `docs/collapsible-entity-lifecycle.md` for detailed explanation.

## Configuration

### Agent Configuration

The application loads agent configuration from `config.json` in the project root:

```json
{
  "agent_servers": [
    {
      "name": "agent-name",
      "command": "path/to/agent",
      "args": ["--arg1", "value1"]
    }
  ]
}
```

**AgentProcessConfig Structure:**
- `name`: Agent identifier
- `command`: Executable path or command
- `args`: Command-line arguments (optional)

The config is loaded asynchronously in `main.rs` and used to initialize the `AgentManager`.

### Settings

Settings can be customized via command-line or environment:
- Default config path: `config.json`
- Override with: `--config path/to/config.json`

**Available Settings** (defined in `src/core/config.rs`):
- `config_path`: Path to agent configuration file
- Agent server configurations
- UI preferences (theme, font, locale, scrollbar, radius)

---

## Refactoring History

The AgentX codebase has undergone systematic refactoring to improve code organization and maintainability.

### Stage 1: Directory Reorganization (2025-12-01)

**Objective**: Reduce root directory clutter and establish clear module boundaries.

**Changes**:
- Created `src/panels/` directory for all panel implementations (8 files moved)
- Created `src/core/` directory for infrastructure:
  - `core/agent/` - Agent client management (from `acp_client.rs`)
  - `core/event_bus/` - Event distribution (from `session_bus.rs`, `permission_bus.rs`)
  - `core/config.rs` - Configuration types
- Moved 12 files into organized structure
- Updated all import paths across the codebase

**Results**:
- ✅ Root directory files reduced by 62% (16+ → 6)
- ✅ Zero compilation errors
- ✅ All tests passing
- ✅ Public API maintained backward compatibility

**Documentation**: See `REFACTORING_STAGE1_SUMMARY.md` for detailed breakdown.

### Stage 2: File Modularization (2025-12-01)

**Objective**: Split large files into manageable, focused modules.

**Changes**:
- **ConversationPanel** (1309 lines) → `panels/conversation_acp/` directory:
  - `panel.rs` (1215 lines) - Main panel logic
  - `types.rs` (94 lines) - Reusable helper traits and types
  - `mod.rs` (6 lines) - Module exports
- Extracted reusable code for better testability
- Simplified ResourceInfo implementation

**Results**:
- ✅ Single file size reduced by 7% (1309 → 1215 lines)
- ✅ 94 lines of reusable code extracted
- ✅ Better separation of concerns
- ✅ Zero compilation errors

**Documentation**: See `REFACTORING_STAGE2_SUMMARY.md` for detailed breakdown.

### Future Refactoring Opportunities

**Stage 3 (Optional)**: Further file splitting
- `code_editor.rs` (1052 lines) - Could extract LSP client logic
- `task_list.rs` (797 lines) - Could separate data loading and rendering

**Stage 4 (Optional)**: Service layer introduction
- Introduce `SessionService`, `AgentService`, `StateService`
- Reduce direct dependencies on global `AppState`
- Improve testability through dependency injection

**Documentation**: See `REFACTORING_PLAN.md` for complete roadmap.

---

## Development Best Practices

### Working with Refactored Code

1. **Import Paths**: All core infrastructure is now under `src/core/`:
   ```rust
   use crate::core::agent::{AgentManager, AgentHandle};
   use crate::core::event_bus::{SessionUpdateBusContainer, SessionUpdateEvent};
   use crate::core::config::Config;
   ```

2. **Panel Development**: All panels live in `src/panels/`:
   ```rust
   use crate::panels::dock_panel::DockPanel;
   use crate::panels::conversation_acp::ConversationPanel;
   ```

3. **Modular Panels**: Large panels can be split into subdirectories:
   ```
   src/panels/my_panel/
   ├── mod.rs       # Public exports
   ├── panel.rs     # Main implementation
   ├── types.rs     # Shared types
   └── helpers.rs   # Utility functions
   ```

4. **Event Bus Usage**: Access through `AppState`:
   ```rust
   let session_bus = AppState::global(cx).session_bus.clone();
   let permission_bus = AppState::global(cx).permission_bus.clone();
   ```

### Debugging Tips with New Structure

Enable module-specific logging:
```bash
# Core infrastructure
RUST_LOG=agentx::core::agent=debug cargo run

# Specific panels
RUST_LOG=agentx::panels::conversation_acp=debug cargo run

# Event buses
RUST_LOG=agentx::core::event_bus=debug cargo run
```

