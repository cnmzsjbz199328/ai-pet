# AI Desktop Pet Theatre — 剩余开发计划

> 本文档覆盖 Task 3 至 Task 11，供审查与排期。  
> 当前状态：Task 1–6 已完成，代码已提交（commit `c7204a6`）。

---

## 整体任务依赖图

```
Task 3 (浮窗) ──┐
Task 4 ✅        ├──► Task 7 (主循环集成) ──┐
Task 5 ✅        │                          ├──► Task 9 (CLI) ──► Task 11 (验收)
Task 6 ✅ ───────┘                          │
Task 8 (IPC) ──────────────────────────────┘
Task 10 (LLM) ─────────────────────────────────────► Task 11
```

---

## Task 3 — 透明无边框 Always-on-Top 浮窗

**文件：** `cli_app/src/window/mod.rs`  
**依赖：** Task 2 ✅  
**阻塞：** Task 7

### 目标
用 winit 0.30 创建一个透明、无边框、始终置顶的浮窗，挂载 pixels 帧缓冲，能正确渲染半透明 RGBA 精灵图。

### 技术要点

#### 窗口创建（winit 0.30 ApplicationHandler API）
```rust
// resumed() 中创建窗口
let window = event_loop.create_window(
    Window::default_attributes()
        .with_title("ai-pet")
        .with_transparent(true)
        .with_decorations(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_inner_size(LogicalSize::new(128u32, 128u32))
        .with_resizable(false),
)?;
```

#### pixels 帧缓冲初始化
```rust
let pixels = {
    let surface_texture = SurfaceTexture::new(128, 128, &window);
    PixelsBuilder::new(128, 128, surface_texture)
        .clear_color(wgpu::Color::TRANSPARENT)
        .build()?
};
```

#### RGBA 精灵合成
- 帧缓冲每帧先全部清零（透明）
- 将当前 `Texture`（80×64 RGBA）按比例缩放后居中绘制到 128×128 缓冲
- 透明像素（alpha=0）直接写 0，不覆盖桌面内容

#### 窗口拖拽
- 监听 `WindowEvent::MouseInput`（左键按下） + `WindowEvent::CursorMoved`
- 通过 `window.drag_window()` 实现无边框拖拽（Windows API 原生支持）

### 验收标准
- [ ] 启动后在桌面右下角出现 128×128 透明浮窗
- [ ] 猫咪 idle 动画正确播放，背景完全透明（可透过窗口看到桌面）
- [ ] 拖拽窗口可移动，始终置顶于其他应用上方
- [ ] 关闭程序后窗口消失，无残留进程

---

## Task 7 — 主循环集成

**文件：** `cli_app/src/window/mod.rs`（扩充 `PetApp`）  
**依赖：** Task 3, Task 4 ✅, Task 5 ✅, Task 6 ✅  
**阻塞：** Task 9

### 目标
在 winit `about_to_wait` 回调中实现完整游戏循环：收消息 → 推进时间轴 → 更新状态机 → 播放动画 → 渲染帧。

### PetApp 完整结构
```rust
pub struct PetApp {
    rx: std::sync::mpsc::Receiver<AppMessage>,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels>,
    animator: Animator,
    state_machine: StateMachine,
    timeline: Timeline,
    last_tick: std::time::Instant,
}
```

### 每帧逻辑（about_to_wait）
```
1. delta = Instant::now() - last_tick; last_tick = now
2. 排空 rx（非阻塞 try_recv 循环）:
   - InjectTimeline(events) → timeline.push_events(events)
                             → state_machine.apply(FsmInput::InjectTimeline)
   - LlmError(msg)          → tracing::error!(msg)
                             → state_machine.apply(FsmInput::LlmError)
   - Shutdown               → event_loop.exit()
3. due = timeline.tick(delta)
   - 对每个 due event: animator.set_action(event.action)
   - if timeline.finished → state_machine.apply(FsmInput::TimelineFinished)
4. texture = animator.update(delta)?
5. 将 texture 绘制到 pixels 缓冲（含缩放）
6. pixels.render()?
```

### 精灵缩放绘制
精灵原始尺寸 80×64，窗口 128×128：
- 缩放比：`scale_x = 128/80 = 1.6`，`scale_y = 128/64 = 2.0`
- 使用最近邻插值（像素风格不做双线性插值）
- 居中偏移：`offset_x = (128 - 80*scale) / 2`

### 输入事件映射
| winit 事件 | FsmInput |
|---|---|
| `MouseButton::Left` 按下 | `Click` |
| `CursorMoved`（左键持续按下） | `Drag` |

### 验收标准
- [ ] 动画帧按 `frame_duration_ms` 节拍正确切换
- [ ] 收到 `InjectTimeline` 后立即开始播放新剧本
- [ ] `Timeline::finished` 后 FSM 回到 Idle，动画切回 idle
- [ ] 点击宠物 FSM 进入 Acting 状态（等待 LLM 响应）
- [ ] 帧率稳定在 ≥30fps，无明显卡顿

