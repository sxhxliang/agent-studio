# AgentX - AI Agent Studio

A full-featured desktop application built with [GPUI Component](https://github.com/longbridge/gpui-component), showcasing a modern dock-based interface for interacting with AI agents. AgentX demonstrates professional-grade UI patterns, real-time event-driven architecture, and comprehensive agent communication capabilities.

![AgentX Screenshot](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)
![Rust Version](https://img.shields.io/badge/Rust-1.85%2B%20(Edition%202024)-orange)
![License](https://img.shields.io/badge/License-Apache--2.0-green)
![Version](https://img.shields.io/badge/Version-0.5.0-brightgreen)

## 🎉 What's New in v0.5.0

### Performance Improvements
- **O(1) Tool Call Lookups**: Replaced O(n) list traversal with HashMap indexing for instant tool call updates
- **Event Batching**: Optimized event bus with debouncing to reduce unnecessary UI re-renders
- **Streaming Optimization**: Efficient message merging without full list traversal on every chunk

### Enhanced UI/UX
- **Comprehensive Settings**: Multi-page settings panel (General, Agent, MCP, Model, Command, Network, Update, About)
- **System Tray Integration**: Background operation with system tray icon and menu
- **Terminal Panel**: Integrated terminal using gpui_term
- **Improved Tooltips**: Added tooltips for code editor features with i18n support
- **Status Messages**: Better status indicators across conversation and task panels

### Developer Experience
- **Refactored Services**: Improved message service with metadata support
- **Better LSP Integration**: Enhanced code editor LSP capabilities
- **Unified Panel Logic**: Consistent panel construction for loading and manual addition
- **Optimized Persistence**: Unified file storage and workspace parameter handling

### Internationalization
- Full i18n support with English and Simplified Chinese translations
- Localized UI text keys across all panels and components

## ✨ Features

### 🎨 **Modern UI Architecture**
- **Dock-based Layout System**: Flexible panel management with four dock areas (Center, Left, Right, Bottom)
- **Persistent Layout State**: Automatic layout saving/loading with versioning support
- **Custom Title Bar**: Native-looking custom window controls on all platforms
- **Theme System**: Multiple color themes with light/dark mode support
- **System Tray Integration**: Background operation with system tray icon and menu

### 💬 **AI Agent Integration**
- **Real-time Communication**: Event-driven architecture using optimized batching pattern
- **Session Management**: Multi-session support with session-scoped message routing
- **Agent Client Protocol (ACP)**: Full implementation of agent communication protocol v0.9.2
- **Permission Handling**: Interactive permission request workflow
- **Dynamic Configuration**: Hot-reload agent configuration without restart

### 🛠️ **Development Tools**
- **Code Editor**: Integrated editor with LSP support (diagnostics, completion, hover, code actions)
- **Terminal Panel**: Integrated terminal with GPUI Term
- **Tree-sitter Integration**: Syntax highlighting for multiple languages
- **Task Management**: Collapsible task list with status tracking
- **Conversation UI**: Rich message components with markdown support and streaming
- **Diff Summary**: File change statistics and visualization with collapsible view

### 🏗️ **Architecture Highlights**
- **Service Layer Pattern**: Separation of business logic from UI components
- **Event Bus System**: Thread-safe message distribution with batching optimization
- **Modular Design**: Clean separation of concerns with well-organized directory structure
- **Performance Optimization**: O(1) tool call lookups and optimized message merging
- **Internationalization**: Multi-language support (English, Chinese Simplified)

## 🚀 Quick Start

### Prerequisites

- **Rust**: 1.85 or later (Edition 2024) - install from [rustup.rs](https://rustup.rs/)
- **Git**: For cloning the repository
- **Platform-specific requirements**:
  - **macOS**: Xcode Command Line Tools
  - **Linux**: Build essentials, X11/Wayland development libraries
  - **Windows**: Visual Studio Build Tools or MSVC

### Installation

This project is part of the **gpui-component workspace**. You can build it in two ways:

**Option 1: From workspace root (recommended):**
```bash
# Clone the workspace
git clone https://github.com/longbridge/gpui-component.git
cd gpui-component

# Run AgentX from workspace
cargo run --example agentx

# Or with logging
RUST_LOG=info cargo run --example agentx
```

**Option 2: As standalone project:**
```bash
# Navigate to the agent-studio directory
cd examples/agent-studio

# Run directly (uses local path dependency)
cargo run

# Or with logging
RUST_LOG=info cargo run
```

### Build Options

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Check for compilation errors
cargo check
```

**Platform-specific Commands:**

**Windows:**
```bash
# Run with logging
set RUST_LOG=info && cargo run

# Debug specific modules
set RUST_LOG=info,agentx::core::services=debug && cargo run
```

**Unix/Linux/macOS:**
```bash
# Run with logging
RUST_LOG=info cargo run

# Debug specific modules
RUST_LOG=info,agentx::core::services=debug cargo run
```

**Note**: This project uses Git dependencies for GPUI and gpui-component. The first build may take some time as Cargo fetches and compiles dependencies.

### Internationalization

AgentX supports multiple languages out of the box:
- **English** (en)
- **Simplified Chinese** (zh-CN)

Language can be changed in Settings → General → Locale. Translations are managed using the `rust-i18n` crate with locale files in the workspace.

### 📖 Usage

### First Launch

On first launch, AgentX displays a welcome panel. You can:
1. Create a new conversation with an AI agent
2. Explore the interface and dock layout
3. Customize themes and settings

### Diff Summary Feature

The application now includes a comprehensive diff summary feature:
- **Automatic Tracking**: All file changes during a session are automatically tracked
- **Visual Summary**: Changes are displayed with file counts, additions (+) and deletions (-)
- **Click Navigation**: Click on files in the summary to jump directly to the change details
- **Context Collapsing**: Large unchanged code sections are collapsed to focus on actual changes
- **New File Detection**: New files are clearly marked with "NEW" indicators

### Interface Layout

```
┌─────────────────────────────────────────────────────────┐
│  Title Bar (Custom) - Menu, Panel Controls              │
├───────────┬─────────────────────────┬───────────────────┤
│           │                         │                   │
│   Left    │        Center           │      Right        │
│   Dock    │      Dock Area          │      Dock         │
│           │  (Conversation/Editor)  │  (Tasks/Tools)    │
│           │                         │                   │
├───────────┴─────────────────────────┴───────────────────┤
│              Bottom Dock (Chat Input)                    │
└─────────────────────────────────────────────────────────┘
```

### Key Actions

- **Add Panel**: Click the panel dropdown in title bar → Select panel type → Choose placement
- **Send Message**: Type in chat input → Press Enter or click Send
- **Switch Theme**: Menu Bar → Themes → Select theme/mode
- **Toggle Panels**: Use View menu or panel visibility toggles
- **View Diff Summary**: File changes automatically shown in conversation panel with collapsible view
- **Expand/Collapse Diffs**: Click on diff sections to show/hide unchanged code context
- **System Tray**: Minimize to system tray for background operation
- **Multi-Session**: Switch between multiple agent sessions using the session manager

### Keyboard Shortcuts

- `Tab` / `Shift+Tab`: Navigate between panels
- `Ctrl+Q` / `Cmd+Q`: Quit application (or minimize to tray if enabled)
- `Ctrl+N` / `Cmd+N`: New conversation session
- `Ctrl+,` / `Cmd+,`: Open settings
- Additional shortcuts available in Menu Bar

## ⚙️ Configuration

### Agent Configuration

Create a `config.json` file in the project root to configure AI agents:

```json
{
  "agent_servers": [
    {
      "name": "my-agent",
      "command": "/path/to/agent/executable",
      "args": ["--arg1", "value1", "--arg2"]
    }
  ]
}
```

**Configuration Fields:**
- `name`: Agent identifier (used in UI)
- `command`: Path to agent executable or command
- `args`: Optional command-line arguments (array)

### Settings

Customize the application through the Settings window (Menu → Settings):

**General Settings:**
- **Theme**: Color scheme and light/dark mode
- **Font**: Editor and UI font selection
- **Locale**: Interface language (English, 简体中文)
- **Scrollbar**: Display mode (auto/always/never)
- **Border Radius**: Component corner rounding

**Agent Settings:**
- Configure agent executables and parameters
- View and modify agent configurations
- Manage multiple agent profiles

**MCP Settings:**
- Model Context Protocol (MCP) server configuration
- MCP capabilities and integrations

**Model Settings:**
- AI model configuration
- Model provider settings
- API key management

**Command Settings:**
- Custom command configuration
- Command shortcuts and bindings

**Network Settings:**
- Proxy configuration
- Network timeout settings
- SSL/TLS options

**Update Settings:**
- Auto-update preferences
- Update channel selection
- Version checking interval

## 🏛️ Architecture

### Directory Structure

```
src/
├── app/                      # Application layer
│   ├── actions.rs           # Centralized action definitions
│   ├── app_state.rs         # Global application state
│   ├── app_menus.rs         # Application menu definitions
│   ├── menu.rs              # Menu system
│   ├── system_tray.rs       # System tray integration
│   ├── themes.rs            # Theme management
│   ├── key_binding.rs       # Keyboard shortcuts
│   └── title_bar.rs         # Custom title bar
│
├── panels/                   # All panel implementations
│   ├── dock_panel.rs        # DockPanel trait and container
│   ├── conversation/        # ACP-enabled conversation panel
│   │   ├── panel.rs         # Main panel logic
│   │   ├── types.rs         # Reusable types
│   │   └── ...
│   ├── code_editor/         # Code editor with LSP
│   ├── task_panel/          # Task management panel
│   ├── terminal_panel.rs    # Terminal integration (gpui_term)
│   ├── welcome_panel.rs     # Welcome screen
│   ├── session_manager.rs   # Multi-session management
│   ├── settings_panel/      # Settings UI (multiple pages)
│   │   ├── panel.rs         # Settings window
│   │   ├── general_page.rs  # General settings
│   │   ├── agent_page.rs    # Agent configuration
│   │   ├── mcp_page.rs      # MCP server settings
│   │   ├── model_page.rs    # Model configuration
│   │   ├── command_page.rs  # Command settings
│   │   ├── network_page.rs  # Network settings
│   │   ├── update_page.rs   # Update settings
│   │   └── about_page.rs    # About information
│   └── tool_call_detail_panel.rs  # Tool call details
│
├── core/                     # Core infrastructure
│   ├── agent/               # Agent process management
│   │   └── client.rs        # AgentManager, AgentHandle, GuiClient
│   ├── event_bus/           # Publish-subscribe event distribution
│   │   ├── core.rs          # Core event bus implementation
│   │   ├── batching.rs      # Event batching optimization
│   │   ├── session_bus.rs   # Session update events
│   │   ├── permission_bus.rs# Permission request events
│   │   ├── workspace_bus.rs # Workspace status events
│   │   ├── code_selection_bus.rs  # Code selection events
│   │   └── agent_config_bus.rs    # Agent config change events
│   ├── services/            # Business logic layer
│   │   ├── agent_service.rs         # Agent/session management
│   │   ├── message_service.rs       # Message handling & event publishing
│   │   ├── persistence_service.rs   # JSONL session persistence
│   │   ├── workspace_service.rs     # Workspace state management
│   │   ├── agent_config_service.rs  # Dynamic agent configuration
│   │   ├── ai_service.rs            # AI model integration
│   │   └── config_watcher.rs        # File system watching
│   ├── nodejs/              # Node.js runtime integration
│   ├── updater/             # Application update system
│   │   ├── checker.rs       # Check for new versions
│   │   ├── downloader.rs    # Download updates
│   │   └── version.rs       # Version parsing and comparison
│   ├── config.rs            # Configuration types
│   └── config_manager.rs    # Configuration management
│
├── components/               # Reusable UI components
│   ├── agent_message.rs     # AI message display
│   ├── user_message.rs      # User message display
│   ├── tool_call_item.rs    # Tool call visualization
│   ├── agent_todo_list.rs   # Todo list component
│   ├── chat_input_box.rs    # Chat input with file upload
│   ├── permission_request.rs # Permission request UI
│   ├── diff_summary.rs      # Diff summary component
│   ├── diff_view.rs         # Diff visualization
│   ├── agent_select.rs      # Agent selection dropdown
│   ├── command_suggestions_popover.rs # Command suggestions
│   ├── file_picker.rs       # File picker dialog
│   ├── input_suggestion.rs  # Input suggestions
│   ├── select_items.rs      # Select items component
│   └── status_indicator.rs  # Status indicator
│
├── workspace/                # Workspace management
│   ├── mod.rs               # DockWorkspace with layout persistence
│   └── actions.rs           # Workspace-specific actions
│
├── schemas/                  # Data models
│   ├── conversation_schema.rs # Conversation data structures
│   ├── task_schema.rs       # Task data structures
│   └── workspace_schema.rs  # Workspace data structures
│
├── utils/                    # Utility functions
│   ├── clipboard.rs         # Clipboard operations
│   ├── file.rs              # File utilities
│   ├── time.rs              # Time utilities
│   └── tool_call.rs         # Tool call utilities
│
├── reqwest_client/          # HTTP client (embedded)
│   ├── mod.rs               # HTTP client interface
│   ├── reqwest_client.rs    # Reqwest implementation
│   └── http_client_tls.rs   # TLS configuration
│
├── assets.rs                 # Asset management (rust-embed)
├── i18n.rs                   # Internationalization
├── lib.rs                    # Library entry point
└── main.rs                   # Application entry point
```

### Key Design Patterns

#### 1. Service Layer Pattern

Business logic is separated from UI through dedicated services:

```rust
// Send a message to an agent
let message_service = AppState::global(cx).message_service()?;
message_service.send_user_message(&agent_name, message).await?;

// Subscribe to session updates
let mut rx = message_service.subscribe_session_updates(Some(session_id));
```

#### 2. Event Bus Architecture

Real-time updates through optimized publish-subscribe pattern with batching:

```
User Input → ChatInput
  ├─→ Immediate publish to session_bus
  │    └─→ ConversationPanel receives instantly
  └─→ agent_handle.prompt()
       └─→ Agent processes
            └─→ GuiClient.session_notification()
                 └─→ session_bus.publish()
                      └─→ Batching layer (debounced updates)
                           └─→ Real-time UI update
```

**Event Buses:**
- **SessionUpdateBus**: Agent messages, tool calls, thinking updates (with batching)
- **PermissionBus**: Permission requests from agents
- **WorkspaceBus**: Workspace status changes
- **CodeSelectionBus**: Code selection events for editor integration
- **AgentConfigBus**: Agent configuration changes with hot-reload

**Performance Optimization:**
- **Event Batching**: Multiple updates are batched and debounced to reduce UI re-renders
- **O(1) Tool Call Lookups**: HashMap-based indexing for instant tool call updates
- **Optimized Message Merging**: Efficient streaming text append without full list traversal

#### 3. DockPanel Trait

Unified interface for all dockable panels:

```rust
pub trait DockPanel: 'static + Sized {
    fn title() -> &'static str;
    fn description() -> &'static str;
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render>;
    // Optional: closable(), zoomable(), title_bg(), paddings()
}
```

## 🛠️ Development

### Running Tests

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test
```

### Development Logging

Control log verbosity with `RUST_LOG`:

**Windows:**
```bash
# General info logging
set RUST_LOG=info && cargo run

# Debug specific modules
set RUST_LOG=info,agentx::core::services=debug && cargo run
set RUST_LOG=info,agentx::panels::conversation=debug && cargo run

# Debug event buses
set RUST_LOG=info,agentx::core::event_bus=debug && cargo run

# Combined debugging
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

# Combined debugging
RUST_LOG=info,agentx::core=debug,agentx::panels=debug cargo run

# Trace all component updates
RUST_LOG=trace cargo run
```

### Performance Profiling (macOS)

```bash
# Enable Metal HUD for FPS/GPU metrics
MTL_HUD_ENABLED=1 cargo run

# Profile with samply
cargo install samply
samply record cargo run --release
```

### Adding a New Panel

1. Create panel file in `src/panels/`:
```rust
// src/panels/my_panel.rs
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
```

2. Register in `src/lib.rs` and add to default layout in `src/workspace/mod.rs`

3. Export from `src/panels/mod.rs`

See [CLAUDE.md](CLAUDE.md) for detailed development guidelines.

## 📦 Dependencies

This project is part of the **gpui-component workspace** and uses local path dependencies.

### Core Framework
- **gpui** - Core GPUI framework from [Zed Industries](https://github.com/zed-industries/zed) (Git dependency)
- **gpui-component** `0.5.0` - UI component library (local path: `../crates/ui`)
- **gpui_term** - Terminal component (Git dependency from [sxhxliang/gpui-term](https://github.com/sxhxliang/gpui-term))

### Agent Communication
- **agent-client-protocol** `0.9.2` - ACP protocol implementation with unstable features
- **tokio** `1.48.0` - Async runtime for agent processes (rt-multi-thread, process, fs, io-util)
- **tokio-util** `0.7.17` - Tokio utilities for compatibility layer
- **async-trait** `0.1.89` - Async trait support
- **smol** `2` - Async executor

### HTTP Client (Embedded)
- **reqwest** - Zed's custom reqwest fork (Git dependency: `zed-reqwest` v0.12.15)
- **rustls** `0.23.26` - TLS implementation
- **rustls-platform-verifier** `0.5.0` - Platform certificate verification
- **bytes**, **futures** - Async I/O utilities

### Language Support
- **tree-sitter-navi** `0.2.2` - Syntax highlighting
- **lsp-types** `0.97.0` - Language Server Protocol types (with proposed features)
- **color-lsp** `0.2.0` - LSP for color support
- **similar** `2.6.0` - Text diff calculation for change statistics

### Utilities
- **serde**, **serde_json** - Serialization/deserialization
- **uuid** `1.11` - Unique identifier generation (v4 feature)
- **chrono** `0.4` - Date/time handling (with serde)
- **tracing**, **tracing-subscriber** - Logging with env-filter
- **rfd** `0.15` - Native file dialogs
- **image** `0.25` - Image processing
- **rust-embed** `8` - Asset embedding (with interpolate-folder-path)
- **rust-i18n** `3` - Internationalization
- **autocorrect** `2.14.2` - Text auto-correction
- **regex** `1` - Regular expressions
- **base64** `0.22` - Base64 encoding/decoding
- **which** `7.0` - Executable path resolution
- **dirs** `6.0` - Platform-specific directories
- **tray-icon** `0.19` - System tray integration

See [Cargo.toml](Cargo.toml) for complete dependency list.

## ⚙️ Build Configuration

### Compilation Profiles

AgentX uses optimized build profiles for different scenarios:

**Development Profile (`cargo build`):**
- `opt-level = 1` - Light optimization for faster compilation
- `debug = "limited"` - Limited debug info
- `strip = "debuginfo"` - Remove debug symbols but keep function names
- Key dependencies (resvg, rustybuzz, taffy, ttf-parser) use `opt-level = 3` for better runtime performance

**Release Profile (`cargo build --release`):**
- `opt-level = "z"` - Optimize for binary size
- `lto = "fat"` - Full link-time optimization
- `codegen-units = 1` - Maximum optimization
- `strip = true` - Remove all symbols and debug info
- `panic = "abort"` - Use abort strategy to reduce binary size

These profiles balance compilation speed (dev) and binary size/performance (release).

### Edition

This project uses **Rust Edition 2024** (requires Rust 1.85+).

## 🗂️ Data Storage

AgentX stores runtime data in the `target/` directory:

- `target/docks-agentx.json` - Layout state (debug builds)
- `target/sessions/*.jsonl` - Session history (JSONL format)
- `target/state.json` - Application state
- `target/workspace-config.json` - Workspace configuration

**Note**: In release builds, files are stored in the project root without `target/` prefix.

## 🤝 Contributing

Contributions are welcome! Please follow these guidelines:

1. **Code Style**: Follow existing patterns (see [CLAUDE.md](CLAUDE.md))
2. **Documentation**: Update relevant docs when adding features
3. **Testing**: Ensure tests pass before submitting PRs
4. **Commit Messages**: Use clear, descriptive commit messages

### Development Workflow

```bash
# Create a feature branch
git checkout -b feature/my-feature

# Make changes and test
cargo test
cargo run

# Commit and push
git commit -m "feat: add my feature"
git push origin feature/my-feature
```

## 📝 Documentation

- **[CLAUDE.md](CLAUDE.md)** - Comprehensive development guide for Claude Code
- **[GPUI Component Workspace](../../README.md)** - Main workspace documentation
- **[Component Gallery](../../crates/story/README.md)** - Interactive UI component examples
- **[GPUI Component Repository](https://github.com/longbridge/gpui-component)** - Official repository
- **[GPUI Documentation](https://www.gpui.rs/)** - GPUI framework documentation
- **[Agent Client Protocol](https://crates.io/crates/agent-client-protocol)** - ACP specification

## 🐛 Troubleshooting

### Common Issues

**Issue**: Application fails to start
- **Solution**: Check `config.json` is valid JSON and agent paths are correct
- Check RUST_LOG output for detailed error messages

**Issue**: Agent not responding
- **Solution**: Verify agent executable is accessible and has execute permissions
- Check agent process logs in the console
- Ensure agent supports Agent Client Protocol v0.9.2

**Issue**: Layout not saving
- **Solution**: Ensure `target/` directory has write permissions
- Check for file system errors in logs

**Issue**: LSP features not working
- **Solution**: Check language server is installed and configured
- Verify LSP server path in settings

**Issue**: Terminal panel not displaying
- **Solution**: Ensure shell path is correctly configured
- Check terminal emulation compatibility

**Issue**: Slow UI performance
- **Solution**: Check if event batching is enabled (should be automatic)
- Monitor CPU/memory usage with `RUST_LOG=debug`
- Consider reducing number of open panels

### Debug Mode

**Windows:**
```bash
set RUST_LOG=debug && cargo run 2>&1 | tee debug.log
```

**Unix/Linux/macOS:**
```bash
RUST_LOG=debug cargo run 2>&1 | tee debug.log
```

## 📄 License

This project is licensed under the **Apache-2.0 License**. See LICENSE file for details.

## 🙏 Acknowledgments

- **[GPUI](https://www.gpui.rs/)** - Zed's native GPU-accelerated UI framework from [Zed Industries](https://github.com/zed-industries/zed)
- **[gpui-component](https://github.com/longbridge/gpui-component)** - UI component library from [LongBridge](https://github.com/longbridge/gpui-component)
- **[Zed](https://zed.dev/)** - Inspiration for editor features and architecture
- **Agent Client Protocol** - Standard protocol for agent communication

## 🔗 Links

- **GPUI Component**: [github.com/longbridge/gpui-component](https://github.com/longbridge/gpui-component)
- **GPUI Framework**: [Zed Industries](https://github.com/zed-industries/zed)
- **Agent Client Protocol**: [crates.io/crates/agent-client-protocol](https://crates.io/crates/agent-client-protocol)

---

**Built with ❤️ using [GPUI](https://www.gpui.rs/) and [GPUI Component](https://github.com/longbridge/gpui-component)**
