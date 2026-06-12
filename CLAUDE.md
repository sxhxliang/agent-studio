# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AgentX is a GPU-accelerated desktop **AI agent studio** (Rust + GPUI) that talks to AI agents over the **Agent Client Protocol (ACP)**, each agent running as a managed subprocess. Agents are configured in `config.json` (Codex, Claude, Gemini, Qwen, etc.).

## ⚠️ Current state: a big-bang rewrite in progress

The repository contains **two architectures side by side**. Know which one you are touching:

- **Hexagonal rewrite (`crates/agentx-*`) — where all active development happens.** Clean ports-and-adapters design, the binary is `agentx-shell`. Build/run/test against these crates.
- **Legacy app (`src/` + `crates/agentx-{types,event-bus,agent,services,acp-ui}`, `git-worktree-manager`) — being cut over and replaced.** Don't add features here. The existing `src/panels`, `src/core/services`, `AppState` service-locator, and `DockPanel` machinery belong to this legacy layer; they are the *source material* for the rewrite, not the target.

New work goes in the hexagonal crates. When migrating a legacy panel, read the `src/panels/*` + `src/components/*` original for behavior **and visual style**, then rebuild it cleanly in `crates/agentx-ui`.

## Build, run, and test (rewrite)

```bash
# Run the rewrite (opens the launcher window; pick an agent from config.json)
cargo run -p agentx-shell

# With logs (env_logger; module names use underscores)
RUST_LOG=info cargo run -p agentx-shell
RUST_LOG=info,agentx_acp=debug,agentx_app=debug cargo run -p agentx-shell

# Type-check / build a single crate
cargo check -p agentx-ui
cargo build -p agentx-shell

# Tests live in the domain-facing crates (agentx-domain/-acp/-app/-store/-bus)
cargo test -p agentx-domain -p agentx-acp -p agentx-app -p agentx-store
cargo test -p agentx-acp <test_name>     # single test by name

cargo fmt
cargo clippy -p agentx-ui -p agentx-shell
```

**Gotcha:** workspace-wide `cargo test` / `cargo clippy --all-targets` currently fails — the legacy `agentx-acp-ui` crate's *test/example* targets don't compile (its library does). Scope commands to the rewrite crates with `-p`. The legacy root binary still builds via plain `cargo run`.

UI/feature correctness can't be verified from the terminal — type-check + tests prove code correctness only. Hand off running the windowed app (and anything touching real agents/network) to the user.

## Hexagonal architecture (the rewrite)

Dependency arrows point **inward** to the domain. Each crate's `lib.rs` header states its dependency rule; honor it.

| Crate | Role | May depend on |
|-------|------|---------------|
| `agentx-domain` | **Core**: entities, value objects, lifecycle state machines, and **port traits**. Zero framework deps (only serde/thiserror/chrono/uuid/async-trait). | nothing internal |
| `agentx-bus` | Infra: a typed async pub/sub `EventBus` (tokio broadcast, routed by `TypeId`). Knows nothing about the domain. | — |
| `agentx-store` | **Driven adapter**: `FsSessionRepository` (JSONL timelines) + `FsConfigStore`; `paths` is the single source of on-disk locations. | domain |
| `agentx-acp` | **Driven adapter**: implements `AgentGateway`/`AgentRegistry` over ACP; supervises agent subprocesses; `mapping.rs` is the ACP↔domain anti-corruption layer. | domain, bus |
| `agentx-app` | **Application**: use-cases orchestrating the domain via ports (`SessionService`, `PersistenceProjector`). | domain, bus |
| `agentx-ui` | **Driving adapter**: GPUI panels/view-models/dock (`ChatView`, `WelcomeView`, `SessionsPanel`). | app, domain, bus, gpui |
| `agentx-shell` | **Composition root**: the binary. The *only* place adapters are constructed and injected into ports. | everything |

