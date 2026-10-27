pub mod attach_parts;
pub mod helper_funcs;
pub mod loader;
mod loader_spawners;
pub mod nif_animation;
mod physics;
pub mod skeleton;
pub mod spawning_ni_helpers;
use avian3d::PhysicsPlugins;
use avian3d::prelude::PhysicsSchedulePlugin;
use bevy_app::{App, Plugin, Update};
use bevy_asset::AssetApp;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::EntityEvent;
use bevy_ecs::reflect::ReflectComponent;
use bevy_math::Vec3;
use bevy_reflect::Reflect;
pub use helper_funcs::*;
use loader::{BMPLoader, Nif, NifAssetLoader};
pub use nif::types::*;

use crate::attach_parts::{attach_parts_obs, insert_into_skeleton_map};
use crate::loader::DDSLoader;
use crate::loader_spawners::NifBox;
use crate::physics::{handle_collider_tasks, spawn_collider_tasks};
use crate::skeleton::{SkeletonIdManager, SkeletonMap};

pub const MW_UNITS_PER_METER: f32 = 69.991251091;
pub const MW_SCALE_TO_BEVY: Vec3 = Vec3::splat(1.0 / MW_UNITS_PER_METER);

#[derive(Component)]
pub struct NeedsNifPhysics(pub Vec<(Entity, NiKey)>);
pub struct BevyNifPlugin;
impl Plugin for BevyNifPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Nif>()
            .preregister_asset_loader::<NifAssetLoader>(&["nif", "kf"])
            .register_asset_loader(NifAssetLoader)
            .init_asset_loader::<BMPLoader>()
            .init_asset_loader::<DDSLoader>()
            .init_resource::<SkeletonIdManager>()
            .init_resource::<SkeletonMap>()
            .add_observer(spawn_collider_tasks)
            .add_observer(attach_parts_obs)
            .add_observer(insert_into_skeleton_map);
        app.add_systems(Update, handle_collider_tasks);
        // check for one of avian's core plugins to check if the user already added the
        // physicsplugin
        if !app.is_plugin_added::<PhysicsSchedulePlugin>() {
            app.add_plugins(PhysicsPlugins::default());
        }
    }
}
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub enum NifColliderData {
    // Just so we can mem::take
    #[default]
    Taken,
    Trimesh(Vec<Vec3>, Vec<[u32; 3]>),
    // TODO:: this should just be compound, with different shapes as an enum
    CompoundBox(Vec<NifBox>),
}
#[derive(EntityEvent)]
pub struct NifInstanceReady(pub Entity);
