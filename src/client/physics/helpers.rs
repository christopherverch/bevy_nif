use bevy::math::Vec3;

/// Projects the `direction` vector onto the `normal` vector.
pub fn project(direction: Vec3, normal: Vec3) -> Vec3 {
    normal * direction.dot(normal)
}

/// Calculates the rejection of the `direction` from the `normal`.
/// This returns the component of `direction` that is parallel to the collision surface.
pub fn reject(direction: Vec3, normal: Vec3) -> Vec3 {
    direction - project(direction, normal)
}
