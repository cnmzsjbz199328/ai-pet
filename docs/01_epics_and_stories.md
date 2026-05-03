# AI Desktop Pet Theatre: Epics & User Stories

## 产品愿景 (Product Vision)

构建一个由 AI（大语言模型）驱动的桌面宠物剧场系统。该系统以 CLI 为主要驱动入口，结合独立的桌面渲染浮窗，实现宠物在桌面的自主交互与 AI 生成剧本的动画表演。它是一个轻量级、高复用的动画编排与时间轴调度引擎。

---

## 核心史诗任务 (Epics)

### Epic 1: 基础工程架构与 CLI 脚手架

建立整个项目的基础工程结构，将项目拆分为核心引擎 Crate（`core_engine`）与可执行程序 Crate（`cli_app`），并实现 CLI 的基础指令流。

- 采用 Cargo Workspace 模式管理依赖，两个 Crate 职责严格隔离。
- 实现 CLI 对应用生命周期的控制：`ai-pet start`（启动）、`ai-pet play "<prompt>"`（触发剧本）、`ai-pet stop`（退出）。
- 构建统一的日志流（`tracing`）和错误处理链路（`thiserror` + `anyhow`）。

### Epic 2: 渲染层与基础动画系统 (Animation Engine)

构建一套基于精灵图（Sprite / PNG 序列帧）的帧动画系统，在独立透明桌面窗口中渲染。

- 使用 **winit 0.30** 创建无边框、背景透明、永远置顶（Always on Top）的浮窗。
- 使用 **pixels 0.14** 作为帧缓冲渲染器，**image 0.25** 解码 PNG 序列帧至内存。
- 实现以 `std::time::Instant` 为驱动的逐帧播放与循环机制，delta time 类型为 `std::time::Duration`。

### Epic 3: 时间轴调度与状态机 (Timeline & State Machine)

系统的核心导演模块，管理宠物当前状态并按时间序列精确投递动作事件。

- 实现核心的有限状态机（FSM）：`Idle → Acting → Scripted → Idle`。
  - `Idle`：默认循环待机动画。
  - `Acting`：等待 LLM 响应中（已接收指令，尚未获得剧本）。
  - `Scripted`：按时间轴播放 AI 生成的动作序列。
- 实现 `Timeline Scheduler`，以毫秒（`u64`）为精度按时分发 `TimelineEvent`。
- 实现动作字典（`HashMap<Action, Animation>`）；未知动作 fallback 为 `Action::Idle` 并记录 `WARN` 日志。

### Epic 4: AI 大语言模型接入与剧本编译 (LLM & Script Parser)

连接远端 LLM，将用户的自然语言输入转化为可被时间轴系统执行的 JSON 结构化剧本。

- 通过 **reqwest 0.12**（rustls-tls feature）接入 OpenAI API（或 DeepSeek 等兼容接口）。
- 使用强约束的 System Prompt Template，要求 LLM 输出纯 JSON，不附带任何说明文字。
- 使用 **serde_json** 实现 JSON 解析器；任何不合规字段（包括未知 `action`）均通过白名单过滤，非法输入 fallback 而非 panic。

---

## 用户故事 (User Stories)

### Epic 1 (CLI & 架构)

- **US 1.1** 作为开发者，我希望系统拆分为 `core_engine` 和 `cli_app` 两个 Crate，以便未来可以单独复用引擎逻辑。
- **US 1.2** 作为用户，我希望能够通过 `ai-pet start` 启动桌面宠物浮窗，窗口显示后 CLI 命令返回，宠物以守护进程形式持续运行。
- **US 1.3** 作为用户，我希望能够在另一个终端通过 `ai-pet play "让他们打一架"` 直接下达表演指令，无需操作界面。

### Epic 2 (渲染层)

- **US 2.1** 作为用户，我希望宠物出现时背景完全透明，不遮挡正常工作区域。
- **US 2.2** 作为用户，我希望动画能以稳定帧率（目标 60 FPS）平滑播放 PNG 序列帧，无闪烁。
- **US 2.3** 作为用户，我可以用鼠标拖拽宠物移动其桌面位置；点击宠物时进入 `Acting` 状态（等待指令）。

### Epic 3 (时间轴调度)

- **US 3.1** 作为系统，当接收到一个 10 秒钟的剧本时，能精准按毫秒调度，在指定时间点切换宠物动作。
- **US 3.2** 作为系统，当动作队列（Timeline）播放完毕后，自动将宠物重置为 `Idle` 状态动画。

### Epic 4 (AI 剧本)

- **US 4.1** 作为用户，当我输入"让宠物A对宠物B生气"时，系统应在后台生成一段 10 秒内包含 `angry`、`walk`、`attack` 等动作的 JSON 剧本并自动播放。
- **US 4.2** 作为开发者，当 AI 输出了白名单外的动作（如 `"fly"`）时，系统忽略该事件节点并 fallback 为 `Action::Idle`，不导致程序崩溃。

---

## MVP 验收标准

1. 支持单只宠物（`pet1`）。
2. 通过 `ai-pet start` 在桌面显示透明浮窗宠物，背景透明，始终置顶。
3. 通过 `ai-pet play "<prompt>"` 在另一终端输入自然语言指令。
4. 系统调用 LLM 生成剧情 JSON，成功解析后灌入 Timeline 引擎。
5. 宠物按剧本时间序列准确播放对应 Sprite 动画。
6. 表演结束后平滑切回 Idle 状态。
7. 断网情况下优雅降级：宠物不崩溃，保持 Idle，终端输出错误日志。
