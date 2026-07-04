use crate::{
    defs::{LandscapeManagementSettings, LoadedLandscapeEntities},
    landscape::{
        landscape_loader::{
            poll_landscape_textures_array_ready, poll_sampled_landscape_images, poll_task_finished,
        },
        terrain_material::TerrainBlendMaterial,
    },
};
use bevy::prelude::*;
use esm_parser_lib::Landscape;
use landscape_loader::{load_landscape, setup_landscape_images};
use landscape_spawning_despawning::spawn_despawn_landscapes;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
pub mod landscape_loader;
pub mod landscape_spawning_despawning;
pub mod terrain_material;

pub static ALL_LANDSCAPES: OnceLock<RwLock<HashMap<(i32, i32), Landscape>>> = OnceLock::new();
pub struct LandscapePlugin;
impl Plugin for LandscapePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TerrainBlendMaterial>::default())
            .insert_resource(LandscapeManagementSettings {
                load_radius_cells: 3,
                unload_radius_cells: 4,
            })
            .insert_resource(LoadedLandscapeEntities::default())
            .add_systems(Update, setup_landscape_images)
            .add_systems(Update, spawn_despawn_landscapes)
            .add_systems(Update, poll_task_finished)
            .add_systems(Update, poll_sampled_landscape_images)
            .add_systems(Update, poll_landscape_textures_array_ready)
            .add_observer(load_landscape);
    }
}
