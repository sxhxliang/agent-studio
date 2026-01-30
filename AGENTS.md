# Repository Guidelines

AgentX is a GPUI desktop application (Rust) demonstrating a dock-based "AI agent studio" UI. This directory is intended to be built either standalone or part of the `gpui-component` workspace.

## Project Structure & Module Organization

- `src/` — Rust sources
  - `src/app/` — menus, actions, theming, window chrome, system tray
  - `src/components/` — reusable UI widgets (agent messages, tool calls, chat input, etc.)
  - `src/panels/` — dockable panels (conversation, editor, tasks, settings, terminal)
  - `src/core/` — services + event bus + agent client + updater (business logic)
  - `src/workspace/` — workspace model/actions and wiring between panels/services
  - `src/utils/` — utility functions (file, clipboard, time, tool calls)
  - `src/schemas/` — data schemas for serialization
- `assets/` — icons and logos used by the UI
- `themes/` + `.theme-schema.json` — theme JSON files and schema
- `locales/` — i18n strings (`*.yml`)
- Runtime outputs: `target/` and `sessions/` are ignored via `.gitignore`.

## Build, Test, and Development Commands

| Command | Description |
|---------|-------------|
| `cargo run` | Run AgentX from this directory |
| `RUST_LOG=info cargo run` | Run with logging enabled |
| `cargo check` | Fast compile/type-check |
| `cargo test` | Run all unit tests |
| `cargo test <filter>` | Run tests matching filter (e.g., `cargo test version`) |
| `cargo fmt` | Format code (required before PRs) |
| `cargo clippy` | Lint; fix warnings or add justifications to `Cargo.toml` |

## Code Style Guidelines

### General Conventions

- **Rust Edition**: 2024; follow `rustfmt` output (`cargo fmt` before committing).
- **Naming**: `snake_case` (modules/functions/methods), `PascalCase` (types/traits/enums), `SCREAMING_SNAKE_CASE` (consts).
- **Comments**: Avoid adding comments unless clarifying complex logic; self-documenting code preferred.

### Import Organization

Group imports in this order with blank lines between groups:

```rust
// External crates (alphabetical)
use anyhow::Context as _;
use gpui::{App, Context, Entity, IntoElement, Render, Styled, Window};

// Local parent module imports
use crate::panels::{CodeEditorPanel, ConversationPanel, SettingsPanel};

// Specific imports from current or sibling modules
use super::app_state::AppState;
```

### Error Handling

- Use `anyhow` for application-level error handling.
- Provide context with `.context()` or `.with_context(|| ...)`:
  ```rust
  let config: Config = std::fs::read_to_string(path)
      .with_context(|| format!("failed to read {}", path.display()))?;
  let config = serde_json::from_str(&raw)
      .with_context(|| format!("invalid config at {}", path.display()))?;
  ```
- Use `anyhow::bail!` for early returns with errors.
- Reserve `panic!` for truly unrecoverable states (e.g., invariant violations).

### Async & Concurrency

- Use `tokio` for async runtime (imported in `Cargo.toml`).
- Use `async-trait` for async trait methods.
- Use `Arc` for shared ownership, `Mutex` or `RwLock` for interior mutability.
- Spawn async tasks with `cx.spawn(...).detach()` for fire-and-forget, or await for critical operations.

### UI Rendering (GPUI)

- Keep UI rendering in `src/components/` and `src/panels/`.
- Implement `Render` trait for views.
- Use `gpui_component` dock utilities for panel behavior.
- Use `ParentElement` and `IntoElement` for component composition:
  ```rust
  div()
      .flex()
      .size_full()
      .child(self.header.clone())
      .child(div().flex_1().child(content))
  ```
- Prefer `cx.update(|cx, ...|)` for UI state mutations.

### Architecture & Patterns

- **Event Bus**: Publish UI updates via event bus (`src/core/event_bus/`) rather than direct cross-panel calls.
- **Services**: Place business logic in `src/core/services/`; keep services small and testable.
- **AppState**: Use global `AppState` for app-wide state (accessed via `AppState::global(cx)`).
- **AppSettings**: Use `AppSettings` for user-configurable preferences.

### Testing Guidelines

- Tests colocated under `#[cfg(test)] mod tests` within the same file (no `tests/` directory).
- Keep tests deterministic: avoid network calls, timers, or flaky operations.
- Use `Arc<AtomicUsize>` for counting test assertions in concurrent scenarios.
- Test event bus subscriptions, parsing/serialization, and version comparisons.

### Git & Pull Requests

- Follow Conventional Commits: `feat(scope): description`, `fix(scope): description`, `refactor`, `docs`, `chore`.
- PRs should include: summary, rationale, reproduction steps, screenshots/GIFs for UI changes.
- Before opening PR: run `cargo fmt`, `cargo test`, `cargo clippy`.

## Configuration & Security

- `config.json` may contain local executable paths and env vars; do not commit secrets.
- Layout/session data written under `target/` in debug builds; delete `docks-agentx.json` to reset UI layout.
- Use `tracing`/`log` for instrumentation; avoid `dbg!` macro in production code (denied in `Cargo.toml` lints).

## Clippy Exceptions

Some lint rules are allowed in `Cargo.toml`. When disabling a lint, add a comment explaining why:

```rust
#[allow(clippy::too_many_arguments)] // Historical API, refactoring deferred
```
