use bevy::math::Vec3;
use bevy::prelude::*;

use crate::physics::actor::ActorFrameData;
use crate::physics::constants::COLLISION_MARGIN;

/// Projects the `direction` vector onto the `normal` vector.
pub fn project(direction: Vec3, normal: Vec3) -> Vec3 {
    normal * direction.dot(normal)
}

/// Calculates the rejection of the `direction` from the `normal`.
/// This returns the component of `direction` that is parallel to the collision surface.
pub fn reject(direction: Vec3, normal: Vec3) -> Vec3 {
    direction - project(direction, normal)
}
pub fn add_margin_to_delta(delta: Vec3) -> Vec3 {
    // 1:1 check for zero length to avoid division by zero in normalization
    if delta.length_squared() == 0.0 {
        return delta;
    }
    // Adds COLLISION_MARGIN units in the direction of the push
    delta + delta.normalize() * COLLISION_MARGIN
}
pub fn is_walkable_slope(normal: Vec3) -> bool {
    // cos(46 deg) ≈ 0.694
    const MAX_SLOPE_COS: f32 = 0.69465837;
    normal.y > MAX_SLOPE_COS
}
#[inline]
pub fn get_pitch_yaw_3d_approx_movement(actor_fd: &ActorFrameData) -> Vec3 {
    let pitch_rotation = Quat::from_rotation_x(-actor_fd.rotation.x);
    let yaw_rotation = Quat::from_rotation_y(-actor_fd.rotation.y);

    // use a 3d approximation of the movement vector to better judge player intent
    let velocity = (yaw_rotation * pitch_rotation) * actor_fd.movement;
    velocity
}
