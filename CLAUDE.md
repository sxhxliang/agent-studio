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
- **DockWorkspace** (`src/workspace.rs`): The root container managing the dock area, title bar, and layout persistence
- **Panels**: Individual UI components implementing the `DockPanel` trait and wrapped in `DockPanelContainer`
- **Dock System**: Uses `DockArea` from gpui-component for flexible panel layout
- **App Module** (`src/app/`): Contains modular application components (actions, menus, themes, title bar)

### Key Components

1. **DockWorkspace** (`src/workspace.rs`):
   - Manages the main dock area with version-controlled layout persistence
   - Saves layout state to `target/docks-agentx.json` (debug) or `docks-agentx.json` (release)
   - Handles layout loading, saving (debounced by 10 seconds), and version migration
   - Provides actions for adding panels and toggling visibility via dropdown menu in title bar
   - Handles session-based panel creation via `AddSessionPanel` action

2. **Panel System** (`src/dock_panel.rs`):
   - `DockPanelContainer`: Wrapper for panels implementing the `Panel` trait from gpui-component
   - `DockPanel`: Custom trait that panels implement to define title, description, behavior
   - `panel<S: DockPanel>()`: Factory method to create panels of any DockPanel type
   - `panel_for_session()`: Specialized method to create session-specific ConversationPanelAcp instances
   - Panel registration happens in `init()` via `register_panel()` with deserialization from saved state
   - All panels are registered under the name `"DockPanelContainer"` with state determining the actual panel type

3. **App Module** (`src/app/`):
   - **actions.rs**: Centralized action definitions with comprehensive documentation (workspace, task list, UI settings, themes, menus)
   - **menu.rs**: Application menu setup and handling
   - **themes.rs**: Theme configuration and switching
   - **title_bar.rs**: Custom application title bar component
   - **app_menus.rs**: Menu construction and organization

4. **Conversation UI Components** (`src/components/`):
   - **AgentMessage** (`agent_message.rs`): Displays AI agent responses with markdown support and streaming capability
   - **UserMessage** (`user_message.rs`): Shows user messages with text and file/resource attachments
   - **ToolCallItem** (`tool_call_item.rs`): Renders tool calls with status badges (pending, running, success, error)
   - **AgentTodoList** (`agent_todo_list.rs`): Interactive todo list with status tracking (pending, in_progress, completed)
   - **ChatInputBox** (`chat_input_box.rs`): Reusable input component with send functionality
   - **TaskListItem** (`task_list_item.rs`): Individual task item display component
   - All components follow a builder pattern for configuration

5. **Panel Implementations**:
   - **ConversationPanel** (`src/conversation.rs`): Mock conversation UI showcasing all message types
   - **ConversationPanelAcp** (`src/conversation_acp.rs`): **ACP-enabled conversation panel** with real-time event bus integration
   - **CodeEditorPanel** (`src/code_editor.rs`): High-performance code editor with LSP integration and tree-sitter
   - **ListTaskPanel** (`src/task_list.rs`): Task list with collapsible sections, loads from `mock_tasks.json`
   - **ChatInputPanel** (`src/chat_input.rs`): Chat input panel with agent/mode selectors, publishes to session bus
   - **WelcomePanel** (`src/welcome_panel.rs`): Welcome screen for new sessions

### Layout Persistence

The dock layout system uses versioned states:
- Current version: 5 (defined in `MAIN_DOCK_AREA` in `src/workspace.rs`)
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

1. Implement the `DockPanel` trait (defined in `src/dock_panel.rs`):
   - `klass()`: Returns the panel type name (auto-implemented from type name)
   - `title()`: Panel display name (static)
   - `description()`: Panel description (static)
   - `new_view()`: Create the panel view entity (returns `Entity<impl Render>`)
   - Optional: `closable()`, `zoomable()`, `title_bg()`, `paddings()`, `on_active()`

2. Add to the match statement in `create_panel_view()` in `src/lib.rs` to handle panel creation

3. Add to default layout in `reset_default_layout()` or `init_default_layout()` in `src/workspace.rs`

