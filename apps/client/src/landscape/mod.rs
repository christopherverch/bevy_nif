use crate::defs::{LandscapeManagementSettings, LoadedLandscapeEntities};
use bevy::prelude::*;
use landscape_loader::{load_landscape, setup_landscape_images};
use landscape_spawning_despawning::spawn_despawn_landscapes;
pub mod landscape_loader;
pub mod landscape_spawning_despawning;
pub mod terrain_material;
pub struct LandscapePlugin;

impl Plugin for LandscapePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LandscapeManagementSettings {
            load_radius_cells: 1,
            unload_radius_cells: 1,
        })
        .insert_resource(LoadedLandscapeEntities::default())
        .add_systems(Update, load_landscape)
        .add_systems(Update, setup_landscape_images)
        .add_systems(Update, spawn_despawn_landscapes);
    }
}
