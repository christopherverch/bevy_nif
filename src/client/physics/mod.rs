use bevy::app::Plugin;

mod components;
mod constants;
mod helpers;
pub mod movement;
pub mod preparation;
mod unstuck;
pub struct CharacterMotionPlugin;
mod apply_animation_motion;
pub mod physics_setup;
pub use apply_animation_motion::calculate_animation_displacement_system;
use bevy::prelude::*;
pub use components::CreaturePhysicsState;
use physics_setup::setup_nif_physics;
impl Plugin for CharacterMotionPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, setup_nif_physics);
        // `calculate_animation_displacement_system`: Produces `AnimationDisplacement` from animations.
        // `prepare_character_velocity_system`: Consumes `AnimationDisplacement` and player input to produce a final `velocity_for_frame`.
        // `character_movement_system`: Consumes `velocity_for_frame` to move the character and resolve all collisions.
        // The entire chain has to run before `PhysicsSet::SyncBackend` so Rapier can see the final, moved transform.
    }
}