Example panel structure:
```rust
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

### Event Bus Architecture (SessionUpdateBus)

The application uses a centralized event bus for real-time message distribution between components:

#### Core Components

1. **SessionUpdateBus** (`src/session_bus.rs`)
   - Thread-safe publish-subscribe pattern
   - `SessionUpdateEvent`: Contains `session_id` and `SessionUpdate` data
   - `subscribe()`: Register callbacks for events
   - `publish()`: Broadcast events to all subscribers
   - Wrapped in `SessionUpdateBusContainer` (Arc<Mutex<>>) for cross-thread safety

2. **GuiClient** (`src/gui_client.rs`)
   - Implements `acp::Client` trait
   - Receives agent notifications via `session_notification()` (line 132-164)
   - **Publishes** to session bus when agent sends updates
   - Used by `AgentManager` to bridge agent I/O threads to GPUI main thread

3. **ConversationPanelAcp** (`src/conversation_acp.rs`)
   - **Subscribes** to session bus on initialization
   - Uses `tokio::sync::mpsc::unbounded_channel` for cross-thread communication
   - Real-time rendering: subscription callback → channel → `cx.spawn()` → `cx.update()` → `cx.notify()`
   - Zero-delay updates (no polling required)

4. **ChatInputPanel** (`src/chat_input.rs`)
   - Publishes user messages to session bus immediately (line 309-330)
   - Provides instant visual feedback before agent response
   - Uses unique `chunk_id` with UUID to identify local messages

#### Message Flow

```
User Input → ChatInputPanel
  ├─→ Immediate publish to session_bus (user message)
  │    └─→ ConversationPanelAcp displays instantly
  └─→ agent_handle.prompt()
       └─→ Agent processes
            └─→ GuiClient.session_notification()
                 └─→ session_bus.publish()
                      └─→ ConversationPanelAcp subscription
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
// Subscribe to session bus (in ConversationPanelAcp)
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

- `src/main.rs`: Application entry, loads config, initializes AgentManager, spawns workspace
- `src/workspace.rs`: DockWorkspace implementation, layout persistence, panel management, action handlers
- `src/lib.rs`: Core initialization, panel registration, DockRoot, AppState with session_bus
- `src/dock_panel.rs`: DockPanel trait, DockPanelContainer, panel factory methods
- `src/app/`:
  - `actions.rs`: **Centralized action definitions** for all app operations (workspace, UI, tasks, menus, themes)
  - `menu.rs`: Application menu setup and handlers
  - `themes.rs`: Theme configuration and switching
  - `title_bar.rs`: Custom application title bar
  - `app_menus.rs`: Menu construction
- `src/components/`: Reusable conversation UI components
  - `agent_message.rs`: AI agent message display
  - `user_message.rs`: User message display with attachments
  - `tool_call_item.rs`: Tool call display with status
  - `agent_todo_list.rs`: Todo list component
  - `chat_input_box.rs`: Reusable input component
  - `task_list_item.rs`: Task item display
- `src/code_editor.rs`: Code editor with LSP integration
- `src/task_list.rs`: Task list panel with collapsible sections
- `src/conversation.rs`: Mock conversation panel (for demonstration)
- `src/conversation_acp.rs`: **ACP-enabled conversation panel** with real-time event bus integration
- `src/chat_input.rs`: Chat input panel, publishes user messages to session bus
- `src/welcome_panel.rs`: Welcome screen for new sessions
- `src/session_bus.rs`: Event bus implementation for cross-thread message distribution
- `src/gui_client.rs`: GUI client that publishes agent updates to session bus
- `src/acp_client.rs`: Agent manager and handle, spawns agents with GuiClient
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
RUST_LOG=info,agentx::gui_client=debug,agentx::conversation_acp=debug cargo run
```

Key log points:
- `"Published user message to session bus"` - ChatInputPanel
- `"Subscribed to session bus with channel-based updates"` - ConversationPanelAcp
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
- Place reusable UI components in `src/components/`
- Keep panel implementations in dedicated files at `src/` root
- Use `mod.rs` files to re-export public APIs
- Group related functionality in submodules (e.g., `src/app/` for application-level modules)
- All GPUI actions should be defined in `src/app/actions.rs` with proper documentation
- Use the `DockPanel` trait for all dockable panels - implement only required methods unless customization needed

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

**Available Settings** (defined in `src/config.rs`):
- `config_path`: Path to agent configuration file
- Agent server configurations
- UI preferences (theme, font, locale, scrollbar, radius)

