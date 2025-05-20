use bevy::prelude::*;
use client::defs::Direction;
use serde::{Deserialize, Serialize};
pub mod client;
pub mod networking;
pub mod serializer;
#[derive(Copy, Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AnimationState {
    Idle,
    Running { direction: Direction },
    Swinging,
    Jumping,
    FirstPersonMoving,
    SneakIdle,
    Knockdown,
    Death,
    Undefined,
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TriggeredAction {
    TakeHit { creature_id: u64, damage: f32 },
}
#[derive(Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RequestedAnimation {
    Common(AnimationState),
    SwingInactive,
    SwingReady,
    SwingCharging,
    SwingReleasing,
    SwingReturning,
    LoopMovement { animation: String },
    FreezeAnimation,
    ResumeAnimation,
}

impl From<RequestedAnimation> for AnimationState {
    fn from(req: RequestedAnimation) -> Self {
        match req {
            RequestedAnimation::Common(state) => state,
            RequestedAnimation::SwingInactive
            | RequestedAnimation::SwingReady
            | RequestedAnimation::SwingCharging
            | RequestedAnimation::SwingReleasing
            | RequestedAnimation::SwingReturning => AnimationState::Swinging,
            RequestedAnimation::LoopMovement { .. } => AnimationState::Undefined,
            RequestedAnimation::FreezeAnimation => AnimationState::Undefined,
            RequestedAnimation::ResumeAnimation => AnimationState::Undefined,
        }
    }
}

impl From<AnimationState> for RequestedAnimation {
    fn from(state: AnimationState) -> Self {
        RequestedAnimation::Common(state)
    }
}
impl AnimationState {
    pub fn is_primary(self) -> bool {
        matches!(
            self,
            AnimationState::Idle
                | AnimationState::Running { .. }
                | AnimationState::Jumping
                | AnimationState::FirstPersonMoving
        )
    }

    pub fn is_secondary(self) -> bool {
        !self.is_primary()
    }
}