**Ports** (traits in `agentx-domain/src/ports.rs`): `AgentGateway` + `AgentRegistry` (impl: `AcpSupervisor`), `SessionRepository` + `ConfigStore` (impl: the `Fs*` store types). The application depends on these traits, never on a concrete adapter. `agentx-ui` must **not** depend on `agentx-acp`/`agentx-store`.

## Cross-cutting design you must understand before editing

These constraints span multiple crates and are easy to violate:

- **GPUI executor ≠ a Tokio runtime.** `agentx-shell` creates **no** app-level Tokio runtime. Store adapters use blocking `std::fs` so their async port methods can be awaited from GPUI's executor. Each ACP agent runs on its **own OS thread** with a current-thread Tokio runtime + `LocalSet` (ACP connection futures are `!Send`). The `PersistenceProjector` drains the bus on `cx.background_executor()` so per-event file appends never block the UI thread.
- **The event bus has no replay.** `EventBus` is broadcast: a subscriber only sees events published *after* it subscribes. **Subscribe consumers before starting the producer** (agent). The shell/launcher subscribes the projector and each chat view's `Receiver<DomainEvent>` *before* booting the agent, then injects the receiver — otherwise session-setup events (slash commands, config options) are lost.
- **Two event layers — don't conflate them.** `SessionEvent` is the in-session timeline (`UserMessage`/`AgentMessage`/`ToolCall`/…) that `SessionRepository` persists and the UI renders. `DomainEvent` is the cross-cutting bus notification (`SessionAppended`, `SessionStatusChanged`, `PermissionRequested`, `SessionConfigChanged`, …). Adapters/UI react to `DomainEvent`; they never call each other directly.
- **ACP types stop at `agentx-acp::mapping`.** All translation between `agent_client_protocol::schema` and domain types lives there (`*_to_domain` inbound, `*_to_acp` outbound). ACP schema types must never leak inward to app/ui/domain. ACP enums are `#[non_exhaustive]` — always include a catch-all arm.
- **Persistence is a projector, not a call.** Nothing calls "save"; `PersistenceProjector` subscribes to the bus and turns `SessionAppended` into stored rows. Streamed agent output is folded into whole `SessionEvent`s by a per-session accumulator in `agentx-acp` before it reaches the bus.
- **GPUI entity lifecycle.** Entities created inside `render()` are dropped when it returns. Store long-lived entities as struct fields (`Entity<T>`) and `.clone()` them in `render`; never `cx.new(...)` a widget inside `render`. Each `agentx-ui` panel splits into a `mod.rs` (state + intents) and a `view.rs` (pure render) — keep that shape.

## Conventions

- **Rust edition 2024.** Run `cargo fmt` before committing.
- **Lints are pre-relaxed in `Cargo.toml`:** `dead_code`, `unused_variables`, `unused_imports`, and most `clippy::style` are `allow`. `dbg!` is **denied** — never commit `dbg!`. When silencing another lint locally, add a one-line `#[allow(...)]` justification.
- **Commits:** Conventional Commits (`feat(scope):`, `fix:`, `refactor:`, `docs:`, `chore:`). Do not commit unless asked.
- **Tests** are colocated in `#[cfg(test)] mod tests`; keep them deterministic (no network/timers). The domain stays pure — pass `now: DateTime<Utc>` in rather than calling `Utc::now()` inside it.
- **Error handling:** `anyhow` at the app/shell boundary with `.context(...)`; typed `thiserror` errors (`AgentError`, `StoreError`) inside the domain/adapters.

## Configuration & data locations

`agentx-store::paths` is authoritative. Per-user data dir: Windows `%APPDATA%\agentx`, Linux `~/.config/agentx`, macOS `~/.agentx`. Within it: `config.json` (agents/models/MCP servers/proxy) and `sessions/{session_id}.jsonl` (per-session timelines).

`config.json` holds local executable paths, env vars, and may hold provider API keys — **do not read or commit it.** If agents don't appear, it's usually a proxy/network issue (configure proxy in settings).
