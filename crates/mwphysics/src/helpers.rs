use glam::Vec3;

/// Project vector u onto vector v.
pub fn project(u: Vec3, v: Vec3) -> Vec3 {
    v * u.dot(v)
}

/// Removes the component of 'direction' that points into the 'planeNormal'.
/// This is what allows actors to slide along walls.
pub fn reject(direction: Vec3, plane_normal: Vec3) -> Vec3 {
    direction - project(direction, plane_normal)
}

/// Uses Morrowind's Constants::sMaxSlope (default 50 degrees).
pub fn is_walkable_slope(normal: Vec3) -> bool {
    // Constants::sMaxSlope = 46.0 degrees.
    // cos(46 deg) ≈ 0.694
    const MAX_SLOPE_COS: f32 = 0.69465837;
    normal.y > MAX_SLOPE_COS
}
