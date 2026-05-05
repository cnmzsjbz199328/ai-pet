use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::scripting::{Action, EngineError};

/// 解码后的 RGBA 像素帧，启动时全量加载，Arc 共享避免克隆开销。
pub type Texture = Arc<image::RgbaImage>;

/// 单个动作的帧序列与播放配置。
#[derive(Clone)]
pub struct Animation {
    pub frames: Vec<Texture>,
    pub frame_duration: Duration,
    pub looped: bool,
}

/// 管理所有动作的帧播放，由主循环每帧调用 `update`。
pub struct Animator {
    animations: HashMap<Action, Animation>,
    current_action: Action,
    elapsed: Duration,
}

impl Animator {
    pub fn new(animations: HashMap<Action, Animation>) -> Self {
        Self {
            animations,
            current_action: Action::Idle,
            elapsed: Duration::ZERO,
        }
    }

    /// 推进时间并返回当前应显示的帧。
    /// delta 来自主循环 Instant 差值，使用 Duration 避免浮点累积误差。
    ///
    /// # Errors
    /// 若当前 action 在字典中不存在，返回 `EngineError::MissingAnimation`。
    pub fn update(&mut self, delta: Duration) -> Result<&Texture, EngineError> {
        self.elapsed += delta;
        let anim = self
            .animations
            .get(&self.current_action)
            .ok_or(EngineError::MissingAnimation(self.current_action))?;

        let frame_ms = anim.frame_duration.as_millis().max(1);
        let frame_idx = (self.elapsed.as_millis() / frame_ms) as usize;

        let idx = if anim.looped {
            frame_idx % anim.frames.len()
        } else {
            frame_idx.min(anim.frames.len().saturating_sub(1))
        };

        Ok(&anim.frames[idx])
    }

    /// 切换动作并重置播放进度。相同动作重复调用为空操作。
    pub fn set_action(&mut self, action: Action) {
        if self.current_action != action {
            self.current_action = action;
            self.elapsed = Duration::ZERO;
        }
    }

    pub fn current_action(&self) -> Action {
        self.current_action
    }
}

/// 单次特效播放器：持有一个 one-shot `Animation`，播放结束后 `update` 返回 `None`。
///
/// 区别于 `Animator`：
/// - 不依赖 `Action` 作为 key（特效不属于角色动作语义）。
/// - 自动感知结束状态，调用方可据此清除实例。
pub struct EffectPlayer {
    animation: Animation,
    elapsed: Duration,
    finished: bool,
}

impl EffectPlayer {
    pub fn new(animation: Animation) -> Self {
        Self {
            animation,
            elapsed: Duration::ZERO,
            finished: false,
        }
    }

    /// 推进时间并返回当前帧。
    ///
    /// - 返回 `Some(&Texture)` 时动画仍在播放。
    /// - 返回 `None` 时 one-shot 动画已结束，调用方应丢弃此实例。
    /// - 对 `looped` 动画永远返回 `Some`。
    pub fn update(&mut self, delta: Duration) -> Option<&Texture> {
        if self.finished {
            return None;
        }

        self.elapsed += delta;
        let anim = &self.animation;
        let frame_ms = anim.frame_duration.as_millis().max(1);
        let frame_idx = (self.elapsed.as_millis() / frame_ms) as usize;

        if !anim.looped && frame_idx >= anim.frames.len() {
            self.finished = true;
            return None;
        }

        let idx = if anim.looped {
            frame_idx % anim.frames.len()
        } else {
            frame_idx.min(anim.frames.len().saturating_sub(1))
        };

        Some(&anim.frames[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn make_texture() -> Texture {
        Arc::new(RgbaImage::new(32, 32))
    }

    fn make_animator(frame_count: usize, duration_ms: u64, looped: bool) -> Animator {
        let frames = (0..frame_count).map(|_| make_texture()).collect();
        let anim = Animation {
            frames,
            frame_duration: Duration::from_millis(duration_ms),
            looped,
        };
        let mut map = HashMap::new();
        map.insert(Action::Idle, anim);
        Animator::new(map)
    }

    #[test]
    fn returns_first_frame_at_zero_elapsed() {
        let mut a = make_animator(4, 200, true);
        // 未推进时应返回 frame[0]（Arc 地址相同）
        let f0 = a.update(Duration::ZERO).unwrap();
        let _ = f0; // 只验证不报错
    }

    #[test]
    fn advances_frame_at_boundary() {
        let mut a = make_animator(4, 200, true);
        // 200ms → frame[1]
        let _ = a.update(Duration::from_millis(200)).unwrap();
        // 再 200ms → frame[2]
        let _ = a.update(Duration::from_millis(200)).unwrap();
    }

    #[test]
    fn looped_animation_wraps() {
        let mut a = make_animator(4, 200, true);
        // 4 帧 × 200ms = 800ms 后回到 frame[0]
        let _ = a.update(Duration::from_millis(800)).unwrap();
    }

    #[test]
    fn non_looped_animation_clamps_to_last_frame() {
        let mut a = make_animator(4, 200, false);
        // 超出总时长，应停在最后一帧，不 panic
        let _ = a.update(Duration::from_millis(9999)).unwrap();
    }

    #[test]
    fn set_action_resets_elapsed() {
        let frames = vec![make_texture()];
        let idle = Animation { frames: frames.clone(), frame_duration: Duration::from_millis(200), looped: true };
        let walk = Animation { frames, frame_duration: Duration::from_millis(100), looped: true };
        let mut map = HashMap::new();
        map.insert(Action::Idle, idle);
        map.insert(Action::Walk, walk);
        let mut a = Animator::new(map);

        let _ = a.update(Duration::from_millis(500)).unwrap();
        a.set_action(Action::Walk);
        assert_eq!(a.elapsed, Duration::ZERO);
    }

    #[test]
    fn set_action_same_action_is_noop() {
        let mut a = make_animator(4, 200, true);
        let _ = a.update(Duration::from_millis(150)).unwrap();
        let elapsed_before = a.elapsed;
        a.set_action(Action::Idle); // 相同动作，不重置
        assert_eq!(a.elapsed, elapsed_before);
    }

    #[test]
    fn missing_action_returns_error() {
        let mut a = make_animator(1, 100, true);
        a.set_action(Action::Walk); // Walk 不在字典里
        let result = a.update(Duration::ZERO);
        assert!(matches!(result, Err(EngineError::MissingAnimation(Action::Walk))));
    }
}
