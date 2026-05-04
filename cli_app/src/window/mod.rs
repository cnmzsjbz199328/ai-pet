/// winit + pixels 透明浮窗模块。
///
/// 实现要点：
/// - 透明、无边框、始终置顶的浮窗
/// - pixels 帧缓冲：仅写入非透明像素，其余保持全透明
/// - 主线程 ApplicationHandler 循环，通过 `rx` 接收来自 Tokio 任务的 AppMessage
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId, WindowLevel};

use core_engine::animation::Animator;
use core_engine::scripting::Action;
use core_engine::state_machine::{FsmInput, PetState, StateMachine};
use core_engine::timeline::Timeline;

use crate::ipc::AppMessage;

/// 精灵原始尺寸（80×64px）。
const SPRITE_WIDTH: usize = 80;
const SPRITE_HEIGHT: usize = 64;
/// 浮窗尺寸（128×128px）。
const WINDOW_WIDTH: usize = 128;
const WINDOW_HEIGHT: usize = 128;
/// 鼠标移动超过此距离（物理像素）才视为拖拽，否则视为点击。
const DRAG_THRESHOLD: f64 = 5.0;

/// 持有窗口渲染状态与跨线程消息接收端。
pub struct PetApp {
    /// 来自 Tokio 任务的消息通道接收端。
    pub rx: std::sync::mpsc::Receiver<AppMessage>,
    window: Option<Arc<Window>>,
    /// pixels 帧缓冲。
    ///
    /// `pixels 0.17` 的 `Pixels<'surf>` 通过 `raw-window-handle 0.6` 持有窗口引用，
    /// 而 `ApplicationHandler` 要求所有状态自含于结构体（自引用结构体在安全 Rust 中
    /// 无法表达），因此使用 `'static` 生命周期。
    ///
    /// 对应地，`try_init_window` 中通过 `Box::leak` 将 `Arc<Window>` 提升为
    /// `'static` 引用：这是**一次性有界泄漏**（固定大小，随进程退出回收），
    /// 对主窗口生命周期等于应用生命周期的场景是可接受的权衡。
    pixels: Option<Pixels<'static>>,
    pub animator: Animator,
    state_machine: StateMachine,
    timeline: Timeline,
    last_tick: Instant,
    /// 上一帧 `timeline.finished` 的值，用于检测 false→true 跳变，
    /// 确保 `TimelineFinished` 事件只发送一次。
    timeline_was_finished: bool,
    // --- 鼠标状态（用于区分点击与拖拽）---
    mouse_left_pressed: bool,
    mouse_press_pos: Option<PhysicalPosition<f64>>,
    is_dragging: bool,
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
            timeline_was_finished: true,
            mouse_left_pressed: false,
            mouse_press_pos: None,
            is_dragging: false,
        }
    }

    /// 窗口创建与 pixels 初始化的内部实现，使用 `?` 传播错误。
    /// 由 `resumed` 调用；出错时记录日志并退出事件循环。
    fn try_init_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window_attributes = Window::default_attributes()
            .with_title("ai-pet")
            .with_transparent(true)
            .with_decorations(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes)?);

        // pixels 0.17 要求 SurfaceTexture 持有 &'surf W（raw-window-handle 0.6），
        // 且 Pixels<'surf> 将该生命周期传递给 wgpu Surface。
        // ApplicationHandler 不支持自引用结构体，故将 Arc<Window> 克隆一份后
        // 通过 Box::leak 提升为 &'static Window（一次性有界泄漏，随进程退出回收）。
        let window_ref: &'static Window = Box::leak(Box::new(Arc::clone(&window)));
        let pixels = {
            let surface_texture =
                SurfaceTexture::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32, window_ref);
            PixelsBuilder::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32, surface_texture)
                .wgpu_backend(pixels::wgpu::Backends::GL) // DX12 在 Poll 模式下 get_current_texture() 死锁
                .clear_color(pixels::wgpu::Color::TRANSPARENT)
                .build()?
        };

        self.pixels = Some(pixels);
        self.window = Some(window);
        self.last_tick = Instant::now();
        tracing::info!("Window created and pixels initialized");
        Ok(())
    }

    /// 将当前动画帧写入 pixels 缓冲（最近邻插值，80×64 → 128×128）。
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
        // 清空为全透明
        frame.fill(0);

        // 遍历目标像素，反查源像素（最近邻）
        for y in 0..WINDOW_HEIGHT {
            for x in 0..WINDOW_WIDTH {
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
    /// 每次事件循环迭代开始时调用，确保 Poll 模式持续生效。
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(err) = self.try_init_window(event_loop) {
            tracing::error!("Failed to initialize window: {}", err);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            // 区分点击与拖拽：
            // - 按下时记录起始位置；
            // - CursorMoved 超过阈值时开始拖拽；
            // - 释放时若未触发拖拽则视为点击，驱动 FSM。
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                match state {
                    ElementState::Pressed => {
                        self.mouse_left_pressed = true;
                        self.mouse_press_pos = None;
                        self.is_dragging = false;
                    }
                    ElementState::Released => {
                        if self.mouse_left_pressed && !self.is_dragging {
                            self.state_machine.apply(FsmInput::Click);
                        }
                        self.mouse_left_pressed = false;
                        self.mouse_press_pos = None;
                        self.is_dragging = false;
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if self.mouse_left_pressed && !self.is_dragging {
                    let over_threshold = match self.mouse_press_pos {
                        Some(start) => {
                            let dx = position.x - start.x;
                            let dy = position.y - start.y;
                            (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD
                        }
                        None => {
                            self.mouse_press_pos = Some(position);
                            false
                        }
                    };
                    if over_threshold {
                        self.is_dragging = true;
                        self.state_machine.apply(FsmInput::Drag);
                        if let Some(window) = &self.window {
                            let _ = window.drag_window();
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(pixels) = &mut self.pixels {
                    if let Err(err) = pixels.render() {
                        tracing::error!("pixels.render error: {}", err);
                        event_loop.exit();
                    }
                }
            }

            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_tick);
        self.last_tick = now;

        // 1. 排空消息队列
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
                    event_loop.exit();
                    return;
                }
            }
        }

        // 2. 推进时间轴，将到期事件的动作同步到 Animator
        let due = self.timeline.tick(delta);
        for event in due {
            self.animator.set_action(event.action);
        }

        // 3. 仅在 finished 发生 false→true 跳变时通知 FSM，避免每帧重复触发
        let just_finished = !self.timeline_was_finished && self.timeline.finished;
        if just_finished {
            self.state_machine.apply(FsmInput::TimelineFinished);
        }
        self.timeline_was_finished = self.timeline.finished;

        // 4. Idle 状态下确保动画回到 idle 动作
        if self.state_machine.state() == PetState::Idle && self.timeline.finished {
            self.animator.set_action(Action::Idle);
        }

        // 5. 更新帧缓冲并直接渲染（不依赖 RedrawRequested：
        //    Windows Poll 模式下 WM_PAINT 低优先级会被饿死）
        self.draw_sprite(delta);
        if let Some(pixels) = &mut self.pixels {
            if let Err(err) = pixels.render() {
                tracing::error!("pixels.render error: {}", err);
                event_loop.exit();
            }
        }
    }
}
