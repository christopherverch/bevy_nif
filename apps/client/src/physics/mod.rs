use avian3d::prelude::PhysicsSystems;
use bevy::app::Plugin;

mod components;
mod constants;
mod helpers;
pub mod movement;
mod prepare_actor_sim_data;
mod stepper;
mod unstuck;

mod apply_animation_motion;
pub mod physics_setup;
mod world_helpers;
pub use apply_animation_motion::*;
use bevy::prelude::*;
pub use components::CreaturePhysicsState;
pub use movement::interpolate_visual_positions;
use physics_setup::setup_nif_physics;
pub use prepare_actor_sim_data::prepare_actor_simulation;
pub use unstuck::unstuck_character;

use crate::{
    mechanics::{update_character_components, update_movement_component_from_intent},
    physics::movement::{
        character_movement_system, init_positions, sync_actor_physics_system, update_positions,
    },
};
#[derive(Component)]
pub struct CreatureControllerLink(pub Entity);

pub struct MWPhysicsPlugin;
impl Plugin for MWPhysicsPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(Update, setup_nif_physics).add_systems(
            FixedUpdate,
            (
                update_movement_component_from_intent,
                update_character_components,
                prepare_actor_simulation,
                init_positions,
                unstuck_character,
                character_movement_system,
                update_positions,
                sync_actor_physics_system,
                calculate_animation_displacement_system,
            )
                .chain()
                .before(PhysicsSystems::Prepare),
        );
    }
}
