use bevy_ecs::{entity::Entity, event::Event};
use bevy_math::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Event, Debug, Serialize, Deserialize, Clone)]
pub struct SetAnimSpeedSeekEvent {
    pub animation: String,
    pub skeleton_id: u64,
    pub speed: f32,
    pub seek_to: Option<f32>,
    pub networked: bool,
}
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
#[derive(Event, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TriggerHit {
    pub attacker_id: u64,
    pub receiver_id: u64,
    pub damage: f32,
    pub networked: bool,
}
