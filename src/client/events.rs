use bevy::prelude::*;

use crate::RequestedAnimation;
#[derive(Event, Clone)]
pub struct RequestAnimation {
    pub entity: Entity,
    pub requested_animation: RequestedAnimation,
}
#[derive(Event, Clone)]
pub struct ToggledThirdPerson {
    pub entity: Entity,
    pub is_third_person: bool,
}
#[derive(Event, Reflect, Clone)]
pub struct OnSwingFinished {
    pub entity: Entity,
}
