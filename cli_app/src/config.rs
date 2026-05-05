use std::collections::HashMap;

use serde::Deserialize;

/// 顶层宠物配置，对应 `assets/config/pet_config.json`。
#[derive(Deserialize)]
pub struct PetConfig {
    /// 宠物 ID，同时作为精灵图子目录名（`sprites/<pet_id>/`）。
    pub pet_id: String,
    pub actions: HashMap<String, ActionConfig>,
    /// 特效动画配置，key 为特效名（与 `sprites/effects/<name>/` 对应）。
    #[serde(default)]
    pub effects: HashMap<String, ActionConfig>,
}

/// 单个动作或特效的帧配置。
#[derive(Deserialize)]
pub struct ActionConfig {
    pub frame_count: usize,
    pub frame_duration_ms: u64,
    pub looped: bool,
}
