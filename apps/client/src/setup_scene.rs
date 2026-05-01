use crate::defs::{PlayerCamera, TestingComponent};
use bevy::{
    core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
    light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver},
    pbr::OpaqueRendererMethod,
    prelude::*,
    render::view::Hdr,
};
use bevy_anti_alias::fxaa::Fxaa;
use bevy_nif::{loader::Nif, spawner::NifScene};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

#[derive(Component)]
pub struct HudText;

use crate::camera::MWCamera;
pub fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let start_location = Vec3::new(185.0, 4.8, -890.0);
    let glass_helmet_pos = Vec3::new(195.0, 4.8, -890.0);
    let head_handle = asset_server.load("data/A_Glass_Helmet.nif");
    spawn_nif_unattached(&mut commands, head_handle, glass_helmet_pos);
    let head = asset_server.load("data/Data Files/Meshes/b/B_N_High Elf_M_Head_01.nif");
    let hair = asset_server.load("data/Data Files/Meshes/b/B_N_High Elf_M_Hair_04.nif");
    commands.spawn((
        Name::new("head"),
        NifScene(head),
        Transform {
            translation: start_location,
            rotation: Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(PI),
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
    commands.spawn((
        Name::new("hair"),
        NifScene(hair),
        Transform {
            translation: start_location,
            rotation: Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(PI),
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
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
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 10.0,
                end: 40.0,
            },
            ..default()
        },
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
            font,
            font_size: 100.0,
            ..default()
        },
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 500.,
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 10.0,
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
        SceneRoot(helmet_scene),
        Transform::from_translation(start_location_moved),
    ));
    let start_location_moved2 = Vec3::new(190.0, 4.8, -898.0);
    let woodelf_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/woodelf.glb"));
    commands.spawn((
        SceneRoot(woodelf_scene),
        Transform::from_translation(start_location_moved2),
    ));
    let start_location_moved3 = Vec3::new(190.0, 4.8, -910.0);
    let darkelf_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/m2/dark_elf/B_N_Dark_Elf_M_Hair_01.glb"));
    commands.spawn((
        Visibility::Visible,
        SceneRoot(darkelf_scene),
        Transform::from_translation(start_location_moved3),
    ));
    let darkelf_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/m2/dark_elf/B_N_Dark_Elf_M_Head_01.glb"));
    commands.spawn((
        Visibility::Visible,
        SceneRoot(darkelf_scene),
        Transform::from_translation(start_location_moved3),
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
            shadows_enabled: true,
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
            font: font_handle,
            font_size: 25.0,
            ..default()
        },
        // Set the justification of the Text
        TextLayout::new_with_justify(Justify::Left),
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

pub fn spawn_nif_unattached(commands: &mut Commands, nif_handle: Handle<Nif>, pos: Vec3) {
    commands.spawn((
        NifScene(nif_handle),
        Visibility::Visible,
        Transform {
            translation: pos,
            rotation: Quat::from_rotation_y(PI),
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
}
