use bevy::prelude::*;
pub mod attach_parts;
pub mod helper_funcs;
pub mod loader;
pub mod nif_animation;
pub mod skeleton;
pub mod spawner;
pub mod spawning_ni_helpers;
use attach_parts::attach_parts;
use loader::{BMPLoader, Nif, NifAssetLoader};
use nif::loader::NiKey;
pub use nif::types::*;
use nif_animation::SkeletonMap;
use nif_animation::animation_setup_system::setup_animations;
use spawner::spawn_nif_scenes;
#[derive(Component)]
pub struct NeedsNifPhysics(pub Vec<(Entity, NiKey)>);
pub struct BevyNifPlugin;
impl Plugin for BevyNifPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Nif>()
            .init_asset_loader::<NifAssetLoader>()
            .init_asset_loader::<BMPLoader>()
            .insert_resource(SkeletonMap::default())
            .add_observer(attach_parts)
            .add_systems(Update, spawn_nif_scenes)
            .add_systems(Update, setup_animations);
    }
}
