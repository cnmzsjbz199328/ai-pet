# AI Desktop Pet Theatre

An LLM-powered desktop pet that lives on your Windows desktop. Describe what you want it to do in natural language, and it performs a scripted animation sequence — driven by Google Gemini.

![demo](docs/demo.gif)

---

## Features

- Transparent, borderless, always-on-top floating window
- PNG sprite frame animations (idle, walk, jump, attack, sleep, happy, angry)
- Natural language → Gemini → structured timeline → smooth animation playback
- Daemon architecture: `start` returns immediately, pet runs in the background
- Drag the pet anywhere on your desktop

---

## Requirements

- Windows 10/11 x64
- [Rust toolchain](https://rustup.rs/) (stable, MSVC target: `x86_64-pc-windows-msvc`)
- Visual Studio 2022 Build Tools with **C++ workload** (for the MSVC linker)
- A [Google AI Studio](https://aistudio.google.com/) API key (free tier works)

---

## Quick Start

### 1. Clone and configure

```bash
git clone <repo-url>
cd idle
```

Create a `.env` file in the project root:

```
GEMINI_API_KEY=your_api_key_here
```

### 2. Build

```powershell
.\build.ps1
```

Or, if your environment already has MSVC and cargo in PATH:

```bash
cargo build --release
```

### 3. Run

```bash
# Start the pet (returns immediately — pet runs in background)
cargo run -p cli_app -- start

# Give it a script in natural language
cargo run -p cli_app -- play "猫咪发现了一只老鼠，愤怒地追击，跳起来攻击，最后开心地庆祝"

# Shut down the pet
cargo run -p cli_app -- stop
```

If you built with `--release`, use the binary directly:

```bash
./target/release/ai-pet start
./target/release/ai-pet play "walk around then sleep"
./target/release/ai-pet stop
```

---

## How It Works

```
ai-pet play "prompt"
      │
      ▼
  IPC (Named Pipe)
      │
      ▼
  Gemini API  ──►  JSON timeline script
      │
      ▼
  Timeline scheduler  ──►  action events at precise ms timestamps
      │
      ▼
  Animator  ──►  PNG frame lookup
      │
      ▼
  pixels (wgpu/GL)  ──►  transparent window
```

1. `play` sends your prompt to the daemon via a Windows Named Pipe.
2. The daemon calls Gemini, which returns a structured JSON script with timestamped actions.
3. The timeline scheduler fires each action at the right moment.
4. The animator plays the corresponding PNG frame sequence.
5. When the script finishes, the pet returns to idle automatically.

---

## Configuration

### `.env`

| Variable | Required | Description |
|---|---|---|
| `GEMINI_API_KEY` | Yes | Google AI Studio API key |
| `GEMINI_MODEL` | No | Model override (default: `gemini-3-flash-preview`) |

### `assets/config/pet_config.json`

Controls the pet's name, window size, and per-action animation settings:

```json
{
  "pet_id": "pet1",
  "display_name": "Mochi",
  "window_size": [128, 128],
  "actions": {
    "idle":   { "frame_count": 8,  "frame_duration_ms": 150, "looped": true  },
    "walk":   { "frame_count": 12, "frame_duration_ms": 80,  "looped": true  },
    "jump":   { "frame_count": 3,  "frame_duration_ms": 100, "looped": false },
    "attack": { "frame_count": 8,  "frame_duration_ms": 80,  "looped": false },
    "happy":  { "frame_count": 8,  "frame_duration_ms": 80,  "looped": false },
    "angry":  { "frame_count": 4,  "frame_duration_ms": 120, "looped": false },
    "sleep":  { "frame_count": 4,  "frame_duration_ms": 400, "looped": true  }
  }
}
```

### Adding custom sprites

1. Create a directory under `assets/sprites/<action_name>/`
2. Place frames as `001.png`, `002.png`, … (RGBA PNG, 80×64px recommended)
3. Add the action to `pet_config.json`
4. Add it to the `Action` enum in `core_engine/src/scripting/mod.rs`

---

## Project Structure

```
├── core_engine/        # Pure logic — animation, FSM, timeline, scripting types
├── cli_app/            # I/O layer — window, IPC, LLM client, CLI
│   └── src/
│       ├── ai/         # Gemini + OpenAI clients, script parser
│       ├── ipc/        # Windows Named Pipe server + client
│       └── window/     # winit event loop + pixels renderer
├── assets/
│   ├── sprites/        # PNG frame sequences, one dir per action
│   └── config/         # pet_config.json
├── .env                # API keys (not committed)
└── build.ps1           # MSVC environment setup helper for PowerShell
```

---

## Development

```bash
# Run unit tests (no display required)
cargo test -p core_engine

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Format
cargo fmt --all
```

Daemon logs are written to `~/.ai-pet/daemon.log`.

---

## Tech Stack

| Role | Crate |
|---|---|
| Window | `winit` 0.30 |
| Renderer | `pixels` 0.17 (wgpu/GL backend) |
| Async runtime | `tokio` |
| HTTP | `reqwest` (rustls) |
| CLI | `clap` |
| Logging | `tracing` |

---

## License

MIT
