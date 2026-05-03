# AI Desktop Pet Theatre: Coding Standards

为保证工程结构清晰可维护，防范架构腐化，制定以下 Rust 编码与项目管理规范。

---

## 一、工程结构与可见性 (Workspace & Visibility)

### Crate 职责边界

项目严格分为两个 Crate，边界不可逾越：

| Crate | 职责 | 禁止引入的依赖 |
|---|---|---|
| `core_engine` | 纯逻辑：状态机、时间轴、动画帧计算 | `reqwest`、`clap`、`tokio`、`winit`、`pixels` 等任何 I/O 与平台依赖 |
| `cli_app` | 组装层：窗口、网络、IPC、CLI 解析 | 无额外限制，但不允许将业务逻辑下沉到此层 |

所有跨 Crate 的依赖注入（如 `LlmClient`）通过 `trait` 对象完成，`core_engine` 不持有任何具体实现引用。

### 访问修饰符规范

- 模块内部类型与函数使用 `pub(crate)`，不对外暴露。
- 只有 `cli_app` 需要直接调用的 API（如 `Animator::new`、`Timeline::push_events`）使用 `pub`。
- 禁止无意义地将所有结构体字段标记为 `pub`；使用构造函数或 builder 模式控制字段访问。

### 平台特定代码守卫

所有仅适用于 Windows 的代码（IPC Named Pipe、Win32 API 调用）必须使用编译守卫：

```rust
#[cfg(target_os = "windows")]
mod ipc_windows { /* Named Pipe 实现 */ }

#[cfg(not(target_os = "windows"))]
mod ipc_stub {
    pub fn start_server() -> anyhow::Result<()> {
        anyhow::bail!("IPC is only supported on Windows")
    }
}
```

---

## 二、错误处理机制 (Error Handling)

生产代码路径中禁止出现未处理的 `.unwrap()` 或 `.expect()`（测试代码除外）。

### `core_engine`：领域错误枚举

使用 `thiserror` 定义精准的领域错误，所有 `core_engine` 公开 API 返回 `Result<T, EngineError>`：

```rust
use thiserror::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Asset not found: {0}")]
    AssetNotFound(PathBuf),

    #[error("Config invalid: {0}")]
    ConfigInvalid(String),

    #[error("Action '{0}' is not in the action whitelist")]
    UnknownAction(String),

    #[error("Invalid timeline event: timestamp {0}ms is out of range")]
    InvalidTimeline(u64),

    #[error("Animation dictionary has no entry for action: {0:?}")]
    MissingAnimation(Action),
}
```

### `cli_app`：应用层错误

使用 `anyhow::Result` 统一处理来自各层的错误冒泡，并与正确的 CLI 退出码绑定：

```rust
fn main() -> anyhow::Result<()> {
    // 错误自动转换并格式化输出给用户
}
```

---

## 三、日志规范 (Logging via `tracing`)

全项目使用 `tracing` 宏，禁止使用 `println!` 或 `eprintln!`。

| 级别 | 使用场景 |
|---|---|
| `TRACE` | 每帧调用的渲染数据、状态机内部的每一步转变 |
| `DEBUG` | LLM 返回的原始 JSON 响应体、资产文件完整路径 |
| `INFO` | 生命周期关键节点：应用启动、收到 Prompt、Timeline 播放开始/结束 |
| `WARN` | Action whitelist fallback 触发（含非法动作原始字符串）、帧率下降 |
| `ERROR` | LLM API 调用失败、IPC 连接断开、渲染上下文丢失 |

日志初始化在 `cli_app/src/main.rs` 中完成，使用 `tracing_subscriber::EnvFilter`，默认级别 `INFO`，可通过环境变量 `RUST_LOG` 覆盖。

---

## 四、代码风格 (Formatting & Linting)

### 格式化

强制启用 `rustfmt`，保持默认配置，CI 检查不通过则构建失败：

```bash
cargo fmt --all -- --check
```

