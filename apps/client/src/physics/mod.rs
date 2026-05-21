use bevy::app::Plugin;

mod components;
mod constants;
mod helpers;
pub mod movement;
mod prepare_actor_sim_data;
mod stepper;
mod unstuck;

mod world_helpers;
pub struct CharacterMotionPlugin;
mod apply_animation_motion;
pub mod physics_setup;
pub use apply_animation_motion::calculate_animation_displacement_system;
use bevy::prelude::*;
pub use components::CreaturePhysicsState;
pub use movement::interpolate_visual_positions;
use physics_setup::setup_nif_physics;
pub use prepare_actor_sim_data::prepare_actor_simulation;
pub use unstuck::unstuck_character;
#[derive(Component)]
pub struct CreatureControllerLink(pub Entity);
impl Plugin for CharacterMotionPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, setup_nif_physics);
        // `calculate_animation_displacement_system`: Produces `AnimationDisplacement` from animations.
        // `prepare_character_velocity_system`: Consumes `AnimationDisplacement` and player input to produce a final `velocity_for_frame`.
        // `character_movement_system`: Consumes `velocity_for_frame` to move the character and resolve all collisions.
        // The entire chain has to run before `PhysicsSet::SyncBackend` so Rapier can see the final, moved transform.
    }
}
