# AgentX - AI Agent Studio

A full-featured desktop application built with [GPUI Component](https://github.com/sxhxliang/gpui-component), showcasing a modern dock-based interface for interacting with AI agents. AgentX demonstrates professional-grade UI patterns, real-time event-driven architecture, and comprehensive agent communication capabilities.

![AgentX Screenshot](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)
![Rust Version](https://img.shields.io/badge/Rust-1.75%2B-orange)
![License](https://img.shields.io/badge/License-MIT-green)

## ✨ Features

### 🎨 **Modern UI Architecture**
- **Dock-based Layout System**: Flexible panel management with four dock areas (Center, Left, Right, Bottom)
- **Persistent Layout State**: Automatic layout saving/loading with versioning support
- **Custom Title Bar**: Native-looking custom window controls on all platforms
- **Theme System**: Multiple color themes with light/dark mode support

### 💬 **AI Agent Integration**
- **Real-time Communication**: Event-driven architecture using publish-subscribe pattern
- **Session Management**: Multi-session support with session-scoped message routing
- **Agent Client Protocol (ACP)**: Full implementation of agent communication protocol
- **Permission Handling**: Interactive permission request workflow

### 🛠️ **Development Tools**
- **Code Editor**: Integrated editor with LSP support (diagnostics, completion, hover, code actions)
- **Tree-sitter Integration**: Syntax highlighting for multiple languages
- **Task Management**: Collapsible task list with status tracking
- **Conversation UI**: Rich message components with markdown support and streaming
- **Diff Summary**: File change statistics and visualization with collapsible view

### 🏗️ **Architecture Highlights**
- **Service Layer Pattern**: Separation of business logic from UI components
- **Event Bus System**: Thread-safe message distribution across components
- **Modular Design**: Clean separation of concerns with well-organized directory structure
- **Diff Visualization**: Context-aware diff display with collapsed unchanged sections

## 🚀 Quick Start

### Prerequisites

- **Rust**: 1.75 or later (install from [rustup.rs](https://rustup.rs/))
- **Git**: For cloning the repository

### Installation

This is a **standalone project** that can be built independently:

```bash
# Clone the repository
git clone <your-agentx-repository-url>
cd agentx

# Run the application
cargo run

# Or run with logging enabled
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

**Note**: This project uses Git dependencies for GPUI and gpui-component. The first build may take some time as Cargo fetches and compiles dependencies.

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

### Keyboard Shortcuts

- `Tab` / `Shift+Tab`: Navigate between panels
- `Ctrl+Q` / `Cmd+Q`: Quit application
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

- **Theme**: Color scheme and light/dark mode
- **Font**: Editor and UI font selection
- **Locale**: Interface language
- **Scrollbar**: Display mode (auto/always/never)
- **Border Radius**: Component corner rounding

## 🏛️ Architecture

### Directory Structure

```
src/
├── app/                      # Application-level modules
│   ├── actions.rs           # Centralized action definitions
│   ├── app_state.rs         # Global application state
│   ├── menu.rs              # Menu system
│   ├── themes.rs            # Theme management
│   └── title_bar.rs         # Custom title bar
│
├── panels/                   # All panel implementations
│   ├── dock_panel.rs        # DockPanel trait and container
│   ├── conversation_acp/    # ACP-enabled conversation panel
│   │   ├── panel.rs         # Main panel logic
│   │   ├── types.rs         # Reusable types
│   │   └── mod.rs
│   ├── code_editor/         # Code editor with LSP
│   ├── task_list/           # Task management panel
│   ├── chat_input.rs        # Chat input panel
│   ├── welcome_panel.rs     # Welcome screen
│   └── settings_window.rs   # Settings UI
│
├── core/                     # Core infrastructure
│   ├── agent/               # Agent client management
│   │   ├── client.rs        # AgentManager, AgentHandle
│   │   └── mod.rs
│   ├── event_bus/           # Event distribution system
│   │   ├── session_bus.rs   # Session updates
│   │   ├── permission_bus.rs# Permission requests
│   │   └── mod.rs
│   ├── services/            # Business logic services
│   │   ├── agent_service.rs # Agent/session management
│   │   ├── message_service.rs# Message handling
│   │   └── mod.rs
│   └── config.rs            # Configuration types
│
├── components/               # Reusable UI components
│   ├── agent_message.rs     # AI message display
│   ├── user_message.rs      # User message display
│   ├── tool_call_item.rs    # Tool call visualization
│   ├── agent_todo_list.rs   # Todo list component
│   └── ...
│
├── workspace/                # Workspace management
│   ├── mod.rs               # DockWorkspace implementation
│   └── actions.rs           # Workspace actions
│
├── schemas/                  # Data models
├── utils/                    # Utility functions
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

Real-time updates through publish-subscribe pattern:

```
User Input → ChatInput
  ├─→ Immediate publish to session_bus
  │    └─→ ConversationPanel receives instantly
  └─→ agent_handle.prompt()
       └─→ Agent processes
            └─→ GuiClient.session_notification()
                 └─→ session_bus.publish()
                      └─→ Real-time UI update
```

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

```bash
# General info logging
RUST_LOG=info cargo run

# Debug specific modules
RUST_LOG=info,agentx::core::services=debug cargo run
RUST_LOG=info,agentx::panels::conversation_acp=debug cargo run

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

This is a **standalone project** with Git-based dependencies.

### Core Framework
- **gpui** - Core GPUI framework from [Zed Industries](https://github.com/zed-industries/zed) (Git dependency)
- **gpui-component** `0.5.0` - UI component library from [LongBridge](https://github.com/longbridge/gpui-component) (Git dependency)

### Agent Communication
- **agent-client-protocol** `0.9.0` - ACP protocol implementation
- **tokio** `1.48.0` - Async runtime for agent processes
- **tokio-util** `0.7.17` - Tokio utilities

### HTTP Client (Embedded)
- **reqwest** - Zed's custom reqwest fork (Git dependency)
- **rustls** `0.23.26` - TLS implementation
- **rustls-platform-verifier** `0.5.0` - Platform certificate verification
- **bytes**, **futures** - Async I/O utilities

### Language Support
- **tree-sitter-navi** `0.2.2` - Syntax highlighting
- **lsp-types** `0.97.0` - Language Server Protocol types
- **color-lsp** `0.2.0` - LSP for color support
- **similar** `2.6.0` - Text diff calculation for change statistics

### Utilities
- **serde**, **serde_json** - Serialization/deserialization
- **uuid** `1.11` - Unique identifier generation
- **chrono** `0.4` - Date/time handling
- **tracing**, **tracing-subscriber** - Logging
- **rfd** `0.15` - Native file dialogs
- **image** `0.25` - Image processing

See [Cargo.toml](Cargo.toml) for complete dependency list.

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
- **[GPUI Component](https://github.com/longbridge/gpui-component)** - Official GPUI Component repository
- **[GPUI Documentation](https://www.gpui.rs/)** - GPUI framework documentation
- **[Workspace Documentation](../../README.md)** - GPUI Component workspace overview
- **[Component Gallery](../../crates/story/README.md)** - UI component examples

## 🐛 Troubleshooting

### Common Issues

**Issue**: Application fails to start
- **Solution**: Check `config.json` is valid JSON and agent paths are correct

**Issue**: Agent not responding
- **Solution**: Verify agent executable is accessible and has execute permissions

**Issue**: Layout not saving
- **Solution**: Ensure `target/` directory has write permissions

**Issue**: LSP features not working
- **Solution**: Check language server is installed and configured

### Debug Mode

Run with full debug logging:
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
