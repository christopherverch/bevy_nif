use bevy_ecs::entity::Entity;
use glam::Vec3;

pub struct RawTraceResult {
    pub fraction: f32,
    pub plane_normal: Vec3,
    pub hit_point: Vec3,
    pub hit_entity: Option<u64>,
}

impl RawTraceResult {
    pub fn has_hit(&self) -> bool {
        self.fraction < 1.0
    }
}
pub struct RayHit {
    /// The exact point in the world where the ray hit a surface.
    pub hit_point: Vec3,
    /// The normal vector of the surface at the hit point.
    pub hit_normal: Vec3,
    /// The distance from the ray's origin to the hit point.
    pub distance: f32,
    /// The entity that was hit (bit representation)
    pub entity: Entity,
}
