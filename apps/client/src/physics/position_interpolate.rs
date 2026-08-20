use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct PositionInterpolation {
    pub velocity: Vec3,
    pub current_position: Vec3,
    pub previous_position: Vec3,
    delta: f32,
}
impl PositionInterpolation {
    pub fn new(velocity: Vec3, current_position: Vec3) -> Self {
        Self {
            velocity,
            current_position,
            previous_position: current_position,
            delta: 0.0,
        }
    }
    pub fn consume_velocity(&mut self) -> Vec3 {
        std::mem::take(&mut self.velocity) // Returns velocity and leaves Vec3::ZERO
    }
    pub fn get_and_update_interpolated_pos(&mut self, dt: f32) -> Vec3 {
        const TICK_RATE: f32 = 1.0 / 60.0;
        self.delta += dt;
        let alpha = (self.delta / TICK_RATE).min(1.0);
        self.previous_position.lerp(self.current_position, alpha)
    }
}
