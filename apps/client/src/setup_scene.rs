use crate::{
    const_defs::CELL_SIZE_METERS,
    defs::{CellLookup, ESM_READER, MWCell, PlayerCamera, TestingComponent},
};
use bevy::{
    camera::Hdr,
    core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
    light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver},
    material::OpaqueRendererMethod,
    prelude::*,
};
use bevy_anti_alias::fxaa::Fxaa;
use bevy_nif::{
    MW_SCALE_TO_BEVY, NIF_TO_BEVY_ROTATION, check_exists, clean_path_common, loader::Nif,
    resolve_bevy_asset_path, spawner::NifScene,
};
use common::consts::MW_UNITS_PER_METER;
use esm_parser_lib::{
    ESM3ExteriorCellRefId, EmptyRefId, EsmReader, RefId,
    types::{cell::Cell, cell_ref::CellRef},
};
use esm_store::EsmStore;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

#[derive(Component)]
pub struct HudText;

use crate::camera::MWCamera;
pub fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    _esm_store: Res<EsmStore>,
) {
    let _esm_reader = ESM_READER.get().unwrap().write().unwrap();

    let start_location = Vec3::new(185.0, 4.8, -890.0);
    let glass_helmet_pos = Vec3::new(195.0, 4.8, -890.0);
    let head_handle = asset_server.load("data/A_Glass_Helmet.nif");
    spawn_nif_unattached(
        &mut commands,
        head_handle,
        glass_helmet_pos,
        Quat::default(),
    );
    test_spawn("Terrain_rock_BC_15.nif", &asset_server, &mut commands, 0.0);
    test_spawn("Terrain_rock_BC_14.nif", &asset_server, &mut commands, 15.0);
    test_spawn("Terrain_rock_BC_18.nif", &asset_server, &mut commands, 30.0);
    test_spawn(
        "Terrain_rock_BC_12.nif",
        &asset_server,
        &mut commands,
        -15.0,
    );

    let head = asset_server.load("data/Data Files/Meshes/b/B_N_High Elf_M_Head_01.nif");
    let hair = asset_server.load("data/Data Files/Meshes/b/B_N_High Elf_M_Hair_04.nif");
    commands.spawn((
        Name::new("head"),
        NifScene(head),
        Visibility::Visible,
        Transform {
            translation: start_location,
            rotation: Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(PI),
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
    commands.spawn((
        Name::new("hair"),
        Visibility::Visible,
        NifScene(hair),
        Transform {
            translation: start_location,
            rotation: Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(PI),
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
    let cell_entity = commands
        .spawn(MWCell {
            has_water: false,
            water_level: 0.0,
        })
        .id();
    let mut cell_lookup = CellLookup::default();
    cell_lookup.0.insert(UVec2::ZERO, cell_entity);
    commands.insert_resource(cell_lookup);
    let camera_start_pos = Vec3::new(190.0, 2.8, -890.0);
    commands.spawn((
        Camera3d::default(),
        Camera::default(),
        Hdr,
        Projection::Perspective(PerspectiveProjection {
            fov: PI / 3.6,
            aspect_ratio: 1.0,
            near: 0.1,
            far: 1000.0,
            ..default()
        }),
        PlayerCamera,
        MWCamera::default(),
        Transform::from_translation(camera_start_pos).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        // MSAA needs to be off for Deferred rendering
        Msaa::Off,
        AmbientLight {
            color: Color::linear_rgb(1.0, 1.0, 1.0),
            brightness: 500.0,
            affects_lightmapped_meshes: true,
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 0.0,
            ..default()
        },
        DepthPrepass,
        MotionVectorPrepass,
        DeferredPrepass,
        Fxaa {
            enabled: true,
            ..default()
        },
    ));
    commands.spawn((
        TestingComponent,
        Transform::from_translation(start_location),
    ));
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");

    let start_location_moved = Vec3::new(190.0, 4.8, -895.0);
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Px(100.0),
            ..default()
        },
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::linear_rgb(1.0, 0.7, 0.7),
            illuminance: 2000.,
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 100.0,
            ..default()
        }
        .build(),
        Transform {
            translation: start_location_moved,
            rotation: Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4),
            ..default()
        },
    ));
    // FlightHelmet
    let helmet_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf"));

    commands.spawn((
        WorldAssetRoot(helmet_scene),
        Transform::from_translation(start_location_moved),
    ));

    let mut forward_mat: StandardMaterial = Color::srgb(0.1, 0.2, 0.1).into();
    forward_mat.opaque_render_method = OpaqueRendererMethod::Forward;

    let sphere_h = meshes.add(Sphere::new(0.125).mesh().uv(32, 18));

    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    // Emissive sphere
    let mut unlit_mat: StandardMaterial = sphere_color.into();
    unlit_mat.unlit = true;
    let start_location_moved4 = Vec3::new(192.0, 4.8, -890.0);
    commands.spawn((
        Mesh3d(sphere_h.clone()),
        MeshMaterial3d(materials.add(unlit_mat)),
        NotShadowCaster,
        Transform::from_translation(start_location_moved4),
    ));

    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    // Light
    commands.spawn((
        PointLight {
            intensity: 800.0,
            radius: 0.125,
            shadow_maps_enabled: true,
            color: sphere_color,
            ..default()
        },
        Transform::from_translation(start_location_moved4),
    ));

    // Spheres
    for i in 0..6 {
        let j = i % 3;
        let s_val = if i < 3 { 0.0 } else { 0.2 };
        let material = if j == 0 {
            materials.add(StandardMaterial {
                base_color: Color::srgb(s_val, s_val, 1.0),
                perceptual_roughness: 0.089,
                metallic: 0.0,
                ..default()
            })
        } else if j == 1 {
            materials.add(StandardMaterial {
                base_color: Color::srgb(s_val, 1.0, s_val),
                perceptual_roughness: 0.089,
                metallic: 0.0,
                ..default()
            })
        } else {
            materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, s_val, s_val),
                perceptual_roughness: 0.089,
                metallic: 0.0,
                ..default()
            })
        };
        commands.spawn((
            Mesh3d(sphere_h.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(
                start_location_moved4.x
                    + (j as f32 * 0.25 + if i < 3 { -0.15 } else { 0.15 } - 0.4),
                start_location_moved4.y + 0.125,
                start_location_moved4.z
                    + (-j as f32 * 0.25 + if i < 3 { -0.15 } else { 0.15 } + 0.4),
            ),
        ));
    }

    // sky
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("888888").unwrap().into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        NotShadowCaster,
        NotShadowReceiver,
    ));

    // Example instructions
    let font_handle = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn((
        // Accepts a `String` or any type that converts into a `String`, such as `&str`
        Text::new("test"),
        TextFont {
            // This font is loaded and will be used instead of the default font.
            font: FontSource::Handle(font_handle),
            font_size: FontSize::Px(25.0),
            ..default()
        },
        // Set the justification of the Text
        TextLayout::justify(Justify::Left),
        // Set the style of the Node itself.
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));
}
fn test_build_seyda_neen(
    store: &EsmStore,
    commands: &mut Commands,
    esm_reader: &mut EsmReader,
    asset_server: &AssetServer,
) {
    let cell = store
        .cells
        .get(&RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(-2, -9)))
        .unwrap();

    cell.restore(esm_reader, 0).unwrap();
    dbg!(&cell);
    let mut cell_ref = CellRef::default();
    let mut is_deleted = false;
    let parent = commands
        .spawn((Transform::default(), Name::new("PARENT")))
        .id();
    while let Ok(has_ref) = Cell::get_next_ref(esm_reader, &mut cell_ref, &mut is_deleted) {
        if !has_ref {
            return;
        }
        if cell_ref.id != RefId::Empty(EmptyRefId) {
            dbg!(&cell_ref.id);
            if let Some(stat) = store.statics.get(&cell_ref.id) {
                let pos = cell_ref.pos.pos;
                let path = format!("data/Data Files/Meshes/{}", stat.model);
                let path = clean_path_common(&path);
                let path = if check_exists(&path) {
                    path
                } else {
                    resolve_bevy_asset_path(&path).unwrap()
                };
                dbg!(&path);
                let handle = asset_server.load(path);
                let mut pos = pos / MW_UNITS_PER_METER;
                let tmp = pos.z;
                pos.z = pos.y;
                pos.y = tmp;
                pos.y = tmp - 1.0 / MW_UNITS_PER_METER;
                pos.x = -pos.x;
                pos.x += CELL_SIZE_METERS / 2.0;
                pos.z -= CELL_SIZE_METERS / 2.0;
                dbg!(pos);
                let rot = cell_ref.pos.rot;
                dbg!(rot);
                let rot = Quat::from_euler(EulerRot::XYZ, -rot.x, rot.y, -rot.z);
                let nif = spawn_nif_unattached(commands, handle, pos, rot);
                commands.entity(parent).add_child(nif);
            }
        }
    }
}

pub fn spawn_nif_unattached(
    commands: &mut Commands,
    nif_handle: Handle<Nif>,
    pos: Vec3,
    rotation: Quat,
) -> Entity {
    commands
        .spawn((
            NifScene(nif_handle),
            Visibility::Visible,
            Transform {
                translation: pos,
                rotation: NIF_TO_BEVY_ROTATION * rotation,
                scale: MW_SCALE_TO_BEVY,
            },
        ))
        .id()
}
fn test_spawn(name: &str, asset_server: &AssetServer, commands: &mut Commands, offset: f32) {
    let path = format!("data/Data Files/Meshes/f/{}", name);
    let path = clean_path_common(&path);
    let path = if check_exists(&path) {
        path
    } else {
        resolve_bevy_asset_path(&path).unwrap()
    };
    let player_start_location = Vec3::new(
        2.0 * CELL_SIZE_METERS + offset,
        2.8,
        -9.0 * CELL_SIZE_METERS,
    );
    let rock = asset_server.load(path);
    spawn_nif_unattached(commands, rock, player_start_location, Quat::default());
}
