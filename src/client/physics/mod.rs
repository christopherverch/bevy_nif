use bevy::app::Plugin;

mod components;
mod constants;
mod helpers;
mod movement;
mod preparation;
mod unstuck;
pub struct CharacterMotionPlugin;
mod apply_animation_motion;
pub mod physics_setup;
pub mod state_determination;
pub use apply_animation_motion::calculate_animation_displacement_system;
use bevy::prelude::*;
use bevy_rapier3d::plugin::PhysicsSet;
pub use components::CreaturePhysicsState;
use movement::character_movement_system;
use physics_setup::setup_nif_physics;
use preparation::prepare_character_velocity_system;
pub use state_determination::determine_character_state_system;
impl Plugin for CharacterMotionPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, setup_nif_physics);
        app.add_systems(
            FixedUpdate,
            (
                calculate_animation_displacement_system,
                prepare_character_velocity_system,
                character_movement_system,
            )
                .chain()
                .before(PhysicsSet::SyncBackend),
        );
    }
}
