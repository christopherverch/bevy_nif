use crate::{
    TriggeredAction,
    client::defs::{AnimationEvent, AttackState, CreatureStats, PlayerState},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SerializableTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
impl From<&Transform> for SerializableTransform {
    fn from(transform: &Transform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}
impl From<SerializableTransform> for Transform {
    fn from(s: SerializableTransform) -> Self {
        Transform {
            translation: s.translation,
            rotation: s.rotation,
            scale: s.scale,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Resource)]
pub struct PlayerStateNetwork {
    pub transform: SerializableTransform,
    pub animation_events: Vec<(u8, AnimationEvent)>,
    pub triggered_actions: Vec<(u8, TriggeredAction)>,
    pub attack_state: AttackState,
    pub creature_stats: CreatureStats,
}
