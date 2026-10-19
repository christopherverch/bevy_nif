pub mod attach_parts;
pub mod helper_funcs;
pub mod loader;
pub mod nif_animation;
pub mod skeleton;
pub mod spawner;
pub mod spawning_ni_helpers;

use crate::loader::DDSLoader;
use attach_parts::attach_parts;
use avian3d::PhysicsPlugins;
use avian3d::prelude::PhysicsSchedulePlugin;
use bevy_app::{App, Plugin, PreUpdate, Update};
use bevy_asset::AssetApp;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_math::Vec3;
pub use helper_funcs::*;
use loader::{BMPLoader, Nif, NifAssetLoader};
pub use nif::types::*;
use nif_animation::SkeletonMap;
use nif_animation::animation_setup_system::setup_animations;
use spawner::spawn_nif_scenes;

pub const MW_UNITS_PER_METER: f32 = 69.991251091;
pub const MW_SCALE_TO_BEVY: Vec3 = Vec3::splat(1.0 / MW_UNITS_PER_METER);

#[derive(Component)]
pub struct NeedsNifPhysics(pub Vec<(Entity, NiKey)>);
pub struct BevyNifPlugin;
impl Plugin for BevyNifPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Nif>()
            .init_asset_loader::<NifAssetLoader>()
            .init_asset_loader::<BMPLoader>()
            .init_asset_loader::<DDSLoader>()
            .insert_resource(SkeletonMap::default())
            .add_observer(attach_parts)
            .add_systems(Update, spawn_nif_scenes)
            .add_systems(PreUpdate, setup_animations);
        // check for one of avian's core plugins to check if the user already added the
        // physicsplugin
        if !app.is_plugin_added::<PhysicsSchedulePlugin>() {
            app.add_plugins(PhysicsPlugins::default());
        }
    }
}
