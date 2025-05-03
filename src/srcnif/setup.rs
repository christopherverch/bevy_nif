// src/setup.rs
use crate::nif::ParsedNifData; // Needs to know about the asset type
use bevy::prelude::*;
use bevy_third_person_camera::*; // Needs camera types if used here
// Import setup_funcs if they are defined elsewhere, or define here
// use crate::setup_funcs::*; // Assuming setup_funcs module exists

#[derive(Resource, Default)]
struct State {
    handle: Handle<ParsedNifData>,
    handle2: Handle<ParsedNifData>,
    printed: bool,
}

pub fn setup(
    mut commands: Commands,
    mut state: ResMut<State>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let cube_handle = asset_server.load(
        GltfAssetLabel::Primitive {
            mesh: 0,
            primitive: 0,
        }
        .from_asset("models/cube/cube.gltf"),
    );
    let material_handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.7, 0.6),
        ..default()
    });
    commands.spawn((
        Mesh3d(cube_handle),
        MeshMaterial3d(material_handle.clone()),
        Transform::from_xyz(20.0, 0.0, 0.0),
    ));
    state.handle = asset_server.load("data/B_N_Dark Elf_M_Head_01.nif");
    state.handle2 = asset_server.load("data/B_N_Dark Elf_M_Hair_01.nif");

    // File extensions are optional, but are recommended for project management and last-resort inference
    //   state.other_handle = asset_server.load("data/asset_no_extension");

    // Will use BlobAssetLoader instead of NifAssetLoader thanks to type inference
    // state.blob = asset_server.load("data/asset.custom");
}