---

## Task 8 — Windows Named Pipe IPC

**文件：** `cli_app/src/ipc/mod.rs`  
**依赖：** Task 2 ✅  
**阻塞：** Task 9

### 目标
实现守护进程（服务端）和 CLI 客户端（`play` / `stop` 子命令）之间的进程间通信。

### 线缆格式（已定义）
```
[4-byte u32 LE 长度][UTF-8 JSON body]
```

消息体格式：
```json
{"prompt": "走两步然后睡觉"}
```

### 服务端实现（run_server）
```rust
pub async fn run_server(
    tx: std::sync::mpsc::Sender<AppMessage>,
    llm: Arc<dyn LlmClient>,
) -> Result<()> {
    loop {
        // 1. ServerOptions::new().create(PIPE_NAME)?
        // 2. server.connect().await?  (等待客户端连接)
        // 3. 读一条消息: read_message(&mut server).await?
        // 4. 解析 JSON -> prompt 字符串
        // 5. 判断 prompt == "__shutdown__" -> tx.send(AppMessage::Shutdown)
        // 6. 否则: spawn 异步任务:
        //      let json = llm.generate_script(&prompt).await?
        //      let events = parse_script(&json)?
        //      tx.send(AppMessage::InjectTimeline(events))
        //    错误时: tx.send(AppMessage::LlmError(err.to_string()))
        // 7. 继续循环（重新创建 pipe，等待下一个客户端）
    }
}
```

**关键注意点：**
- Named Pipe 服务端每次 `connect` 只服务一个客户端，断开后需重新 `ServerOptions::new().create()` 再 `connect`
- LLM 调用必须在独立 `tokio::spawn` 中执行，不能阻塞 pipe accept 循环
- `tx.send` 失败（主线程已退出）时直接 `return Ok(())`

### 客户端实现（send_prompt）
```rust
pub async fn send_prompt(prompt: &str) -> Result<()> {
    // 1. ClientOptions::new().open(PIPE_NAME)? （服务端未启动则返回 Err）
    // 2. write_message(&mut client, &json!({ "prompt": prompt }).to_string()).await?
    // 3. client flush + 关闭
}
```

### 验收标准
- [ ] `ai-pet play "跳一下"` 成功将消息写入管道
- [ ] 守护进程收到消息后触发 LLM 调用（或 MockLlm 响应）
- [ ] `ai-pet stop` 发送 `__shutdown__` 后守护进程正常退出
- [ ] 守护进程未启动时，`play` 命令打印明确错误，退出码非 0

---

## Task 9 — CLI 入口（clap subcommands）

**文件：** `cli_app/src/main.rs`  
**依赖：** Task 7, Task 8  
**阻塞：** Task 11

### 目标
填充 `main.rs` 中三个 `todo!()` 桩，让 `start` / `play` / `stop` 子命令正确运作。

### start 子命令
```rust
Command::Start => {
    // 1. 读取 assets/config/pet_config.json
    // 2. 加载所有精灵帧到 HashMap<Action, Animation>
    // 3. 创建 Animator
    // 4. 从环境变量 OPENAI_API_KEY 读取 key，创建 OpenAiClient（或 MockLlmClient）
    // 5. 创建 mpsc::channel
    // 6. 启动 tokio runtime（tokio::runtime::Builder::new_multi_thread）
    // 7. runtime.spawn(ipc::windows::run_server(tx, llm))
    // 8. 创建 PetApp { rx, animator, ... }
    // 9. EventLoop::new()?.run_app(&mut pet_app)
}
```

**线程模型要点：**
- `EventLoop::run_app` 必须在主线程调用（Windows 要求）
- tokio runtime 用 `Arc<Runtime>` 持有，在 `PetApp` 中不直接使用 `#[tokio::main]`

### play 子命令
```rust
Command::Play { prompt } => {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(ipc::windows::send_prompt(&prompt))
}
```

### stop 子命令
```rust
Command::Stop => {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(ipc::windows::send_prompt("__shutdown__"))
}
```

### 资产加载逻辑
```rust
fn load_sprites(config: &PetConfig, assets_dir: &Path) -> Result<HashMap<Action, Animation>> {
    // 对每个 action:
    //   读取 assets/sprites/<action>/001.png … NNN.png
    //   image::open(path)?.to_rgba8() -> Arc<RgbaImage>
    //   frames 不足 frame_count -> EngineError::AssetNotFound
    //   构造 Animation { frames, frame_duration, looped }
}
```

### 验收标准
- [ ] `cargo run -- start` 启动窗口，idle 动画播放
- [ ] `cargo run -- play "..."` 在运行中守护进程上生效
- [ ] `cargo run -- stop` 关闭守护进程，窗口消失
- [ ] 缺少 `OPENAI_API_KEY` 时打印友好提示（降级为 Mock 或报错）

---

