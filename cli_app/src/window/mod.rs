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
use core_engine::state_machine::StateMachine;
use core_engine::timeline::Timeline;

use crate::ipc::AppMessage;

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
    fn draw_sprite(&mut self) {
        let pixels = match &mut self.pixels {
            Some(p) => p,
            None => return,
        };

        let frame = pixels.frame_mut();
        // 1. 清空缓冲（全透明）
        frame.fill(0);

        // TODO(Task 7): 从 animator 获取当前纹理并绘制
        // 目前先留白，Task 3 要求的是窗口创建和基础渲染结构
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
            .with_inner_size(LogicalSize::new(128u32, 128u32))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes).expect("failed to create window"));
        self.window = Some(window.clone());

        // 使用 Box::leak 获取 'static 引用以满足 Pixels<'static> 的要求
        // 在桌面宠物应用中，主窗口生命周期即应用生命周期，此泄漏是可接受的。
        let leaked_arc: &'static Arc<Window> = Box::leak(Box::new(window));
        let window_static: &'static Window = leaked_arc;

        // 2. 初始化 pixels 帧缓冲
        let pixels = {
            let surface_texture = SurfaceTexture::new(128, 128, window_static);
            PixelsBuilder::new(128, 128, surface_texture)
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
                if let Some(window) = &self.window {
                    let _ = window.drag_window();
                }
            }
            WindowEvent::RedrawRequested => {
                self.draw_sprite();
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
        // TODO(Task 7): 排空 rx，推进 Timeline，更新 Animator，重绘帧
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

