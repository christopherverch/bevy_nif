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
#[derive(Event, Reflect, Clone)]
pub struct AttackTrigger {
    pub entity: Entity,
    pub origin: Vec3,
    pub direction: Vec3,
    pub damage: f32,
}
#[derive(Event, Reflect, Clone)]
pub struct TakeHit {
    pub creature_id: u64,
    pub damage: f32,
    pub is_from_network: bool,
}

#[derive(Event, Reflect, Clone)]
pub struct KnockdownFinished(pub Entity);

#[derive(Event, Reflect, Clone)]
pub struct SoundEvent(pub String);
