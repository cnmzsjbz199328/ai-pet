use anyhow::Result;
use async_trait::async_trait;
use core_engine::scripting::TimelineEvent;

/// LLM 客户端抽象，通过 trait 实现与具体 API 的解耦（便于 Mock 测试）。
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_script(&self, prompt: &str) -> Result<String>;
}

/// 解析 LLM 返回的 JSON 字符串为 `TimelineEvent` 列表。
/// 未知 action 字段自动 fallback 为 Idle，不返回 Err。
/// 非合法 JSON 返回 Err。
pub fn parse_script(json: &str) -> Result<Vec<TimelineEvent>> {
    // TODO(Task 10): 实现完整解析与白名单验证
    todo!("Task 10: parse_script")
}

// ---------------------------------------------------------------------------
// OpenAI 实现（Task 10 实现）
// ---------------------------------------------------------------------------

pub struct OpenAiClient {
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn generate_script(&self, _prompt: &str) -> Result<String> {
        // TODO(Task 10): 实现 OpenAI Chat Completion 调用
        todo!("Task 10: OpenAiClient::generate_script")
    }
}

// ---------------------------------------------------------------------------
// Mock 实现（用于集成测试）
// ---------------------------------------------------------------------------

pub struct MockLlmClient {
    pub preset_response: String,
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn generate_script(&self, _prompt: &str) -> Result<String> {
        Ok(self.preset_response.clone())
    }
}
