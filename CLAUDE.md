# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**AI Desktop Pet Theatre** — an LLM-driven desktop pet system for Windows. A transparent, always-on-top floating window renders PNG sprite-based animations. A CLI controls the pet lifecycle; natural-language prompts are sent to an LLM, which returns a structured JSON script that drives a timeline-based animation engine.

Target platform: **Windows 11, x86_64-pc-windows-msvc**.

---

## Workspace Structure

```
ai-pet/
├── Cargo.toml              # Workspace root
├── rust-toolchain.toml     # Pinned toolchain
├── core_engine/            # Pure logic — no I/O, no network, no window
│   └── src/
│       ├── animation/      # Sprite frame playback (Duration-driven)
│       ├── state_machine/  # FSM: Idle → Acting → Scripted → Idle
│       ├── timeline/       # Event scheduler (u64 ms timestamps)
│       └── scripting/      # Shared types: Action enum, TimelineEvent
└── cli_app/                # Assembly + I/O
    └── src/
        ├── main.rs         # Clap CLI (subcommands: start / play / stop)
        ├── ai/             # LlmClient trait + OpenAI implementation
        ├── ipc/            # Windows Named Pipe server + client
        └── window/         # winit EventLoop + pixels renderer
assets/
├── sprites/<action>/       # 001.png, 002.png, … (one dir per Action)
└── config/pet_config.json  # Per-pet animation config (see schema below)
```

---

## Commands

```bash
# Build
cargo build
cargo build --release

# Test core_engine only (fast, no display needed — matches CI scope)
cargo test -p core_engine

# Full test suite (requires Windows display for window tests)
cargo test

# Lint (must pass before committing)
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check

# Run
cargo run -p cli_app -- start          # start daemon + window
cargo run -p cli_app -- play "prompt"  # inject prompt into running daemon
cargo run -p cli_app -- stop           # shut down daemon
```

---

## Technology Stack (fixed — not up for debate)

| Role | Crate | Version |
|---|---|---|
| Window | `winit` | 0.30 |
| Renderer | `pixels` | 0.14 |
| PNG decode | `image` | 0.25 |
| Async runtime | `tokio` (multi-thread) | 1 |
| HTTP | `reqwest` (rustls-tls) | 0.12 |
| CLI | `clap` (derive) | 4 |
| JSON | `serde` + `serde_json` | 1 |
| Logging | `tracing` + `tracing-subscriber` | 0.1 |
| Engine errors | `thiserror` | 2 |
| App errors | `anyhow` | 1 |

---

## Architecture: Key Invariants

### Crate boundary
`core_engine` must stay pure: no `reqwest`, `clap`, `tokio`, `winit`, `pixels`, or any I/O dependency. All infrastructure is wired in `cli_app` via trait objects (e.g. `LlmClient`).

### Thread model
- **Main thread**: owned by `winit::EventLoop::run` (Windows requirement — cannot block).
- **Tokio runtime**: spawned at startup, runs IPC listener and LLM HTTP calls as tasks.
- **Cross-thread**: `std::sync::mpsc` — Tokio tasks hold `Sender<AppMessage>`, main thread drains `Receiver` on every `Event::AboutToWait`.
- **Game loop**: `ControlFlow::Poll`. Each `AboutToWait`: drain receiver → `timeline.tick(delta)` → `state_machine.apply()` → `animator.update(delta)` → `pixels.render()`.

### AppMessage
```rust
pub enum AppMessage {
    InjectTimeline(Vec<TimelineEvent>),
    LlmError(String),
    Shutdown,
}
```

### FSM state transitions (complete)
| From | Event | To |
|---|---|---|
| `Idle` | `InjectTimeline` | `Scripted` |
| `Idle` | `Click` | `Acting` |
| `Acting` | `InjectTimeline` | `Scripted` |
| `Acting` | `LlmError` | `Idle` |
| `Scripted` | `TimelineFinished` | `Idle` |
| Any | `Drag` | unchanged |

`Acting` = "waiting for LLM response". `Scripted` = "playing the script".

### IPC (Windows Named Pipes)
- Pipe name: `\\.\pipe\ai-pet-ipc`
- Wire format: `[4-byte u32 LE length][UTF-8 JSON body]`
- Message body: `{"prompt": "..."}`
- All pipe code gated with `#[cfg(target_os = "windows")]`; non-Windows stub returns `Err`.

### LLM output contract
System prompt enforces JSON-only output. Expected structure:
```json
{
  "characters": ["pet1"],
  "events": [
    {"timestamp_ms": 0,    "actor_id": "pet1", "action": "angry"},
    {"timestamp_ms": 2000, "actor_id": "pet1", "action": "walk"}
  ]
}
```
Field name is `actor_id` in both JSON and Rust struct. Action whitelist: `idle walk jump attack sleep happy angry`. Unknown actions fall back to `Action::Idle` with `tracing::warn!` — never return `Err` for unknown actions. Non-JSON responses return `Err`.

### Texture type
```rust
type Texture = Arc<image::RgbaImage>;
```
PNGs are decoded eagerly at startup. Load failures → `EngineError::AssetNotFound(path)`.

### pet_config.json schema
```json
{
  "pet_id": "pet1",
  "display_name": "Mochi",
  "window_size": [128, 128],
  "default_action": "idle",
  "actions": {
    "idle":   {"frame_count": 4, "frame_duration_ms": 200, "looped": true},
    "walk":   {"frame_count": 6, "frame_duration_ms": 100, "looped": true},
    "sleep":  {"frame_count": 2, "frame_duration_ms": 500, "looped": true},
    "jump":   {"frame_count": 5, "frame_duration_ms": 80,  "looped": false},
    "attack": {"frame_count": 6, "frame_duration_ms": 80,  "looped": false},
    "happy":  {"frame_count": 4, "frame_duration_ms": 150, "looped": false},
    "angry":  {"frame_count": 4, "frame_duration_ms": 150, "looped": false}
  }
}
```

---

## Error Handling

- `core_engine`: `thiserror` typed `EngineError` (`AssetNotFound`, `ConfigInvalid`, `UnknownAction`, `InvalidTimeline`, `MissingAnimation`).
- `cli_app`: `anyhow::Result` for all public functions; bind to correct exit codes.
- No `.unwrap()` / `.expect()` in production paths.

## Logging

`tracing` only — no `println!`. Default level `INFO`, override via `RUST_LOG`. See `docs/03_coding_standards.md` for level-by-level guidance.

## Testing

CI scope: `cargo test -p core_engine` only (headless). Window/IPC tests are manual. Use `#[cfg(not(feature = "ci"))]` to exclude display-dependent tests from CI. See `docs/04_testing_and_verification.md` for the full manual checklist.

---

## Design Docs

- `docs/01_epics_and_stories.md` — epics, user stories, MVP acceptance criteria
- `docs/02_technical_design.md` — architecture diagram, module designs, thread model, IPC wire format, LLM prompt template, Windows rendering notes
- `docs/03_coding_standards.md` — crate boundaries, error handling, logging levels, asset schema, CI pipeline, toolchain pinning
- `docs/04_testing_and_verification.md` — unit test cases, mock strategy, manual verification checklist