### Lint

启用 `clippy` all + pedantic，所有 warning 视为 error：

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

在每个 Crate 的 `lib.rs` / `main.rs` 顶部声明：

```rust
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)] // 按需逐条豁免，需附注释说明原因
```

### 命名规范

| 场景 | 规范 |
|---|---|
| 结构体 / 枚举 / Trait | `CamelCase` |
| 函数 / 变量 / 模块 | `snake_case` |
| 常量 / 静态变量 | `SCREAMING_SNAKE_CASE` |

---

## 五、资产管理规范 (Asset Management)

### 目录结构

```
assets/
├── sprites/
│   ├── idle/     001.png  002.png  …
│   ├── walk/     001.png  …
│   ├── jump/     001.png  …
│   ├── attack/   001.png  …
│   ├── sleep/    001.png  …
│   ├── happy/    001.png  …
│   └── angry/    001.png  …
└── config/
    └── pet_config.json
```

帧文件命名格式：`{frame_number:03}.png`，从 `001` 开始，无前导零歧义。

### `pet_config.json` Schema

```json
{
  "pet_id": "pet1",
  "display_name": "Mochi",
  "window_size": [128, 128],
  "default_action": "idle",
  "actions": {
    "idle":   { "frame_count": 4, "frame_duration_ms": 200, "looped": true  },
    "walk":   { "frame_count": 6, "frame_duration_ms": 100, "looped": true  },
    "sleep":  { "frame_count": 2, "frame_duration_ms": 500, "looped": true  },
    "jump":   { "frame_count": 5, "frame_duration_ms": 80,  "looped": false },
    "attack": { "frame_count": 6, "frame_duration_ms": 80,  "looped": false },
    "happy":  { "frame_count": 4, "frame_duration_ms": 150, "looped": false },
    "angry":  { "frame_count": 4, "frame_duration_ms": 150, "looped": false }
  }
}
```

- 此文件由 `core_engine` 在启动时通过 `serde_json` 解析。
- 缺少 `actions` 中任何键，或字段类型不符，返回 `EngineError::ConfigInvalid`。
- 路径通过运行时 `std::env::current_dir()` 拼接，禁止硬编码绝对路径。

---

## 六、工具链与 CI 管理 (Toolchain & CI)

### 工具链锁定

项目根目录须包含 `rust-toolchain.toml`：

```toml
[toolchain]
channel  = "stable"
# 更新时修改此版本号，并在 commit message 中注明原因
version  = "1.78.0"
targets  = ["x86_64-pc-windows-msvc"]
```

### `Cargo.lock` 策略

`Cargo.lock` 必须提交到版本控制。本项目为二进制工程（非发布库），提交 lock 文件可保证构建的完全可复现性。

### 依赖版本策略

所有依赖使用 **major.minor** 精确约束，patch 版本浮动：

```toml
winit   = "0.30"   # 正确
pixels  = "0.14"   # 正确
winit   = "*"      # 禁止
winit   = ">=0.28" # 禁止
```

`cargo update` 是有意为之的操作，需 code review，不自动执行。

### CI Pipeline（GitHub Actions）

CI 运行在 `windows-latest`（项目目标平台）：

```yaml
# .github/workflows/ci.yml
jobs:
  ci:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Format check
        run: cargo fmt --all -- --check
      - name: Lint
        run: cargo clippy --all-targets --all-features -- -D warnings
      - name: Test core_engine
        run: cargo test -p core_engine
      - name: Build release
        run: cargo build --release
```

**CI 范围约束：** 窗口渲染测试、IPC 集成测试需要显示器环境，在 CI 中跳过。这类测试通过 `ci` feature 标记排除：

```toml
# cli_app/Cargo.toml
[features]
ci = []  # 启用时跳过需要 display/IPC 的测试
```

```rust
#[cfg(not(feature = "ci"))]
#[test]
fn test_window_transparency() { /* ... */ }
```
