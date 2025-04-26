use bevy::prelude::*;

use crate::AnimationState;
#[derive(Event, Clone)]
pub struct RequestAnimation {
    pub id: u64,
    pub animation_state: AnimationState,
}
