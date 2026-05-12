use bevy_ecs::entity::Entity;
use glam::{DVec3, Vec3};
/// Bevy entity in bits
pub struct PtrHolder {
    pub entity: Entity,
    pub velocity: Vec3,
    pub simulation_position: Vec3,
    // Position tracking has logic, so keep fields private
    pub(crate) position: DVec3,
    pub(crate) previous_position: DVec3,
}
impl PtrHolder {
    pub fn new(entity: Entity, pos: Vec3) -> Self {
        Self {
            entity,
            simulation_position: pos,
            velocity: Vec3::ZERO,
            position: pos.into(),
            previous_position: pos.into(),
        }
    }

    pub fn set_position(&mut self, new_pos: DVec3) {
        self.previous_position = self.position;
        self.position = new_pos;
    }

    pub fn get_position(&self) -> DVec3 {
        self.position
    }
    pub fn get_previous_position(&self) -> DVec3 {
        self.previous_position
    }

    pub fn consume_velocity(&mut self) -> Vec3 {
        std::mem::take(&mut self.velocity) // Returns velocity and leaves Vec3::ZERO
    }
}
