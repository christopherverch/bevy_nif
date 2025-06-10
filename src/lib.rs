use bevy::prelude::*;
pub mod nif;
pub mod nif_animation;
use nif::attach_parts::attach_parts;
use nif::loader::{BMPLoader, Nif, NifAssetLoader};
use nif::spawner::spawn_nif_scenes;
pub use nif::types::*;
use nif_animation::SkeletonMap;
use nif_animation::animation_setup_system::setup_animations;
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
