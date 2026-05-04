use anyhow::Result;
use async_trait::async_trait;
use core_engine::scripting::TimelineEvent;
use serde::Deserialize;
use serde_json::json;

/// LLM 客户端抽象，通过 trait 实现与具体 API 的解耦（便于 Mock 测试）。
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn generate_script(&self, prompt: &str) -> Result<String>;
}

#[derive(Deserialize)]
struct ScriptResponse {
    #[allow(dead_code)]
    characters: Vec<String>,
    events: Vec<TimelineEvent>,
}

/// 解析 LLM 返回的 JSON 字符串为 `TimelineEvent` 列表。
/// 未知 action 字段自动 fallback 为 Idle，不返回 Err。
/// 非合法 JSON 返回 Err。
pub fn parse_script(json: &str) -> Result<Vec<TimelineEvent>> {
    let resp: ScriptResponse = serde_json::from_str(json)?;
    Ok(resp.events)
}

// ---------------------------------------------------------------------------
// 共用 System Prompt
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str = "\
You are a cat animation director. Output ONLY valid JSON, no markdown, no explanation.
Format:
{
  \"characters\": [\"pet1\"],
  \"events\": [
    {\"timestamp_ms\": 0,    \"actor_id\": \"pet1\", \"action\": \"idle\"},
    {\"timestamp_ms\": 2000, \"actor_id\": \"pet1\", \"action\": \"walk\"}
  ]
}
Action whitelist: idle walk jump attack sleep happy angry
Unknown actions are forbidden. Total duration should not exceed 30 seconds.";

// ---------------------------------------------------------------------------
// Gemini 实现
// ---------------------------------------------------------------------------

pub struct GeminiClient {
    api_key: String,
    model: String,
    http: reqwest::Client,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: std::env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-3-flash-preview".to_string()),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn generate_script(&self, prompt: &str) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        );

        let body = json!({
            "system_instruction": {
                "parts": [{"text": SYSTEM_PROMPT}]
            },
            "contents": [
                {"role": "user", "parts": [{"text": prompt}]}
            ],
            "generationConfig": {
                "temperature": 0.7,
                "responseMimeType": "application/json"
            }
        });

        let resp = self
            .http
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("Unexpected Gemini response shape: {}", resp))
    }
}

// ---------------------------------------------------------------------------
// OpenAI 实现（保留备用）
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
    async fn generate_script(&self, prompt: &str) -> Result<String> {
        let body = json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user",   "content": prompt}
            ],
            "temperature": 0.7
        });

        let resp = self
            .http
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        resp["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("Unexpected OpenAI response shape"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_engine::scripting::Action;

    #[test]
    fn test_parse_valid_script() {
        let json = r#"{
            "characters": ["pet1"],
            "events": [
                {"timestamp_ms": 0, "actor_id": "pet1", "action": "walk"},
                {"timestamp_ms": 1000, "actor_id": "pet1", "action": "jump"}
            ]
        }"#;
        let events = parse_script(json).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].action, Action::Walk);
        assert_eq!(events[1].action, Action::Jump);
    }

    #[test]
    fn test_parse_script_with_unknown_action_fallback() {
        let json = r#"{
            "characters": ["pet1"],
            "events": [
                {"timestamp_ms": 0, "actor_id": "pet1", "action": "dance"}
            ]
        }"#;
        let events = parse_script(json).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, Action::Idle); // fallback
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = r#"{"invalid": "json""#;
        let result = parse_script(json);
        assert!(result.is_err());
    }
}
