/// winit + pixels 透明浮窗模块。
///
/// 实现要点（Task 3 完成后填充）：
/// - 透明、无边框、始终置顶的浮窗
/// - pixels 帧缓冲：仅写入非透明像素，其余保持全透明
/// - 主线程 ApplicationHandler 循环，通过 `rx` 接收来自 Tokio 任务的 AppMessage
///
/// # 当前状态
/// Task 3 stub：所有方法留待实现。
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::ipc::AppMessage;

/// 持有窗口渲染状态与跨线程消息接收端。
pub struct PetApp {
    /// 来自 Tokio 任务的消息通道接收端。
    pub rx: std::sync::mpsc::Receiver<AppMessage>,
}

impl ApplicationHandler for PetApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // TODO(Task 3): 创建透明置顶窗口，初始化 pixels 帧缓冲
        todo!("Task 3: create transparent window")
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
        // TODO(Task 7): 处理输入事件（点击、拖拽），驱动 FSM
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // TODO(Task 7): 排空 rx，推进 Timeline，更新 Animator，重绘帧
    }
}
