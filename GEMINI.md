# AI Desktop Pet Theatre: GEMINI.md

This file provides foundational guidance, architectural invariants, and development conventions for the **AI Desktop Pet Theatre** project.

---

## Project Overview

**AI Desktop Pet Theatre** is an LLM-driven desktop pet system for Windows. It features a transparent, always-on-top floating window that renders PNG sprite-based animations. The pet's lifecycle and behavior are controlled via a CLI, where natural-language prompts are sent to an LLM to generate structured animation scripts.

- **Target Platform:** Windows 11, `x86_64-pc-windows-msvc`.
- **Core Goal:** Create a modular, performance-oriented, and extensible desktop pet engine.

---

## Workspace Structure & Architecture

The project is structured as a Rust workspace with two primary crates, enforcing a strict separation of concerns.

### 1. `core_engine` (Pure Logic)
- **Role:** Handles animation frame calculation, state machine transitions, timeline scheduling, and shared data types.
- **Invariant:** Must remain "pure." No I/O, no networking, no windowing, and no platform-specific dependencies (e.g., `tokio`, `reqwest`, `winit`, `pixels`).
- **Communication:** Uses traits for dependency injection (e.g., `LlmClient`).

### 2. `cli_app` (Assembly & I/O)
- **Role:** The application entry point. Handles window creation (`winit`), rendering (`pixels`), IPC (Windows Named Pipes), LLM HTTP calls (`reqwest`), and CLI parsing (`clap`).
- **Thread Model:**
    - **Main Thread:** Runs the `winit` EventLoop (required for Windows).
    - **Tokio Runtime:** Spawned at startup for async tasks (IPC, LLM).
    - **Sync:** Uses `std::sync::mpsc` to bridge the Tokio runtime and the main event loop.

---

## Technology Stack

| Role | Crate | Version |
|---|---|---|
| **Windowing** | `winit` | 0.30 |
| **Rendering** | `pixels` | 0.14 |
| **Async Runtime** | `tokio` | 1.x |
| **HTTP Client** | `reqwest` | 0.12 |
| **CLI Parser** | `clap` | 4.x |
| **Serialization** | `serde` / `serde_json` | 1.x |
| **Logging** | `tracing` | 0.1 |
| **Error Handling** | `thiserror` (Engine) / `anyhow` (App) | 2.x / 1.x |

---

## Development Conventions

### 1. Error Handling
- **Production Path:** NO `.unwrap()` or `.expect()`.
- **`core_engine`:** Use `thiserror` to define granular, domain-specific `EngineError` variants.
- **`cli_app`:** Use `anyhow::Result` for application-level error management and CLI output.

### 2. Logging & Output
- **`tracing` only:** Use `info!`, `warn!`, `error!`, `debug!`, and `trace!`.
- **No `println!`:** Avoid standard output macros in production code.
- **Default Level:** `INFO` (configurable via `RUST_LOG`).

### 3. Crate Boundaries & Visibility
- **Modular Privacy:** Prefer `pub(crate)` for internal logic. Only expose what is necessary for `cli_app` or public API.
- **Dependency Injection:** Inject external capabilities into `core_engine` via traits.

### 4. Windows-Specific Guards
- All IPC and Win32 code must be wrapped in `#[cfg(target_os = "windows")]` with appropriate stubs for non-Windows builds.

### 5. Formatting & Linting
- **`rustfmt`:** Always format code before committing.
- **`clippy`:** All warnings are treated as errors (`-D warnings`). Pedantic lints are encouraged.

---

## Building, Running & Testing

### Key Commands
```bash
# Build the project
cargo build

# Run the full test suite (requires Windows display)
cargo test

# Fast test (headless, CI-friendly)
cargo test -p core_engine

# Run the application (Start daemon/window)
cargo run -p cli_app -- start

# Send a prompt to the running daemon
cargo run -p cli_app -- play "Make the pet jump and then sleep"

# Stop the daemon
cargo run -p cli_app -- stop

# Lint and Format Check
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

### Testing Strategy
- **`core_engine`:** Exhaustive unit tests for state transitions and animation logic.
- **`cli_app`:** Integration tests for IPC and LLM clients.
- **CI Scope:** The CI pipeline (GitHub Actions) runs `cargo test -p core_engine`. Window/display-dependent tests are excluded using the `ci` feature flag.

---

## Assets & Configuration

### `assets/sprites/`
- Sprites are organized by action folder (e.g., `idle/`, `walk/`).
- Frame naming convention: `001.png`, `002.png`, etc.

### `assets/config/pet_config.json`
Defines the animation parameters (frame counts, durations, looping behavior) for each action. `core_engine` validates this file on startup.

---

## Project Documentation
Refer to the `docs/` directory for detailed specifications:
- `01_epics_and_stories.md`: Roadmap and requirements.
- `02_technical_design.md`: Architecture and thread models.
- `03_coding_standards.md`: Detailed coding and asset rules.
- `04_testing_and_verification.md`: Manual test checklists.
- `05_implementation_plan.md`: Current development status.
