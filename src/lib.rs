use bevy::prelude::*;
use serde::{Deserialize, Serialize};
pub mod client;
pub mod networking;
pub mod serializer;
#[derive(Copy, Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AnimationState {
    Idle,
    Running,
    Swinging,
    Jumping,
    FirstPersonMoving,
}
#[derive(Copy, Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RequestedAnimation {
    Common(AnimationState),
    SwingInactive,
    SwingReady,
    SwingCharging,
    SwingReleasing,
    SwingReturning,
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
                | AnimationState::Running
                | AnimationState::Jumping
                | AnimationState::FirstPersonMoving
        )
    }

    pub fn is_secondary(self) -> bool {
        !self.is_primary()
    }
}
