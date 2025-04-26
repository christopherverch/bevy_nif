use crate::client::defs::PlayerState;
use crate::{AnimationState, AnimationWeights};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

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
#[derive(Serialize, Deserialize, Resource)]
pub struct PlayerStateNetwork {
    pub transform: SerializableTransform,
    pub animation_weights: AnimationWeights,
    pub current_animation: AnimationState,
    pub current_secondary_animation: AnimationState,
    pub is_attacking: bool,
}
