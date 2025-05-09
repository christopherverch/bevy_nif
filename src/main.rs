use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_third_person_camera::*;
mod modular_characters;
mod nif;
mod setup;
pub use nif::types::*;
use nif::{
    animation::{BoneMap, build_animation_clip_system},
    attach_parts::attach_parts,
    loader::*,
    spawner::spawn_nif_scenes,
};
use setup::SceneEntitiesByName;
#[allow(dead_code)]
#[derive(Event, Clone, Debug)]
pub struct NifInstantiated {
    pub handle: Handle<Nif>,
    pub root_entity: Entity,
}
#[allow(dead_code)]
#[derive(Component)]
pub struct LoadedNifScene(pub Handle<Nif>);
pub struct BevyNifPlugin;
impl Plugin for BevyNifPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Nif>()
            .init_asset_loader::<NifAssetLoader>()
            .init_asset_loader::<BMPLoader>()
            .add_systems(Update, spawn_nif_scenes);
    }
}
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin {})
        .add_plugins(BevyNifPlugin)
        .insert_resource(SceneEntitiesByName::default())
        .insert_resource(BoneMap::default())
        .insert_resource(AmbientLight {
            // Add ambient light resource
            color: Color::srgb(1.0, 0.8, 0.6), // Match warm tone
            brightness: 100.0,
        })
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(ThirdPersonCameraPlugin)
        .add_systems(Startup, setup::setup_scene)
        .add_systems(Startup, setup::setup)
        .add_observer(attach_parts)
        .add_systems(Update, build_animation_clip_system.after(spawn_nif_scenes))
        .run();
}
