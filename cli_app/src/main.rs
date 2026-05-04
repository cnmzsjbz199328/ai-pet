#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod ai;
mod config;
mod ipc;
mod window;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use winit::event_loop::{ControlFlow, EventLoop};

use core_engine::animation::{Animation, Animator};
use core_engine::scripting::Action;
use crate::ai::{LlmClient, MockLlmClient, OpenAiClient};
use crate::config::PetConfig;
use crate::window::PetApp;

#[derive(Parser)]
#[command(name = "ai-pet", about = "AI Desktop Pet Theatre")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 启动桌面宠物守护进程（透明浮窗 + IPC 服务端）
    Start,
    /// 向运行中的守护进程发送表演指令
    Play {
        /// 自然语言剧本指令，如 "走两步然后睡觉"
        prompt: String,
    },
    /// 关闭守护进程
    Stop,
}

fn main() -> Result<()> {
    // 初始化日志：默认 INFO 级别，可通过 RUST_LOG 环境变量覆盖
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Start => {
            tracing::info!("Starting ai-pet daemon...");
            
            // 1. 加载配置
            let config_path = Path::new("assets/config/pet_config.json");
            let config_json = std::fs::read_to_string(config_path)
                .with_context(|| format!("Failed to read config: {:?}", config_path))?;
            let config: PetConfig = serde_json::from_str(&config_json)?;

            // 2. 加载精灵图
            let animations = load_all_animations(&config, Path::new("assets/sprites"))?;
            let animator = Animator::new(animations);

            // 3. 设置 LLM 客户端
            let api_key = std::env::var("OPENAI_API_KEY").ok();
            let llm: Arc<dyn LlmClient> = if let Some(key) = api_key {
                tracing::info!("Using OpenAI LLM client");
                Arc::new(OpenAiClient::new(key))
            } else {
                tracing::warn!("OPENAI_API_KEY not found, falling back to Mock LLM client");
                Arc::new(MockLlmClient {
                    preset_response: r#"{
                        "characters": ["pet1"],
                        "events": [
                            {"timestamp_ms": 0, "actor_id": "pet1", "action": "happy"},
                            {"timestamp_ms": 1000, "actor_id": "pet1", "action": "idle"}
                        ]
                    }"#.to_string()
                })
            };

            // 4. 创建跨线程通道
            let (tx, rx) = std::sync::mpsc::channel();

            // 5. 启动 Tokio 运行时并运行 IPC 服务端
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            
            let tx_clone = tx.clone();
            let llm_clone = llm.clone();
            runtime.spawn(async move {
                if let Err(err) = ipc::windows::run_server(tx_clone, llm_clone).await {
                    tracing::error!("IPC Server error: {}", err);
                }
            });

            // 6. 运行 winit 事件循环（Poll 模式：持续轮询，驱动动画游戏循环）
            let event_loop = EventLoop::new()?;
            event_loop.set_control_flow(ControlFlow::Poll);
            let mut app = PetApp::new(rx, animator);
            event_loop.run_app(&mut app).context("Event loop failed")
        }
        Command::Play { prompt } => {
            tracing::info!(%prompt, "Sending play command to daemon");
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(ipc::windows::send_prompt(&prompt))
        }
        Command::Stop => {
            tracing::info!("Sending stop command to daemon");
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(ipc::windows::send_prompt("__shutdown__"))
        }
    }
}

fn load_all_animations(pet_config: &PetConfig, sprites_dir: &Path) -> Result<HashMap<Action, Animation>> {
    let mut animations = HashMap::new();

    for (action_name, action_cfg) in &pet_config.actions {
        let action = Action::from_str_or_fallback(action_name);
        
        // 避开 Unknown 映射
        if action_name != "idle" && action == Action::Idle {
            tracing::warn!("Skipping unknown action config: {}", action_name);
            continue;
        }

        let mut frames = Vec::new();
        for i in 1..=action_cfg.frame_count {
            let file_name = format!("{:03}.png", i);
            let path = sprites_dir.join(action_name).join(file_name);
            
            let img = image::open(&path)
                .with_context(|| format!("Failed to load sprite: {:?}", path))?
                .to_rgba8();
            frames.push(Arc::new(img));
        }

        animations.insert(action, Animation {
            frames,
            frame_duration: Duration::from_millis(action_cfg.frame_duration_ms),
            looped: action_cfg.looped,
        });
    }

    if !animations.contains_key(&Action::Idle) {
        anyhow::bail!("Config must contain 'idle' action");
    }

    Ok(animations)
}
