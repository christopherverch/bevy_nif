use bevy::prelude::*;
mod modular_characters;
pub mod nif;
use nif::animation::{SkeletonMap, build_animation_clip_system};
use nif::attach_parts::attach_parts;
use nif::loader::{BMPLoader, Nif, NifAssetLoader};
use nif::spawner::spawn_nif_scenes;
pub use nif::types::*;
pub struct BevyNifPlugin;
impl Plugin for BevyNifPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Nif>()
            .init_asset_loader::<NifAssetLoader>()
            .init_asset_loader::<BMPLoader>()
            .insert_resource(SkeletonMap::default())
            .add_observer(attach_parts)
            .add_systems(Update, build_animation_clip_system.after(spawn_nif_scenes))
            .add_systems(Update, spawn_nif_scenes);
    }
}
