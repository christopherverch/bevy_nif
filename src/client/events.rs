use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::AnimationConfig;
#[derive(Event, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RequestAnimation {
    pub skeleton_id: u64,
    pub animation_config: AnimationConfig,
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
#[derive(Event)]
pub struct PlayerReadyWeapon {
    pub player_entity: Entity,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SoundSource {
    Attached(Entity),
    Located(Vec3),
    LocalPlayer,
}
#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub struct SoundEvent {
    pub sound_source: SoundSource,
    pub sound_path: String,
}

#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub struct SetAnimSpeedSeekEvent {
    pub animation: String,
    pub skeleton_id: u64,
    pub speed: f32,
    pub seek_to: Option<f32>,
    pub networked: bool,
}
