use crate::client::defs::{AttackState, PlayerState};
use crate::{AnimationState, RequestedAnimation};
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
    pub current_animation: AnimationState,
    pub current_secondary_animation: AnimationState,
    pub requested_primary_animation: Option<RequestedAnimation>,
    pub requested_secondary_animation: Option<RequestedAnimation>,
    pub attack_state: AttackState,
}
impl From<&PlayerState> for PlayerStateNetwork {
    fn from(state: &PlayerState) -> Self {
        PlayerStateNetwork {
            transform: SerializableTransform::from(&state.transform),
            current_animation: state.current_animation,
            current_secondary_animation: state.secondary_animation,
            requested_primary_animation: state.requested_primary_animation,
            requested_secondary_animation: state.requested_secondary_animation,
            attack_state: state.attack_state,
        }
    }
}
