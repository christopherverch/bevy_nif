use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_third_person_camera::*;
mod setup;

mod nif;
pub use nif::types::*;
use nif::{loader::*, spawner::spawn_nif_scenes};
#[allow(dead_code)]
#[derive(Event, Clone, Debug)]
pub struct NifInstantiated(pub Handle<Nif>);
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
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: true,
        })
        .add_plugins(BevyNifPlugin)
        .insert_resource(AmbientLight {
            // Add ambient light resource
            color: Color::srgb(1.0, 0.8, 0.6), // Match warm tone
            brightness: 100.0,
            affects_lightmapped_meshes: true, // Keep low! Adjust if shadows need deepening (e.g., 0.03)
        })
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(ThirdPersonCameraPlugin)
        .add_systems(Startup, setup::setup_scene)
        .add_systems(Startup, setup::setup)
        .add_systems(Update, rotate)
        .add_observer(print_on_nif_instantiated)
        .run();
}
fn rotate(mut query: Query<&mut Transform, With<LoadedNifScene>>) {
    // Can't print results if the assets aren't ready
    for mut nif in query.iter_mut() {
        let nif_rotation = Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0).normalize(), 0.02);
        nif.rotate(nif_rotation);
    }
}
fn print_on_nif_instantiated(_trigger: Trigger<NifInstantiated>) {}
