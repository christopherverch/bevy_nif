use bevy::prelude::*;
use serde::{Deserialize, Serialize};
pub mod client;
pub mod networking;
pub mod serializer;
#[derive(Clone, Component, Copy, Debug, Serialize, Deserialize)]
pub struct AnimationWeights {
    pub idle: f32,
    pub run: f32,
    pub swing: f32,
}
#[derive(Copy, Component, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AnimationState {
    Idle,
    Running,
    Swinging,
    Jumping,
}
