#![allow(dead_code)]
// This file defines the Rust structs and enums necessary to replicate the state
// management logic found in OpenMW's `character.hpp` and `character.cpp`.

use bevy::prelude::*;

/// A component that stores the calculated root motion displacement for a single frame.
/// This is calculated by one system and consumed by the physics movement system.
#[derive(Component, Debug, Default)]
pub struct RootMotionState {
    /// The name of the animation group that was playing last frame.
    pub last_animation_group: String,
    /// The elapsed time of the animation from the previous frame.
    pub previous_time: f32,
}
// Speed determined by how far the movement animation moved in this frame
// ex: runforward moved 0.1 meter forward since last frame, this would be 0.1
// used for the physics movement system to determine how far we should move in 3rd person
#[derive(Component, Default)]
pub struct AnimationDisplacement(pub Vec3);
