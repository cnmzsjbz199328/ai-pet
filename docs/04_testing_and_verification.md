# AI Desktop Pet Theatre: Testing & Verification Plan

本项目涉及 LLM 的非确定性输出与 UI 渲染的强平台关联性，采用分层测试策略：**`core_engine` 全自动化，`cli_app` I/O 层手动验收为主**。

---

## 一、CI 测试范围

CI 仅运行 `core_engine` 的单元测试（不依赖显示器、网络或 IPC）：

```bash
cargo test -p core_engine
```

以下测试类型**不**在 CI 中运行，通过手动验收替代：

- 需要桌面渲染环境的窗口测试
- 需要运行中守护进程的 IPC 集成测试
- 需要真实 LLM API 的端到端测试

---

## 二、核心单元测试 (`core_engine`)

### 1. 状态机 (State Machine)

覆盖所有合法与非法的状态转移路径：

```rust
#[test]
fn idle_receives_inject_timeline_transitions_to_scripted() { ... }

#[test]
fn idle_receives_click_transitions_to_acting() { ... }

#[test]
fn acting_receives_inject_timeline_transitions_to_scripted() { ... }

#[test]
fn acting_receives_llm_error_returns_to_idle() { ... }

#[test]
fn scripted_receives_finished_returns_to_idle() { ... }

#[test]
fn drag_event_does_not_change_state_in_any_state() { ... }
```

### 2. Timeline 调度器

目标：100% 逻辑分支覆盖率。使用 mock delta time 推进时间轴。

```rust
#[test]
fn events_dispatched_at_correct_timestamp() {
    // 构造 3 个事件 @0ms, @500ms, @1000ms
    // 推进 delta 模拟时间，验证每次 tick 返回正确事件
}

#[test]
fn out_of_order_events_are_sorted_on_push() {
    // push [1000ms, 200ms, 500ms] 顺序的事件
    // 验证内部排序后正确按时序分发
}

#[test]
fn timeline_fires_finished_signal_after_last_event() { ... }

#[test]
fn empty_timeline_does_not_panic() { ... }
```

### 3. Action 白名单与 Fallback

```rust
#[test]
fn valid_action_strings_parse_correctly() {
    // "idle" -> Action::Idle, "walk" -> Action::Walk, 等
}

#[test]
fn unknown_action_string_falls_back_to_idle() {
    // "fly" -> Action::Idle (不返回 Err，记录 WARN)
}

#[test]
fn malformed_json_returns_err() {
    // 非 JSON 字符串 -> Err，不 panic
}

#[test]
fn valid_json_with_unknown_actions_produces_fallback_events() {
    // JSON 中包含 "fly"，该事件被 fallback 为 Idle，其余事件正常解析
}
```

### 4. `pet_config.json` 加载器

```rust
#[test]
fn valid_config_loads_all_actions() { ... }

#[test]
fn config_missing_action_key_returns_config_invalid_error() {
    // config 中缺少 "walk" 键 -> EngineError::ConfigInvalid
}

#[test]
fn config_malformed_json_returns_config_invalid_error() { ... }

#[test]
fn config_wrong_field_type_returns_config_invalid_error() {
    // "frame_count": "four" (字符串而非整数) -> EngineError::ConfigInvalid
}
```

### 5. Animator 帧计算

```rust
#[test]
fn animator_returns_correct_frame_at_boundary_time() {
    // 4 帧动画，每帧 200ms：
    // elapsed=0ms -> frame[0]
    // elapsed=200ms -> frame[1]
    // elapsed=799ms -> frame[3]
}

#[test]
fn looped_animation_wraps_frame_index() {
    // elapsed=800ms (第5帧位置) -> 循环回 frame[0]
}

#[test]
fn non_looped_animation_clamps_to_last_frame() {
    // elapsed 超过总时长 -> 保持在最后一帧
}

#[test]
fn set_action_resets_elapsed_time() { ... }
```

---

## 三、集成测试与 Mock (`cli_app`)

### LLM 接口 Mock

定义 `LlmClient` trait 实现解耦，使集成测试不依赖真实 API：

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_script(&self, prompt: &str) -> anyhow::Result<String>;
}

// 测试用实现
pub struct MockLlmClient {
    pub preset_response: String,
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn generate_script(&self, _prompt: &str) -> anyhow::Result<String> {
        Ok(self.preset_response.clone())
    }
}
```

### 集成测试覆盖链路

```
CLI play 命令 -> IPC 消息 -> MockLlmClient 响应 -> JSON 解析 -> Timeline 注入
```

此链路在 CI 中通过 `MockLlmClient` 自动运行（不需要 display），用 `#[cfg(not(feature = "ci"))]` 跳过窗口部分。

---

## 四、MVP 手动验收清单

以下项目需要在实际 Windows 桌面环境中逐项验收：

### 启动与渲染

- [ ] 执行 `ai-pet start`，桌面出现宠物浮窗
- [ ] 宠物背景完全透明，不显示白色/黑色矩形背景
- [ ] 宠物浮层在所有其他窗口之上（Always on Top）
- [ ] 宠物 Idle 动画循环播放，无闪烁
- [ ] 使用任务管理器确认：待机时 CPU < 5%，GPU 占用合理

### CLI 与 LLM 联动

- [ ] 保持 `ai-pet start` 运行，另开终端执行 `ai-pet play "走两步然后睡个觉"`
- [ ] 主进程日志输出：收到 Prompt → 请求 LLM → 解析 JSON → 开始播放
- [ ] 宠物准确执行 Walk → Sleep 的动画序列
- [ ] 序列结束后，宠物自动平滑切回 Idle 动画

### 健壮性与边界

- [ ] 断网状态下执行 `ai-pet play ...`：终端打印 ERROR 日志，桌面宠物不崩溃，保持 Idle
- [ ] 输入极端 prompt（如超长文本、纯符号）：不导致 panic，宠物保持 Idle
- [ ] LLM 返回非 JSON 文本：JSON 解析失败，WARN 日志，宠物保持 Idle
- [ ] 拖拽宠物：宠物随鼠标移动，松手后停留在新位置，动画不中断
