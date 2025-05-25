use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::RequestedAnimation;
#[derive(Event, Clone)]
pub struct RequestAnimation {
    pub entity_id: u64,
    pub requested_animation: RequestedAnimation,
    pub networked: bool,
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
pub struct TryAttackTrigger {
    pub attacker_entity: Entity,
    pub attacker_id: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub damage: f32,
}
#[derive(Event, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TriggerHit {
    pub attacker_id: u64,
    pub receiver_id: u64,
    pub damage: f32,
    pub networked: bool,
}

#[derive(Event, Reflect, Clone)]
pub struct KnockdownFinished(pub Entity);

#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub struct SoundEvent {
    pub sound_anchor_entity: Option<Entity>,
    pub translation: Option<Vec3>,
    pub sound_path: String,
    pub is_attached_to_local_player: bool,
}
