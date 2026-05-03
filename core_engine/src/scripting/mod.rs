use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 所有合法的宠物动作（白名单）。
/// LLM 输出的字符串必须能反序列化为此枚举，否则 fallback 为 Idle。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Idle,
    Walk,
    Jump,
    Attack,
    Sleep,
    Happy,
    Angry,
}

impl Default for Action {
    fn default() -> Self {
        Self::Idle
    }
}

impl Action {
    /// 从字符串解析，未知动作 fallback 为 Idle 并记录 WARN 日志。
    pub fn from_str_or_fallback(s: &str) -> Self {
        match s {
            "idle"   => Self::Idle,
            "walk"   => Self::Walk,
            "jump"   => Self::Jump,
            "attack" => Self::Attack,
            "sleep"  => Self::Sleep,
            "happy"  => Self::Happy,
            "angry"  => Self::Angry,
            unknown  => {
                tracing::warn!(action = unknown, "Unknown action, falling back to Idle");
                Self::Idle
            }
        }
    }
}

/// 单条时间轴事件，由 LLM 剧本解析器生成。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub timestamp_ms: u64,
    pub actor_id: String,
    #[serde(deserialize_with = "deserialize_action")]
    pub action: Action,
}

fn deserialize_action<'de, D>(deserializer: D) -> Result<Action, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(Action::from_str_or_fallback(&s))
}

/// 引擎域错误。
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Asset not found: {0}")]
    AssetNotFound(std::path::PathBuf),

    #[error("Config invalid: {0}")]
    ConfigInvalid(String),

    #[error("Animation dictionary has no entry for action: {0:?}")]
    MissingAnimation(Action),

    #[error("Invalid timeline event: timestamp {0}ms is out of range")]
    InvalidTimeline(u64),
}
