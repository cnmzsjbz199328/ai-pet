use std::collections::HashMap;

use serde::Deserialize;

/// 顶层宠物配置，对应 `assets/config/pet_config.json`。
#[derive(Deserialize)]
pub struct PetConfig {
    pub actions: HashMap<String, ActionConfig>,
}

/// 单个动作的帧配置。
#[derive(Deserialize)]
pub struct ActionConfig {
    pub frame_count: usize,
    pub frame_duration_ms: u64,
    pub looped: bool,
}
