/// winit + pixels 透明浮窗模块。
///
/// 实现要点：
/// - 透明、无边框、始终置顶的浮窗
/// - pixels 帧缓冲：仅写入非透明像素，其余保持全透明
/// - 主线程 ApplicationHandler 循环，通过 `rx` 接收来自 Tokio 任务的 AppMessage
use std::sync::Arc;
use std::time::Instant;

use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use core_engine::animation::Animator;
use core_engine::state_machine::{FsmInput, PetState, StateMachine};
use core_engine::timeline::Timeline;

use crate::ipc::AppMessage;

/// 将 80x64 的精灵图居中缩放绘制到 128x128 缓冲。
/// 使用最近邻插值。
const SPRITE_WIDTH: usize = 80;
const SPRITE_HEIGHT: usize = 64;
const WINDOW_WIDTH: usize = 128;
const WINDOW_HEIGHT: usize = 128;

/// 持有窗口渲染状态与跨线程消息接收端。
pub struct PetApp {
    /// 来自 Tokio 任务的消息通道接收端。
    pub rx: std::sync::mpsc::Receiver<AppMessage>,
    /// winit 窗口对象。
    pub window: Option<Arc<Window>>,
    /// pixels 帧缓冲。
    pub pixels: Option<Pixels<'static>>,
    /// 动画引擎。
    pub animator: Animator,
    /// 状态机。
    pub state_machine: StateMachine,
    /// 时间轴调度器。
    pub timeline: Timeline,
    /// 上一帧的时间戳，用于计算 delta time。
    pub last_tick: Instant,
}

impl PetApp {
    pub fn new(rx: std::sync::mpsc::Receiver<AppMessage>, animator: Animator) -> Self {
        Self {
            rx,
            window: None,
            pixels: None,
            animator,
            state_machine: StateMachine::new(),
            timeline: Timeline::new(),
            last_tick: Instant::now(),
        }
    }

    /// 将 80x64 的精灵图绘制到 128x128 的 pixels 缓冲中（带缩放和居中）。
    fn draw_sprite(&mut self, delta: std::time::Duration) {
        let texture = match self.animator.update(delta) {
            Ok(t) => t,
            Err(err) => {
                tracing::error!("Animator update error: {}", err);
                return;
            }
        };

        let pixels = match &mut self.pixels {
            Some(p) => p,
            None => return,
        };

        let frame = pixels.frame_mut();
        // 1. 清空缓冲（全透明）
        frame.fill(0);

        // 2. 居中缩放绘制（最近邻插值）
        // 计算缩放：80->128 (x1.6), 64->128 (x2.0)
        // 实际上我们直接遍历目标像素 (128x128)，反查源像素 (80x64)
        for y in 0..WINDOW_HEIGHT {
            for x in 0..WINDOW_WIDTH {
                // 反查坐标
                let src_x = (x * SPRITE_WIDTH / WINDOW_WIDTH).min(SPRITE_WIDTH - 1);
                let src_y = (y * SPRITE_HEIGHT / WINDOW_HEIGHT).min(SPRITE_HEIGHT - 1);

                let pixel = texture.get_pixel(src_x as u32, src_y as u32);
                let idx = (y * WINDOW_WIDTH + x) * 4;
                frame[idx] = pixel[0];
                frame[idx + 1] = pixel[1];
                frame[idx + 2] = pixel[2];
                frame[idx + 3] = pixel[3];
            }
        }
    }
}

impl ApplicationHandler for PetApp {

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // 1. 创建透明、无边框、始终置顶的窗口
        let window_attributes = Window::default_attributes()
            .with_title("ai-pet")
            .with_transparent(true)
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes).expect("failed to create window"));
        self.window = Some(window.clone());

        // 使用 Box::leak 获取 'static 引用以满足 Pixels<'static> 的要求
        // 在桌面宠物应用中，主窗口生命周期即应用生命周期，此泄漏是可接受的。
        let leaked_arc: &'static Arc<Window> = Box::leak(Box::new(window));
        let window_static: &'static Window = leaked_arc;

        // 2. 初始化 pixels 帧缓冲
        let pixels = {
            let surface_texture = SurfaceTexture::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32, window_static);
            PixelsBuilder::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32, surface_texture)
                .clear_color(pixels::wgpu::Color::TRANSPARENT)
                .build()
                .expect("failed to create pixels")
        };
        self.pixels = Some(pixels);

        self.last_tick = Instant::now();
        tracing::info!("Window created and pixels initialized");
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                _event_loop.exit();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // 点击进入 Acting 状态（Task 7 要求）
                self.state_machine.apply(FsmInput::Click);
                if let Some(window) = &self.window {
                    let _ = window.drag_window();
                }
            }
            WindowEvent::RedrawRequested => {
                // RedrawRequested 仅负责渲染，更新逻辑放在 about_to_wait
                if let Some(pixels) = &mut self.pixels {
                    if let Err(err) = pixels.render() {
                        tracing::error!("pixels.render error: {}", err);
                        _event_loop.exit();
                    }
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_tick);
        self.last_tick = now;

        // 1. 处理来自 IPC/LLM 的消息
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::InjectTimeline(events) => {
                    self.timeline.push_events(events);
                    self.state_machine.apply(FsmInput::InjectTimeline);
                }
                AppMessage::LlmError(err) => {
                    tracing::error!("LLM Error: {}", err);
                    self.state_machine.apply(FsmInput::LlmError);
                }
                AppMessage::Shutdown => {
                    _event_loop.exit();
                    return;
                }
            }
        }

        // 2. 推进时间轴并更新动画动作
        let events = self.timeline.tick(delta);
        for event in events {
            self.animator.set_action(event.action);
        }

        if self.timeline.finished {
            self.state_machine.apply(FsmInput::TimelineFinished);
        }

        // 3. 根据 FSM 状态强制动作（例如 Acting 状态下可能需要特定动画）
        // 如果 Timeline 已空且处于 Idle，Animator 会保持当前动作（通常是 Idle）
        if self.state_machine.state() == PetState::Idle 
           && self.timeline.finished {
            self.animator.set_action(core_engine::scripting::Action::Idle);
        }

        // 4. 重绘
        self.draw_sprite(delta);
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

