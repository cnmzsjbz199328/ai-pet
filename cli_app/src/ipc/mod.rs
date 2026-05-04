/// IPC 消息：从 Tokio 任务发往 winit 主线程。
#[derive(Debug)]
pub enum AppMessage {
    /// 注入新剧本事件列表。
    InjectTimeline(Vec<core_engine::scripting::TimelineEvent>),
    /// LLM 调用失败，携带错误描述。
    LlmError(String),
    /// 关闭守护进程。
    Shutdown,
}

// ---------------------------------------------------------------------------
// Windows Named Pipe 实现
// ---------------------------------------------------------------------------

pub const PIPE_NAME: &str = r"\\.\pipe\ai-pet-ipc";

#[cfg(target_os = "windows")]
pub mod windows {
    use anyhow::Result;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    use super::{AppMessage, PIPE_NAME};

    /// 服务端：循环 accept，每条连接读一条消息并解析 prompt，
    /// 通过 `tx` 发给主线程。
    pub async fn run_server(
        tx: std::sync::mpsc::Sender<AppMessage>,
        llm: std::sync::Arc<dyn crate::ai::LlmClient>,
    ) -> Result<()> {
        loop {
            // Named Pipe 服务端每次 connect 后需重新创建实例以接受下一个连接
            let mut server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(PIPE_NAME)?;

            tracing::debug!("IPC server waiting for connection...");
            server.connect().await?;

            let body = match read_message(&mut server).await {
                Ok(b) => b,
                Err(err) => {
                    tracing::warn!("Failed to read IPC message: {}", err);
                    continue;
                }
            };

            let json: serde_json::Value = match serde_json::from_str(&body) {
                Ok(j) => j,
                Err(err) => {
                    tracing::warn!("Invalid IPC JSON: {}", err);
                    continue;
                }
            };

            let prompt = match json["prompt"].as_str() {
                Some(p) => p,
                None => {
                    tracing::warn!("IPC message missing 'prompt' field");
                    continue;
                }
            };

            if prompt == "__shutdown__" {
                tracing::info!("IPC shutdown requested");
                let _ = tx.send(AppMessage::Shutdown);
                break;
            }

            // 异步处理 LLM 调用，避免阻塞 IPC 接收循环
            let tx_clone = tx.clone();
            let llm_clone = llm.clone();
            let prompt_owned = prompt.to_string();

            tokio::spawn(async move {
                tracing::info!("Generating script for prompt: {}", prompt_owned);
                match llm_clone.generate_script(&prompt_owned).await {
                    Ok(script_json) => {
                        match crate::ai::parse_script(&script_json) {
                            Ok(events) => {
                                let _ = tx_clone.send(AppMessage::InjectTimeline(events));
                            }
                            Err(err) => {
                                let _ = tx_clone.send(AppMessage::LlmError(format!("Parse error: {}", err)));
                            }
                        }
                    }
                    Err(err) => {
                        let _ = tx_clone.send(AppMessage::LlmError(format!("LLM error: {}", err)));
                    }
                }
            });
        }
        Ok(())
    }

    /// 客户端：连接并发送一条 prompt 消息，然后退出。
    /// 消息格式：[4-byte u32 LE length][UTF-8 JSON body]
    pub async fn send_prompt(prompt: &str) -> Result<()> {
        let mut client = ClientOptions::new().open(PIPE_NAME)?;
        let body = serde_json::json!({ "prompt": prompt }).to_string();
        write_message(&mut client, &body).await?;
        client.flush().await?;
        Ok(())
    }

    /// 读取一条消息：4字节长度前缀 + JSON body。
    async fn read_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<String> {
        let len = reader.read_u32_le().await? as usize;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).await?;
        Ok(String::from_utf8(buf)?)
    }

    /// 写入一条消息：4字节长度前缀 + JSON body。
    async fn write_message<W: AsyncWriteExt + Unpin>(writer: &mut W, body: &str) -> Result<()> {
        let bytes = body.as_bytes();
        writer.write_u32_le(u32::try_from(bytes.len())?).await?;
        writer.write_all(bytes).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 非 Windows 平台 stub（保证 crate 可编译）
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
pub mod windows {
    use anyhow::bail;

    pub async fn run_server(
        _tx: std::sync::mpsc::Sender<super::AppMessage>,
        _llm: std::sync::Arc<dyn crate::ai::LlmClient>,
    ) -> anyhow::Result<()> {
        bail!("IPC Named Pipe is only supported on Windows")
    }

    pub async fn send_prompt(_prompt: &str) -> anyhow::Result<()> {
        bail!("IPC Named Pipe is only supported on Windows")
    }
}
