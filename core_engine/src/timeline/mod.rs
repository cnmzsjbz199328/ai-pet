use std::time::Duration;

use crate::scripting::TimelineEvent;

/// 时间轴调度器：将 AI 剧本事件按毫秒时序分发给动画引擎。
pub struct Timeline {
    /// 始终保持按 timestamp_ms 升序排列。
    events: Vec<TimelineEvent>,
    /// 已经分发到的事件游标（索引）。
    cursor: usize,
    /// 从剧本开始累计的已流逝毫秒数。
    elapsed_ms: u64,
    /// 所有事件已全部分发。
    pub finished: bool,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            cursor: 0,
            elapsed_ms: 0,
            finished: true, // 初始无剧本，视为已完成
        }
    }

    /// 注入新剧本，自动按 timestamp_ms 升序排序，重置播放进度。
    pub fn push_events(&mut self, mut events: Vec<TimelineEvent>) {
        events.sort_by_key(|e| e.timestamp_ms);
        self.events = events;
        self.cursor = 0;
        self.elapsed_ms = 0;
        self.finished = self.events.is_empty();
    }

    /// 推进时间，返回本 tick 内到期的所有事件（按时序排列）。
    /// 全部事件分发完毕后将 `finished` 置为 true。
    pub fn tick(&mut self, delta: Duration) -> Vec<&TimelineEvent> {
        if self.finished {
            return Vec::new();
        }

        self.elapsed_ms += u64::try_from(delta.as_millis()).unwrap_or(u64::MAX);

        let mut due = Vec::new();
        while self.cursor < self.events.len()
            && self.events[self.cursor].timestamp_ms <= self.elapsed_ms
        {
            due.push(&self.events[self.cursor]);
            self.cursor += 1;
        }

        if self.cursor >= self.events.len() {
            self.finished = true;
            tracing::info!("Timeline finished: all events dispatched");
        }

        due
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::{Action, TimelineEvent};

    fn event(ms: u64, action: Action) -> TimelineEvent {
        TimelineEvent { timestamp_ms: ms, actor_id: "pet1".into(), action }
    }

    #[test]
    fn events_dispatched_at_correct_timestamp() {
        let mut t = Timeline::new();
        t.push_events(vec![event(0, Action::Idle), event(500, Action::Walk), event(1000, Action::Sleep)]);

        let due = t.tick(Duration::from_millis(0));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].action, Action::Idle);

        let due = t.tick(Duration::from_millis(500));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].action, Action::Walk);

        let due = t.tick(Duration::from_millis(500));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].action, Action::Sleep);

        assert!(t.finished);
    }

    #[test]
    fn out_of_order_events_are_sorted_on_push() {
        let mut t = Timeline::new();
        t.push_events(vec![event(1000, Action::Sleep), event(200, Action::Walk), event(500, Action::Jump)]);

        // 推进到 300ms，应只有 200ms 的事件到期
        let due = t.tick(Duration::from_millis(300));
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].timestamp_ms, 200);
    }

    #[test]
    fn multiple_events_in_same_tick_all_dispatched() {
        let mut t = Timeline::new();
        t.push_events(vec![event(0, Action::Idle), event(0, Action::Walk)]);

        let due = t.tick(Duration::ZERO);
        assert_eq!(due.len(), 2);
    }

    #[test]
    fn finished_is_set_after_last_event() {
        let mut t = Timeline::new();
        t.push_events(vec![event(100, Action::Idle)]);
        assert!(!t.finished);

        t.tick(Duration::from_millis(200));
        assert!(t.finished);
    }

    #[test]
    fn empty_timeline_does_not_panic() {
        let mut t = Timeline::new();
        t.push_events(vec![]);
        let due = t.tick(Duration::from_millis(1000));
        assert!(due.is_empty());
        assert!(t.finished);
    }

    #[test]
    fn push_events_resets_previous_playback() {
        let mut t = Timeline::new();
        t.push_events(vec![event(0, Action::Idle), event(500, Action::Walk)]);
        t.tick(Duration::from_millis(1000));
        assert!(t.finished);

        // 重新注入，应从头开始
        t.push_events(vec![event(0, Action::Jump)]);
        assert!(!t.finished);
        let due = t.tick(Duration::ZERO);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].action, Action::Jump);
    }
}
