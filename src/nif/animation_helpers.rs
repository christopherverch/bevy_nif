use bevy::math::{Quat, Vec3};

#[derive(Debug, Clone, Copy)] // Assuming KeyType is simple
pub enum KeyType {
    Linear,
    Quadratic,
    TBC,
    XYZ,
    Constant,
}
impl Default for KeyType {
    fn default() -> Self {
        KeyType::Linear
    }
} // Example default

#[derive(Debug, Clone, Copy)]
pub struct KeyQuaternion {
    pub time: f32,
    pub value: Quat, /* + TBC/Quadratic data if needed */
}
#[derive(Debug, Clone, Copy)]
pub struct KeyVec3 {
    pub time: f32,
    pub value: Vec3, /* + TBC/Quadratic data if needed */
}
#[derive(Debug, Clone, Copy)]
pub struct KeyFloat {
    pub time: f32,
    pub value: f32, /* + TBC/Quadratic data if needed */
}
