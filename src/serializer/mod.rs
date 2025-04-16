use bevy::prelude::*;
use serde::{Deserialize, Serialize};
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
#[derive(Resource, Default)]
pub struct PlayerState {
    pub transform: Transform,
}
#[derive(Serialize, Deserialize, Resource)]
pub struct PlayerStateNetwork {
    pub transform: SerializableTransform,
}
