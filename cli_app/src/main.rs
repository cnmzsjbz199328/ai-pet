#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod ai;
mod ipc;
mod window;

use anyhow::Result;
use clap::{Parser, Subcommand};

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
            // TODO(Task 9): 启动 tokio runtime + IPC 服务端 + winit EventLoop
            todo!("Task 9: start daemon")
        }
        Command::Play { prompt } => {
            tracing::info!(%prompt, "Sending play command to daemon");
            // TODO(Task 9): 作为 IPC 客户端发送 prompt
            todo!("Task 9: play command")
        }
        Command::Stop => {
            tracing::info!("Sending stop command to daemon");
            // TODO(Task 9): 发送 Shutdown 消息
            todo!("Task 9: stop command")
        }
    }
}
