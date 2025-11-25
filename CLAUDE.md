# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the `agentx` Agent Studio application, part of the gpui-component workspace. It demonstrates building a desktop application with GPUI Component, featuring:

- A dock-based layout system with multiple panels (left, right, bottom, center)
- Custom title bar with menu integration
- Code editor with LSP support (diagnostics, completion, hover)
- Persistent layout state management
- Theme support and customization

## Architecture

### Application Structure

- **Main Entry**: `src/main.rs` creates the application window with a `DockWorkspace`
- **DockWorkspace**: The root container managing the dock area, title bar, and layout persistence
- **Panels**: Individual UI components implementing the `DockPanel` trait and wrapped in `DockPanelContainer`
- **Dock System**: Uses `DockArea` from gpui-component for flexible panel layout

### Key Components

1. **DockWorkspace** (`src/main.rs`):
   - Manages the main dock area with version-controlled layout persistence
   - Saves layout state to `target/docks.json` (debug) or `docks.json` (release)
   - Handles layout loading, saving (debounced by 10 seconds), and version migration
   - Provides actions for adding panels and toggling visibility

2. **Panel System** (`src/lib.rs`):
   - `DockPanelContainer`: Wrapper for panels implementing the `Panel` trait
   - `DockPanel`: Trait that panels implement to define title, description, behavior
   - Panel registration happens in `init()` with deserialization from saved state

3. **CodeEditorPanel** (`src/editor.rs`):
   - High-performance code editor with LSP integration
   - Uses tree-sitter for syntax highlighting (navi language)
   - Mock LSP providers (completion, hover, diagnostics, code actions)
   - File tree integration for navigation

### Layout Persistence

The dock layout system uses versioned states:
- Current version: 5 (defined in `MAIN_DOCK_AREA`)
- When version mismatch detected, prompts user to reset to default
- Layout automatically saved 10 seconds after changes
- Layout saved on app quit via `on_app_quit` hook

## Development Commands

### Build and Run

```bash
# Run the agentx example
cargo run --example agentx

# Or from the workspace root
cargo run
```

### Build Only

```bash
cargo build --example agentx
```

### Development with Performance Profiling (macOS)

```bash
# Enable Metal HUD to see FPS
MTL_HUD_ENABLED=1 cargo run --example agentx

# Profile with samply
samply record cargo run --example agentx
```

## GPUI Component Integration

### Initialization Pattern

Always call `gpui_component::init(cx)` before using any GPUI Component features. This Agent Studio extends initialization with:

```rust
pub fn init(cx: &mut App) {
    agentx::init(cx);  // Custom initialization
    cx.bind_keys([...]);
    cx.activate(true);
}
```

### Root Element Requirement

The first level element in a window must be a `Root` from gpui-component:

```rust
cx.new(|cx| Root::new(view, window, cx))
```

This provides essential UI layers (sheets, dialogs, notifications).

### Creating Custom Panels

To add a new panel type:

1. Implement the `DockPanel` trait:
   - `title()`: Panel display name
   - `description()`: Panel description
   - `new_view()`: Create the panel view
   - Optional: `closable()`, `zoomable()`, `on_active()`

2. Register in `DockPanelState::to_story()` match statement

3. Add to default layout in `reset_default_layout()` or `init_default_layout()`

## Key Concepts

### Dock Placement

Panels can be added to: `Center`, `Left`, `Right`, `Bottom`

### Window Management

- Window bounds are centered and sized to 85% of display (max 1600x1200)
- Minimum window size: 640x480
- Custom titlebar on macOS/Windows, client decorations on Linux

### State Management

- Global state via `AppState` for tracking invisible panels
- Panel state serialization via `dump()` and deserialization via panel registry
- Layout state includes panel positions, sizes, and active tabs

## Testing

Run the complete story gallery from workspace root:

```bash
cargo run
```

This displays all GPUI components in a comprehensive gallery interface.

## Workspace Structure

This Agent Studio is part of a Cargo workspace at `../../`:

- `crates/ui`: Core gpui-component library
- `crates/story`: Story framework and examples
- `crates/macros`: Procedural macros
- `crates/assets`: Asset handling
- `examples/agentx`: This application
- `examples/hello_world`, `examples/input`, etc.: Other examples
- `crates/ui/src/icon.rs`: gpui-component IconName library
- `crates/story/src/*.rs`: gpui-component library examples 

## Dependencies

Key workspace dependencies:
- `gpui = "0.2.2"`: Core GPUI framework
- `gpui-component`: UI component library (workspace member)
- `gpui-component-assets`: Asset integration (workspace member)
- LSP support: `lsp-types`, `color-lsp`
- Syntax highlighting: `tree-sitter-navi`

## Coding Style

- Follow existing patterns for component creation and layout
- Use `cx.new()` for creating entities
- Prefer `Entity<T>` over raw views for state management
- Use GPUI's reactive patterns: subscriptions, notifications, actions
- Mouse cursor: use `default` not `pointer` for buttons (desktop convention)
- Default size: `md` for most components