## Task 10 — LLM 接入与剧本解析器

**文件：** `cli_app/src/ai/mod.rs`  
**依赖：** Task 2 ✅  
**阻塞：** Task 11

### 目标
实现 `parse_script()` 和 `OpenAiClient::generate_script()`，完成从自然语言到 `Vec<TimelineEvent>` 的完整链路。

### parse_script 实现
```rust
pub fn parse_script(json: &str) -> Result<Vec<TimelineEvent>> {
    // 1. serde_json::from_str::<serde_json::Value>(json)?  // 非法 JSON -> Err
    // 2. 取 value["events"] 数组
    // 3. 对每个元素:
    //      timestamp_ms: u64
    //      actor_id: String
    //      action: Action::from_str_or_fallback(&action_str)  // 未知 -> Idle + warn
    // 4. 返回 Vec<TimelineEvent>
}
```

### LLM System Prompt（已定稿）
```
You are a cat animation director. Output ONLY valid JSON, no markdown, no explanation.
Format:
{
  "characters": ["pet1"],
  "events": [
    {"timestamp_ms": 0,    "actor_id": "pet1", "action": "idle"},
    {"timestamp_ms": 2000, "actor_id": "pet1", "action": "walk"}
  ]
}
Action whitelist: idle walk jump attack sleep happy angry
Unknown actions are forbidden. Total duration should not exceed 30 seconds.
```

### OpenAiClient::generate_script 实现
```rust
async fn generate_script(&self, prompt: &str) -> Result<String> {
    let body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user",   "content": prompt}
        ],
        "temperature": 0.7
    });
    let resp = self.http
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&self.api_key)
        .json(&body)
        .send().await?
        .error_for_status()?
        .json::<serde_json::Value>().await?;

    resp["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("Unexpected OpenAI response shape"))
}
```

### 验收标准
- [ ] 合法 JSON 输入正确解析为 `Vec<TimelineEvent>`
- [ ] 未知 action 字段 fallback 为 Idle，不返回 Err，打印 WARN 日志
- [ ] 非 JSON 字符串返回 `Err`
- [ ] OpenAI 调用返回的 JSON body 能正确提取 content 字段
- [ ] HTTP 错误（401/429/500）通过 `error_for_status()` 转为 anyhow Err

---

## Task 11 — 端到端联调与 MVP 手动验收

**依赖：** Task 9, Task 10  
**负责人：** 开发者手动执行

### 验收清单

#### 启动与渲染
- [ ] `cargo run -- start` 在桌面出现透明猫咪浮窗
- [ ] idle 动画以正确帧率循环播放
- [ ] 窗口置顶，可拖拽，背景透明

#### IPC 通信
- [ ] `cargo run -- play "跳一下然后睡觉"` 发送成功，守护进程日志可见
- [ ] `cargo run -- stop` 守护进程优雅退出，窗口消失

#### LLM → 动画链路
- [ ] 发送自然语言指令后，LLM 返回 JSON，动画按脚本顺序播放
- [ ] 脚本播放完毕后自动回到 idle 状态
- [ ] 点击宠物触发 Acting 状态（等待 LLM），LLM 返回后开始播放

#### 降级与错误处理
- [ ] 无 `OPENAI_API_KEY` 时启动不崩溃，使用 Mock 响应或打印明确提示
- [ ] LLM 返回非法 JSON 时记录 ERROR 日志，FSM 回到 Idle，不崩溃
- [ ] IPC 客户端在守护进程未启动时打印明确错误，退出码为 1

#### 性能
- [ ] 动画帧率 ≥ 30fps（通过 RUST_LOG=debug 观察 tick 间隔）
- [ ] 内存占用 < 50MB（Task Manager 观察）

---

## 开发顺序建议

| 顺序 | Task | 预计工作量 | 关键风险 |
|---|---|---|---|
| 1 | **Task 3** 透明浮窗 | 中 | winit 0.30 API 与 pixels 集成的透明度配置 |
| 2 | **Task 10** LLM 解析器 | 小 | parse_script 单元测试覆盖 edge case |
| 3 | **Task 8** IPC 服务端 | 中 | Named Pipe 重连逻辑、跨线程 tx 生命周期 |
| 4 | **Task 7** 主循环集成 | 中 | delta time 精度、精灵缩放算法 |
| 5 | **Task 9** CLI 入口 | 小 | 资产路径解析（相对 vs 绝对） |
| 6 | **Task 11** 端到端验收 | — | 需要真实 Windows 显示器环境 |

> Task 10 可与 Task 3 并行推进，互不依赖。

---

## 前置环境确认

在开始 Task 3 之前，请确认以下环境已就绪：

```bash
# 确认 Rust 工具链已安装
rustup show

# 确认目标平台
rustup target list --installed | grep msvc

# 首次构建（会拉取全部依赖，需要网络）
cargo build
```

若 `rustup` 未安装，从 https://rustup.rs 下载安装程序，安装完成后重启终端。
