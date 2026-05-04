#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod ai;
mod config;
mod ipc;
mod window;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use winit::event_loop::{ControlFlow, EventLoop};

use crate::ai::{LlmClient, MockLlmClient, OpenAiClient};
use crate::config::PetConfig;
use crate::window::PetApp;
use core_engine::animation::{Animation, Animator};
use core_engine::scripting::Action;

#[derive(Parser)]
#[command(name = "ai-pet", about = "AI Desktop Pet Theatre")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 启动桌面宠物守护进程（透明浮窗 + IPC 服务端）
    Start {
        /// [内部] 表示当前进程已是守护子进程，跳过 detach 步骤
        #[arg(long, hide = true)]
        daemon: bool,
    },
    /// 向运行中的守护进程发送表演指令
    Play {
        /// 自然语言剧本指令，如 "走两步然后睡觉"
        prompt: String,
    },
    /// 关闭守护进程
    Stop,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Start { daemon } => {
            if daemon {
                // ── 守护子进程路径：初始化日志后运行完整窗口循环 ──────────
                init_daemon_logging()?;
                tracing::info!("ai-pet daemon starting (child process)...");
                run_daemon()
            } else {
                // ── 父进程路径：仅初始化控制台日志，detach 后立即返回 ────
                init_console_logging();
                spawn_daemon_process()
            }
        }
        Command::Play { prompt } => {
            init_console_logging();
            tracing::info!(%prompt, "Sending play command to daemon");
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(ipc::windows::send_prompt(&prompt))
        }
        Command::Stop => {
            init_console_logging();
            tracing::info!("Sending stop command to daemon");
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(ipc::windows::send_prompt("__shutdown__"))
        }
    }
}

// ---------------------------------------------------------------------------
// 日志初始化
// ---------------------------------------------------------------------------

/// 控制台日志（父进程 / play / stop 命令使用）
fn init_console_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

/// 文件日志（守护子进程使用）：写入 ~/.ai-pet/daemon.log
fn init_daemon_logging() -> Result<()> {
    use std::fs;
    use tracing_subscriber::fmt::writer::BoxMakeWriter;

    let log_dir = daemon_log_path()?;
    if let Some(parent) = log_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_dir)
        .with_context(|| format!("Cannot open daemon log: {:?}", log_dir))?;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(BoxMakeWriter::new(log_file))
        .with_ansi(false) // 文件中不输出 ANSI 颜色转义码
        .init();

    Ok(())
}

/// 返回守护进程日志文件路径：`~/.ai-pet/daemon.log`
fn daemon_log_path() -> Result<PathBuf> {
    let home =
        dirs_next::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".ai-pet").join("daemon.log"))
}

// ---------------------------------------------------------------------------
// 守护进程化：spawn 自身后立即返回
// ---------------------------------------------------------------------------

/// 父进程调用：以 `start --daemon` 参数 detach 启动自身子进程，然后立即返回。
fn spawn_daemon_process() -> Result<()> {
    let exe = std::env::current_exe().context("Cannot determine current executable path")?;

    // 将当前工作目录传给子进程，保证相对路径（assets/）能正确解析
    let cwd = std::env::current_dir().context("Cannot determine current working directory")?;

    tracing::info!("Spawning daemon process: {:?}", exe);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS (0x00000008): 子进程不继承父进程控制台
        // CREATE_NO_WINDOW  (0x08000000): 不创建新控制台窗口
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        std::process::Command::new(&exe)
            .args(["start", "--daemon"])
            .current_dir(&cwd)
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn()
            .context("Failed to spawn daemon process")?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 平台：简单 detach（不支持真正守护进程化，仅保证编译通过）
        std::process::Command::new(&exe)
            .args(["start", "--daemon"])
            .current_dir(&cwd)
            .spawn()
            .context("Failed to spawn daemon process")?;
    }

    let log_path = daemon_log_path().unwrap_or_else(|_| PathBuf::from("~/.ai-pet/daemon.log"));
    tracing::info!("Daemon launched. Log: {:?}", log_path);
    println!("ai-pet daemon started. Log: {}", log_path.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// 守护进程主体：窗口 + IPC 循环
// ---------------------------------------------------------------------------

/// 守护子进程调用：加载资产、启动 Tokio / IPC、运行 winit 事件循环。
fn run_daemon() -> Result<()> {
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
            }"#
            .to_string(),
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

// ---------------------------------------------------------------------------
// 资产加载
// ---------------------------------------------------------------------------

fn load_all_animations(
    pet_config: &PetConfig,
    sprites_dir: &Path,
) -> Result<HashMap<Action, Animation>> {
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

        animations.insert(
            action,
            Animation {
                frames,
                frame_duration: Duration::from_millis(action_cfg.frame_duration_ms),
                looped: action_cfg.looped,
            },
        );
    }

    if !animations.contains_key(&Action::Idle) {
        anyhow::bail!("Config must contain 'idle' action");
    }

    Ok(animations)
}
