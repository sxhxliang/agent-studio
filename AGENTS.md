# Repository Guidelines

AgentX is a GPUI desktop application (Rust) demonstrating a dock-based “AI agent studio” UI. This directory is intended to be built either standalone or as part of the `gpui-component` workspace.

## Project Structure & Module Organization

- `src/` — Rust sources
  - `src/app/` — menus, actions, theming, window chrome
  - `src/components/` — reusable UI widgets
  - `src/panels/` — dockable panels (conversation, editor, tasks, settings)
  - `src/core/` — services + event bus + agent client + updater (business logic)
  - `src/workspace/` — workspace model/actions and wiring between panels/services
- `assets/` — icons and logos used by the UI
- `themes/` + `.theme-schema.json` — theme JSON files and schema
- `locales/` — i18n strings (`*.yml`)
- Runtime outputs: `target/` and `sessions/` are ignored via `.gitignore`.

## Build, Test, and Development Commands

- `cargo run` — run AgentX from this directory
- `RUST_LOG=info cargo run` — run with logging enabled
- `cargo check` — fast compile/type-check
- `cargo test` / `cargo test <filter>` — run unit tests
- `cargo fmt` — format code (required before PRs)
- `cargo clippy` — lint (fix or justify warnings)

If you’re working from the workspace root, you can run: `cd ../.. && cargo run --example agentx`.

## Coding Style & Naming Conventions

- Rust edition is `2024`; follow `rustfmt` output (`cargo fmt`).
- Keep naming idiomatic: `snake_case` (modules/functions), `PascalCase` (types), `SCREAMING_SNAKE_CASE` (consts).
- Keep UI rendering in `src/components/` and `src/panels/`; keep non-UI logic in `src/core/`.
- Prefer small, testable services and publish UI updates via the event bus rather than direct cross-panel calls.

## Testing Guidelines

- Tests are colocated under `#[cfg(test)] mod tests` (no `tests/` directory).
- Add unit tests for event-bus behavior, parsing/serialization, and versioning; keep tests deterministic (avoid network and timers).

## Commit & Pull Request Guidelines

- Recent history often follows Conventional Commits (e.g. `feat(task-panel): …`, `refactor(workspace): …`); use `feat|fix|refactor|docs|chore` with an optional scope when possible.
- PRs should include: a clear summary, rationale, reproduction steps, and screenshots/GIFs for UI changes.
- Before opening a PR, run: `cargo fmt`, `cargo test`, `cargo clippy`.

## Configuration & Security Tips

- `config.json` may contain local executable paths and environment variables; do not commit secrets.
- Layout/session data is written under `target/` in debug builds (and may be stored in the project directory for release builds); deleting generated layout state (e.g. `docks-agentx.json`) resets the UI layout.
