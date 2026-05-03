use crate::scripting::Action;

/// 宠物有限状态机的全部状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetState {
    /// 默认待机，循环播放 idle 动画。
    Idle,
    /// 已接收到 play 指令，等待 LLM 响应中。
    Acting,
    /// 正在按时间轴播放 AI 生成的剧本。
    Scripted,
}

/// 驱动状态机的输入事件。
#[derive(Debug, Clone)]
pub enum FsmInput {
    InjectTimeline,
    LlmError,
    Click,
    Drag,
    TimelineFinished,
}

pub struct StateMachine {
    state: PetState,
}

impl StateMachine {
    pub fn new() -> Self {
        Self { state: PetState::Idle }
    }

    pub fn state(&self) -> PetState {
        self.state
    }

    /// 按转移表处理输入，返回新状态。Drag 不改变状态。
    pub fn apply(&mut self, input: FsmInput) -> PetState {
        use FsmInput::{Click, Drag, InjectTimeline, LlmError, TimelineFinished};
        use PetState::{Acting, Idle, Scripted};

        let next = match (self.state, input) {
            (Idle,     InjectTimeline)  => Scripted,
            (Idle,     Click)           => Acting,
            (Acting,   InjectTimeline)  => Scripted,
            (Acting,   LlmError)        => Idle,
            (Scripted, TimelineFinished)=> Idle,
            (_,        Drag)            => self.state, // 任意状态下 Drag 不改变状态
            (state,    input) => {
                tracing::trace!(?state, ?input, "FSM: ignored input in current state");
                state
            }
        };

        if next != self.state {
            tracing::info!(from = ?self.state, to = ?next, "FSM state transition");
        }
        self.state = next;
        next
    }

    /// 根据当前状态返回应播放的默认动作（供动画引擎参考）。
    pub fn default_action(&self) -> Action {
        match self.state {
            PetState::Idle     => Action::Idle,
            PetState::Acting   => Action::Idle, // 等待期间维持 Idle
            PetState::Scripted => Action::Idle, // Timeline 覆盖此值
        }
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_inject_timeline_goes_to_scripted() {
        let mut fsm = StateMachine::new();
        assert_eq!(fsm.apply(FsmInput::InjectTimeline), PetState::Scripted);
    }

    #[test]
    fn idle_click_goes_to_acting() {
        let mut fsm = StateMachine::new();
        assert_eq!(fsm.apply(FsmInput::Click), PetState::Acting);
    }

    #[test]
    fn acting_inject_timeline_goes_to_scripted() {
        let mut fsm = StateMachine::new();
        fsm.apply(FsmInput::Click);
        assert_eq!(fsm.apply(FsmInput::InjectTimeline), PetState::Scripted);
    }

    #[test]
    fn acting_llm_error_returns_to_idle() {
        let mut fsm = StateMachine::new();
        fsm.apply(FsmInput::Click);
        assert_eq!(fsm.apply(FsmInput::LlmError), PetState::Idle);
    }

    #[test]
    fn scripted_timeline_finished_returns_to_idle() {
        let mut fsm = StateMachine::new();
        fsm.apply(FsmInput::InjectTimeline);
        assert_eq!(fsm.apply(FsmInput::TimelineFinished), PetState::Idle);
    }

    #[test]
    fn drag_does_not_change_state_from_any_state() {
        for initial_input in [None, Some(FsmInput::Click), Some(FsmInput::InjectTimeline)] {
            let mut fsm = StateMachine::new();
            if let Some(input) = initial_input {
                fsm.apply(input);
            }
            let before = fsm.state();
            fsm.apply(FsmInput::Drag);
            assert_eq!(fsm.state(), before);
        }
    }

    #[test]
    fn ignored_input_leaves_state_unchanged() {
        // Scripted 状态下 Click 应被忽略
        let mut fsm = StateMachine::new();
        fsm.apply(FsmInput::InjectTimeline);
        assert_eq!(fsm.state(), PetState::Scripted);
        fsm.apply(FsmInput::Click);
        assert_eq!(fsm.state(), PetState::Scripted);
    }
}
