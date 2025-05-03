//! Implements loader for a custom asset type.
mod error;
mod nif;
mod parser;
mod setup;
mod setup_funcs;
mod structs;
use crate::structs::*;
use bevy::asset::RenderAssetUsages;
use bevy::pbr::ExtendedMaterial;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::render::render_resource::Face;
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_nif::ColorKeyExtension;
use bevy_third_person_camera::*;
use error::ParseError;
use nif::*;
use parser::parse_nif_start;
use setup_funcs::*;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
#[derive(Component)]
struct NifSceneRoot {
    source_asset: Handle<ParsedNifData>,
}
#[derive(Default)]
struct NifAssetLoader;

impl AssetLoader for NifAssetLoader {
    type Asset = ParsedNifData;
    type Settings = ();
    type Error = ParseError;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        parse_nif_start(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["nif"]
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<State>()
        .init_asset::<ParsedNifData>()
        .init_asset_loader::<NifAssetLoader>()
        .init_asset::<ColorKeyExtension>()
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: true,
        })
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, ColorKeyExtension>,
        >::default())
        .add_plugins(ThirdPersonCameraPlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(Startup, setup)
        .add_systems(Update, spawn_nif_scenes_observer)
        .run();
}

fn print_on_load(mut state: ResMut<State>, custom_assets: Res<Assets<ParsedNifData>>) {
    let custom_asset = custom_assets.get(&state.handle);
    let custom_asset2 = custom_assets.get(&state.handle2);

    // Can't print results if the assets aren't ready
    if state.printed {
        return;
    }

    if custom_asset.is_none() {
        info!("Custom Asset Not Ready");
        return;
    }

    info!("Custom asset loaded: {:?}", custom_asset.unwrap());
    info!("Custom asset loaded: {:?}", custom_asset2.unwrap());

    // Once printed, we won't print again
    state.printed = true;
}
fn resolve_nif_path(nif_path: &str) -> String {
    // Basic cleanup - Needs proper implementation!
    let cleaned = nif_path.trim().replace('\\', "/");
    if !cleaned.starts_with("textures/") && !cleaned.is_empty() {
        format!("textures/{}", cleaned)
    } else {
        cleaned
    }
}
fn convert_nif_mesh(data: &NiTriShapeData) -> Option<Mesh> {
    // Check if essential data exists
    let vertices_nif = data.tri_base.geom_base.vertices.as_ref()?;
    let indices_nif = &data.triangles;

    if vertices_nif.is_empty() || indices_nif.is_empty() {
        return None; // No geometry
    }

    // Convert vertex positions
    let converted_vertices: Vec<[f32; 3]> = vertices_nif.iter().map(|v| v.0).collect();

    // Convert normals (if they exist)
    let converted_normals: Option<Vec<[f32; 3]>> = data
        .tri_base
        .geom_base
        .normals
        .as_ref()
        .map(|n| n.iter().map(|v| v.0).collect());

    // Convert UVs (use the first UV set if it exists)
    let converted_uvs: Option<Vec<[f32; 2]>> = data
        .tri_base
        .geom_base
        .uv_sets
        .get(0) // Get the first UV set
        .map(|uv_set| uv_set.iter().map(|v| v.0).collect());

    // Create the Bevy Mesh
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, converted_vertices);
    if let Some(normals) = converted_normals {
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    } else {
        // Generate flat normals if missing (or handle differently)
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
    }
    if let Some(uvs) = converted_uvs {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }
    // NIF uses u16 indices
    mesh.insert_indices(Indices::U16(indices_nif.clone()));

    // TODO: Add vertex colors (Mesh::ATTRIBUTE_COLOR) if data.has_vertex_colors is true

    // Potential Coordinate System Conversion:
    // If NIF is Z-up, Left-handed and Bevy is Y-up, Right-handed,
    // you might need to swizzle/negate vertex positions, normals, and adjust transforms.
    // mesh.transform_vertices(...) could be useful here *after* setting attributes.

    Some(mesh)
}

// Helper to convert NIF transform to Bevy transform
fn convert_nif_transform(nif_transform: &NiTransform) -> Transform {
    // --- 1. Extract NIF data into Bevy types (in NIF's coordinate space) ---

    // NIF Rotation Matrix to Bevy Quaternion
    // This assumes your Mat3::from_cols construction correctly interprets the NIF matrix layout.
    // It constructs a Bevy Mat3 using columns derived from the NIF data.
    let rot_mat = Mat3::from_cols(
        Vec3::new(
            nif_transform.rotation.0[0][0],
            nif_transform.rotation.0[1][0],
            nif_transform.rotation.0[2][0],
        ), // First Column
        Vec3::new(
            nif_transform.rotation.0[0][1],
            nif_transform.rotation.0[1][1],
            nif_transform.rotation.0[2][1],
        ), // Second Column
        Vec3::new(
            nif_transform.rotation.0[0][2],
            nif_transform.rotation.0[1][2],
            nif_transform.rotation.0[2][2],
        ), // Third Column
    );
    let initial_rotation = Quat::from_mat3(&rot_mat);

    let initial_translation = Vec3::from_array(nif_transform.translation.0);
    // Assuming uniform scale. If NIF has non-uniform scale, it would need handling here too.
    let initial_scale = Vec3::splat(nif_transform.scale);

    // This transform represents the object in NIF's coordinate system (e.g., Z-up)
    let nif_space_transform = Transform {
        translation: initial_translation,
        rotation: initial_rotation,
        scale: initial_scale,
    };

    // --- 2. Define the Coordinate System Correction ---
    // We need to rotate the NIF coordinate system (assumed Z-up, Y-forward)
    // into Bevy's coordinate system (Y-up, -Z forward).
    // This requires a -90 degree rotation around the X axis.
    let correction_rotation = Quat::from_rotation_x(-FRAC_PI_4); // -90 degrees in radians
    let correction_transform = Transform::from_rotation(correction_rotation);

    // --- 3. Apply the Correction ---
    // Multiply the correction by the original transform. This transforms the object
    // from NIF space into Bevy space.
    let bevy_space_transform = correction_transform * nif_space_transform;

    // --- 4. Return the corrected transform ---
    bevy_space_transform
}
