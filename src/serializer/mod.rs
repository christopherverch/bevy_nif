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
