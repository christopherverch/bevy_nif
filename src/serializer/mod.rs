use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;
#[derive(Serialize, Deserialize)]
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
#[derive(Resource, Component)]
pub struct PlayerState {
    pub transform: Transform,
    pub last_packet_time: Instant,
}
#[derive(Serialize, Deserialize, Resource)]
pub struct PlayerStateNetwork {
    pub transform: SerializableTransform,
}
impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            transform: Transform::default(),
            last_packet_time: Instant::now(),
        }
    }
}
