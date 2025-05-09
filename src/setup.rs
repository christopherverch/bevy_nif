use crate::nif::attach_parts::AttachmentType;
use crate::nif::loader::Nif;
use crate::nif::spawner::NifScene;
use bevy::prelude::*;
use bevy_third_person_camera::*;

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut scene_entities_by_name: ResMut<SceneEntitiesByName>,
) {
    let entity = commands
        .spawn((
            InheritedVisibility::VISIBLE,
            Transform::from_xyz(0.0, -3.5, 1.0),
        ))
        .id();
    let entity_player = commands
        .spawn((
            InheritedVisibility::VISIBLE,
            Transform::from_xyz(2.0, -1.0, 0.0),
        ))
        .id();
    //"data/B_N_Dark Elf_F_Forearm.nif",
    let paths = [
        "data/base_anim.nif",
        "data/B_N_Dark Elf_M_Neck.nif",
        "data/B_N_Dark Elf_M_Head_01.nif",
        "data/b_n_dark elf_m_skins.nif",
    ]
    .to_vec();
    let asset_names = ["skeleton", "neck", "head", "torso"].to_vec();
    let attachment_types = [
        AttachmentType::Skinned,
        AttachmentType::Rigid {
            target_bone: "Neck".to_string(),
        },
        AttachmentType::Rigid {
            target_bone: "Head".to_string(),
        },
        AttachmentType::Skinned,
    ]
    .to_vec();
    spawn_gltfs(
        paths,
        asset_server,
        &mut scene_entities_by_name,
        asset_names,
        entity,
        entity_player,
        &mut commands,
        0,
        Transform::from_xyz(0.0, 0.0, 0.0),
        attachment_types,
    );
}
use std::collections::HashMap;
use std::f32::consts::PI;

use bevy::render::mesh::Mesh;
use bevy::{
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
    },
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver},
};
#[derive(Default, Resource, Debug)]
pub struct SceneEntitiesByName(pub HashMap<(String, u64), Entity>);
#[derive(Default, Resource)]
pub struct GameAssets {
    pub gltf_files: HashMap<String, Handle<Nif>>,
}
#[derive(Component)]
pub struct SceneName {
    pub scene_name: String,
    pub id: u64,
    pub parent: Entity,
}
fn spawn_gltfs(
    paths: Vec<&str>,
    asset_server: Res<AssetServer>,
    scene_entities_by_name: &mut ResMut<SceneEntitiesByName>,
    asset_names: Vec<&str>,
    entity: Entity,
    parent_entity: Entity,
    commands: &mut Commands,
    client_id: u64,
    transform: Transform,
    mut attachment_type: Vec<AttachmentType>,
) {
    for i in 0..paths.len() {
        let asset_handle = asset_server.load(paths[i]);
        spawn_gltf_as_child(
            asset_handle,
            scene_entities_by_name,
            &asset_names[i],
            entity,
            parent_entity,
            commands,
            client_id,
            transform,
            attachment_type.remove(0),
        );
    }
}
pub fn spawn_gltf_as_child(
    asset_handle: Handle<Nif>,
    scene_entities_by_name: &mut ResMut<SceneEntitiesByName>,
    asset_name: &str,
    entity: Entity,
    parent_entity: Entity,
    commands: &mut Commands,
    client_id: u64,
    transform: Transform,
    attachment_type: AttachmentType,
) {
    println!("trying to spawn asset: {}", asset_name);
    //let scene = gltf.default_scene.clone().unwrap();
    println!("player id: {}", entity);
    commands.entity(entity).with_children(|parent| {
        let player_skeleton_entity = parent
            .spawn((
                InheritedVisibility::VISIBLE,
                NifScene(asset_handle.clone()),
                transform,
                SceneName {
                    scene_name: asset_name.to_string(),
                    id: client_id,
                    parent: parent_entity,
                },
                attachment_type,
            ))
            .id();
        scene_entities_by_name.0.insert(
            (asset_name.to_string(), (client_id)),
            player_skeleton_entity,
        );
        println!("inserting entity with id: {}", player_skeleton_entity);
    });
}

pub fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            // Deferred both supports both hdr: true and hdr: false
            hdr: true,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: PI / 3.6,
            aspect_ratio: 1.0,
            near: 0.1,
            far: 1000.0,
        }),
        ThirdPersonCamera::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        // MSAA needs to be off for Deferred rendering
        Msaa::Off,
        DepthPrepass,
        MotionVectorPrepass,
        DeferredPrepass,
        Fxaa::default(),
    ));
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font,
            font_size: 00.0,
            ..default()
        },
    ));
    let transform_almost_down = Transform::from_rotation(Quat::from_axis_angle(
        Vec3::X,
        std::f32::consts::PI / 2.0 * -0.98,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 600.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.85, 0.7), // Slightly warm (orangey/yellowish) white
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
        transform_almost_down,
    ));

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
        TextLayout::new_with_justify(JustifyText::Left),
        // Set the style of the Node itself.
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}
