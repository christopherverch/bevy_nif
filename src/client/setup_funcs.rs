use bevy::asset::RenderAssetUsages;
use bevy::gltf::Gltf;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::{
    animation::AnimationTargetId,
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
        //prepass::NormalPrepass
    },
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver, OpaqueRendererMethod},
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_nif;
use bevy_nif::nif::animation::NifAnimator;
use bevy_nif::nif::attach_parts::AttachmentType;
use bevy_nif::nif::spawner::NifScene;
use bevy_rapier3d::prelude::*;
use cdda_bevy::client::defs::{
    AnimateableEntity, AnimationNodeConfig, Animator, CreatureStats, CurrentClientId,
    KnockdownEventAdded, LocalPlayer, LocalPlayerMesh, MaskGroup, MeshEntityLink, NeedsSetup,
    NetworkedEntity, NifAnimatorLink, Pause, Player, PlayerCamera, PlayerEntityLink, PlayerRoot,
    PlayerState, SceneEntitiesByName, SceneName, SpawnSceneState, Speed, TestingComponent,
};
use cdda_bevy::client::events::KnockdownFinished;
use cdda_bevy::client::third_person_camera::{ThirdPersonCamera, ThirdPersonCameraTarget};
use esm_parser_lib::{EsmReader, LAND_NUM_VERTS, LAND_SIDE_VERTICES, LandPayloadData};
use std::collections::HashMap;
use std::f32::consts::*;
use std::path::PathBuf;

use crate::actor_funcs::actor_take_hit;
#[derive(Component)]
pub struct HudText;

#[derive(Default, Resource)]
pub struct GameAssets {
    pub gltf_files: HashMap<String, Handle<Gltf>>,
}
const MASK_GROUP_PATHS_ARMS: [(&str, &str); 12] = [
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest",
        "Shoulder.L/UpperArm.L/LowerArm.L/Wrist.L",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.L/UpperArm.L/LowerArm.L/Wrist.L",
        "Index1.L/Index2.L/Index3.L/Index4.L",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.L/UpperArm.L/LowerArm.L/Wrist.L",
        "Middle1.L/Middle2.L/Middle3.L/Middle4.L",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.L/UpperArm.L/LowerArm.L/Wrist.L",
        "Ring1.L/Ring2.L/Ring3.L/Ring4.L",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.L/UpperArm.L/LowerArm.L/Wrist.L",
        "Pinky1.L/Pinky2.L/Pinky3.L/Pinky4.L",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.L/UpperArm.L/LowerArm.L/Wrist.L",
        "Thumb1.L/Thumb2.L/Thumb3.L/Thumb4.L",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.R",
        "UpperArm.R/LowerArm.R/Wrist.R",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.R/UpperArm.R/LowerArm.R/Wrist.R",
        "Index1.R/Index2.R/Index3.R/Index4.R",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.R/UpperArm.R/LowerArm.R/Wrist.R",
        "Middle1.R/Middle2.R/Middle3.R/Middle4.R",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.R/UpperArm.R/LowerArm.R/Wrist.R",
        "Ring1.R/Ring2.R/Ring3.R/Ring4.R",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.R/UpperArm.R/LowerArm.R/Wrist.R",
        "Pinky1.R/Pinky2.R/Pinky3.R/Pinky4.R",
    ),
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest/Shoulder.R/UpperArm.R/LowerArm.R/Wrist.R",
        "Thumb1.R/Thumb2.R/Thumb3.R/Thumb4.R",
    ),
];
const MASK_GROUP_PATHS_BODY_WITHOUT_ARMS: [(&str, &str); 7] = [
    (
        "CharacterArmature.001/Root/Body/Hips/Abdomen",
        "Torso/Chest/Neck/Head",
    ),
    ("CharacterArmature.001/Root/", "Foot.L"),
    ("CharacterArmature.001/Root/", "PT.L"),
    ("CharacterArmature.001/Root/", "Foot.R"),
    ("CharacterArmature.001/Root/", "PT.R"),
    ("CharacterArmature.001/Root/Body", "UpperLeg.L/LowerLeg.L"),
    ("CharacterArmature.001/Root/Body", "UpperLeg.R/LowerLeg.R"),
];
#[allow(dead_code)]
pub fn spawn_gltf_as_child(
    gltfs: &Res<Assets<Gltf>>,
    asset_handles: &Res<GameAssets>,
    scene_entities_by_name: &mut ResMut<SceneEntitiesByName>,
    entity: Entity,
    parent_entity: Entity,
    commands: &mut Commands,
    client_id: u64,
    mut transform: Transform,
    asset_names: Vec<&str>,
) {
    for asset_name in asset_names {
        let scale = if asset_name == "models/sword.glb" {
            0.1
        } else {
            1.0
        };
        println!("trying to spawn asset: {}", asset_name);
        if let Some(handle) = asset_handles.gltf_files.get(asset_name) {
            if let Some(gltf) = gltfs.get(handle) {
                let scene = gltf.default_scene.clone().unwrap();
                println!("player id: {}", entity);
                commands.entity(entity).with_children(|parent| {
                    transform.scale = Vec3::new(scale, scale, scale);
                    let player_skeleton_entity = parent
                        .spawn((
                            SceneRoot(scene.clone()),
                            transform,
                            SceneName {
                                scene_name: asset_name.to_string(),
                                id: client_id,
                                parent: parent_entity,
                            },
                        ))
                        .id();
                    scene_entities_by_name.0.insert(
                        (asset_name.to_string(), (client_id)),
                        player_skeleton_entity,
                    );
                    println!("inserting entity with id: {}", player_skeleton_entity);
                });
            }
        }
    }
}
pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &NeedsSetup)>,
    client_id: Res<CurrentClientId>,
    mut player_entity: ResMut<LocalPlayer>,
) {
    let collider_height = 1.2;
    for (entity, setup_info) in &mut query {
        println!("setting up");
        commands.entity(entity).insert((
            PlayerRoot,
            Player,
            Name::new(format!("Player {}", setup_info.id)),
            PlayerState {
                id: setup_info.id,
                ..Default::default()
            },
            AnimateableEntity,
            Transform::default(),
            Visibility::Visible,
            NetworkedEntity(entity),
            CreatureStats::new(setup_info.id),
        ));
        let player_mesh = commands
            .spawn((
                PlayerEntityLink(Some(entity)),
                Visibility::Visible,
                Transform::from_xyz(0.0, -collider_height / 1.3, 0.0),
            ))
            .id();
        commands
            .entity(entity)
            .insert(MeshEntityLink(Some(player_mesh)));
        //add player as a child so we won't rotate the camera when rotating the player

        println!("client id: {} {}", client_id.0, setup_info.id);
        if client_id.0 == setup_info.id {
            //if this is the local player
            commands.entity(player_mesh).insert(LocalPlayerMesh);
            commands.entity(entity).insert((
                ThirdPersonCameraTarget,
                LocalPlayer { id: setup_info.id },
                Visibility::Hidden,
            ));
            player_entity.id = setup_info.id;
        }
        println!("adding skeleton with id:{}", setup_info.id);
        let mut rotation = Quat::from_rotation_x(-FRAC_PI_2);
        rotation = rotation * Quat::from_rotation_z(PI);
        let player_mesh = spawn_nif_attached(
            commands.reborrow(),
            &asset_server,
            setup_info.id,
            Transform {
                translation: Vec3::new(0.0, -collider_height / 1.3, 0.0),
                rotation,
                scale: Vec3::splat(0.015),
            },
        );
        commands.entity(entity).add_child(player_mesh);
        commands.entity(entity).observe(actor_take_hit);
        commands.entity(entity).insert((
            Transform::from_xyz(0.0, 2.0, 0.0),
            Collider::capsule_y(collider_height / 2.0, 0.3),
            KinematicCharacterController {
                custom_mass: Some(5.0),
                up: Vec3::Y,
                offset: CharacterLength::Absolute(0.01),
                slide: true,
                autostep: Some(CharacterAutostep {
                    max_height: CharacterLength::Relative(0.3),
                    min_width: CharacterLength::Relative(0.5),
                    include_dynamic_bodies: false,
                }),
                // Don’t allow climbing slopes larger than 45 degrees.
                max_slope_climb_angle: 45.0_f32.to_radians(),
                // Automatically slide down on slopes smaller than 30 degrees.
                min_slope_slide_angle: 30_f32.to_radians(),
                apply_impulse_to_dynamic_bodies: true,
                normal_nudge_factor: 0.01,
                snap_to_ground: None,
                ..default()
            },
            Speed(4.0),
        ));
        println!(
            "player id and transform: {}{}",
            setup_info.id, setup_info.player_transform.translation
        );
        commands.entity(entity).remove::<NeedsSetup>();
    }
}
#[allow(dead_code)]
pub fn setup_assets(
    asset_paths: Vec<&str>,
    mut asset_handles: ResMut<GameAssets>,
    asset_server: Res<AssetServer>,
) {
    for asset_path in asset_paths {
        let gltf_scene = asset_server.load(asset_path);
        asset_handles
            .gltf_files
            .insert(asset_path.to_string(), gltf_scene);
    }
}
pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut gizmo_config_store: ResMut<GizmoConfigStore>, //mesh_query: Query<Entity, With<LocalPlayerMesh>>,
) {
    let (config, _) = gizmo_config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.enabled = true;
    spawn_nif_unattached(commands.reborrow(), &asset_server);

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
        PlayerCamera,
        ThirdPersonCamera::default(),
        Transform::from_xyz(0.7, 0.7, 1.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        // MSAA needs to be off for Deferred rendering
        Msaa::Off,
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 5.0,
                end: 20.0,
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 2000.0,
            ..default()
        },
        DepthPrepass,
        MotionVectorPrepass,
        DeferredPrepass,
        Fxaa::default(),
    ));
    commands.spawn((TestingComponent, Transform::default()));
    let font = asset_server.load("fonts/FiraMono-Medium.ttf");
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
            illuminance: 15_000.,
            shadows_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            maximum_distance: 10.0,
            ..default()
        }
        .build(),
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4)),
    ));
    // FlightHelmet
    let helmet_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf"));

    commands.spawn(SceneRoot(helmet_scene.clone()));
    commands.spawn((
        SceneRoot(helmet_scene),
        Transform::from_xyz(-4.0, 0.0, -3.0),
    ));
    let woodelf_scene =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/woodelf.glb"));
    commands.spawn((
        SceneRoot(woodelf_scene),
        Transform::from_xyz(-2.0, 0.0, -3.0),
    ));
    let darkelf_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/m2/dark_elf/B_N_Dark_Elf_M_Hair_01.glb"));
    commands.spawn((
        SceneRoot(darkelf_scene),
        Transform::from_xyz(-3.0, 1.8, -3.0),
    ));
    let darkelf_scene = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/m2/dark_elf/B_N_Dark_Elf_M_Head_01.glb"));
    commands.spawn((
        SceneRoot(darkelf_scene),
        Transform::from_xyz(-3.0, 1.8, -3.0),
    ));

    let mut forward_mat: StandardMaterial = Color::srgb(0.1, 0.2, 0.1).into();
    forward_mat.opaque_render_method = OpaqueRendererMethod::Forward;
    let forward_mat_h = materials.add(forward_mat);

    // Plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(forward_mat_h.clone()),
        Collider::cuboid(50.0 / 2.0, 0.0 / 2.0, 50.0 / 2.0),
    ));
    let cube_h = meshes.add(Cuboid::new(0.1, 0.1, 0.1));
    let cube_h2 = meshes.add(Cuboid::new(2.0, 2.0, 2.0));
    commands.spawn((
        Mesh3d(cube_h2.clone()),
        MeshMaterial3d(forward_mat_h.clone()),
        Collider::cuboid(2.0 / 2.0, 2.0 / 2.0, 2.0 / 2.0),
        Transform {
            rotation: Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4),
            translation: Vec3::new(2.0, 0.0, 2.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        },
    ));
    commands.spawn((
        Mesh3d(cube_h2.clone()),
        MeshMaterial3d(forward_mat_h.clone()),
        Collider::cuboid(2.0 / 2.0, 2.0 / 2.0, 2.0 / 2.0),
        Transform {
            rotation: Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_8),
            translation: Vec3::new(2.0, 0.0, 2.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        },
    ));
    commands.spawn((
        Mesh3d(cube_h2.clone()),
        MeshMaterial3d(forward_mat_h.clone()),
        Collider::cuboid(2.0 / 2.0, 2.0 / 2.0, 2.0 / 2.0),
        Transform {
            rotation: Quat::from_euler(EulerRot::ZYX, 0.0, 0.0, -FRAC_PI_4 - FRAC_PI_8),
            translation: Vec3::new(4.0, 0.0, 4.0),
            scale: Vec3::new(1.0, 1.0, 1.0),
        },
    ));
    let sphere_h = meshes.add(Sphere::new(0.125).mesh().uv(32, 18));

    // Cubes
    commands.spawn((
        Mesh3d(cube_h.clone()),
        MeshMaterial3d(forward_mat_h.clone()),
        Transform::from_xyz(-0.3, 0.5, -0.2),
    ));
    commands.spawn((
        Mesh3d(cube_h),
        MeshMaterial3d(forward_mat_h),
        Transform::from_xyz(0.2, 0.5, 0.2),
    ));

    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    let sphere_pos = Transform::from_xyz(0.4, 0.5, -0.8);
    // Emissive sphere
    let mut unlit_mat: StandardMaterial = sphere_color.into();
    unlit_mat.unlit = true;
    commands.spawn((
        Mesh3d(sphere_h.clone()),
        MeshMaterial3d(materials.add(unlit_mat)),
        sphere_pos,
        NotShadowCaster,
    ));
    // Light
    commands.spawn((
        PointLight {
            intensity: 800.0,
            radius: 0.125,
            shadows_enabled: true,
            color: sphere_color,
            ..default()
        },
        sphere_pos,
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
                j as f32 * 0.25 + if i < 3 { -0.15 } else { 0.15 } - 0.4,
                0.125,
                -j as f32 * 0.25 + if i < 3 { -0.15 } else { 0.15 } + 0.4,
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
        TextLayout::new_with_justify(JustifyText::Left),
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
#[derive(Resource, Default)]
enum DefaultRenderMode {
    #[default]
    Deferred,
}
/*
pub fn setup_scene_once_loaded(
    mut commands: Commands,
    mesh_query: Query<(&PlayerEntityLink, &Animator, &AnimationEntityLink)>,
    graphs: Res<Assets<AnimationGraph>>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    mut clips: ResMut<Assets<AnimationClip>>,
) {
    fn get_clip<'a>(
        node: AnimationNodeIndex,
        graph: &AnimationGraph,
        clips: &'a mut Assets<AnimationClip>,
    ) -> &'a mut AnimationClip {
        let node = graph.get(node).unwrap();
        let clip = match &node.node_type {
            AnimationNodeType::Clip(handle) => clips.get_mut(handle),
            _ => unreachable!(),
        };
        clip.unwrap()
    }

    for (entity, mut player) in &mut animation_players {
        for (player_entity_link, animator, animation_entity_link) in mesh_query.iter() {
            let Some(animation_entity) = animation_entity_link.0.get("models/main_skeleton.glb")
            else {
                return;
            };
            if *animation_entity == entity {
                println!("animation player added on entity: {}", entity);

                if animator.animations.len() == 0 {
                    return;
                }
                for node_index in 0..animator.animations.len() {
                    /if node_index != AnimationState::Swinging as usize {
                            player.play(animator.animations[node_index]).repeat();
                        }
                    }

                    let graph = graphs.get(&animator.graph).unwrap();
                    let swinging_animation = get_clip(
                        animator.animations[AnimationState::Swinging as usize],
                        graph,
                        &mut clips,
                    );
                    //      swinging_animation.add_event(
                    //        0.0,
                    //      OnSwingFinished {
                    //        entity: player_entity_link.0.unwrap(),
                    //  },
                    //);

                    println!("adding animationntransitions on entity: {}", entity);
                    commands
                        .entity(entity)
                        .insert(AnimationGraphHandle(animator.graph.clone()));

            }
        }
    }
}*/
fn create_animation_nodes(
    animation_names: &[&str],
    clip_map: &HashMap<String, Handle<AnimationClip>>,
) -> Vec<(String, AnimationNodeConfig)> {
    animation_names
        .iter()
        .filter_map(|&name| {
            clip_map.get(name).map(|clip_handle| {
                let config = if name == "Swing_Weapon" {
                    // Mask out the arms for swinging weapons
                    let mask = !(1 << MaskGroup::ARMS as u64);
                    AnimationNodeConfig::Masked {
                        clip: clip_handle.clone(),
                        mask,
                        speed: 1.0,
                    }
                } else {
                    AnimationNodeConfig::Standard {
                        clip: clip_handle.clone(),
                        speed: 1.0,
                    }
                };
                (name.to_string(), config)
            })
        })
        .collect()
}
fn build_animation_graph(
    node_configs: Vec<(String, AnimationNodeConfig)>,
) -> (AnimationGraph, Vec<AnimationNodeIndex>) {
    let mut animation_graph = AnimationGraph::new();
    let blend_node = animation_graph.add_blend(0.5, animation_graph.root);
    let mut node_indices = Vec::new();

    for (_name, config) in node_configs {
        let node_index = match config {
            AnimationNodeConfig::Standard { clip, speed } => {
                animation_graph.add_clip(clip, speed, blend_node)
            }
            AnimationNodeConfig::Masked { clip, mask, speed } => {
                animation_graph.add_clip_with_mask(clip, mask, speed, blend_node)
            }
        };
        node_indices.push(node_index);
    }

    (animation_graph, node_indices)
}

pub fn detect_gltf_children(
    _trigger: Trigger<SceneInstanceReady>,
    players_without_animators: Query<Entity, (With<PlayerEntityLink>, Without<Animator>)>,
    assets_gltf: Res<Assets<Gltf>>,
    mut commands: Commands,
    asset_handles: Res<GameAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut spawn_state: ResMut<NextState<SpawnSceneState>>,
) {
    // Iterate over loaded GLTF assets
    for (handle, gltf) in assets_gltf.iter() {
        if let Some(asset_handle) = asset_handles
            .gltf_files
            .get(&"models/main_skeleton.glb".to_string())
        {
            //skip any not main_skeleton gltfs
            if asset_handle.id() != handle {
                continue;
            }
        }
        println!("found glb {:?}", handle);
        let animation_names = ["Idle", "Run", "Swing_Weapon", "Jump", "First_Person_Moving"];
        // Collect relevant animation clips
        let clips_to_process: Vec<(Box<str>, Handle<AnimationClip>)> = gltf
            .named_animations
            .iter()
            .filter_map(|(name, clip)| {
                let name_str: &str = &name;
                println!("Processing animation clip: {}", name_str);
                if animation_names.contains(&name_str) {
                    println!("Processing animation clip: {}", name);
                    Some((name.clone(), clip.clone())) // Keep the name for potential ordering
                } else {
                    None
                }
            })
            .collect();
        let clip_map: HashMap<String, Handle<AnimationClip>> = clips_to_process
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(); // ⬇️ Create animation node configs
        let node_configs = create_animation_nodes(&animation_names, &clip_map);
        // Build animation graph
        let (mut animation_graph, node_indices) = build_animation_graph(node_configs);

        //  Add target masking (arms)
        for (_, (mask_group_prefix, mask_group_suffix)) in MASK_GROUP_PATHS_ARMS.iter().enumerate()
        {
            let prefix: Vec<_> = mask_group_prefix.split('/').map(Name::new).collect();
            let suffix: Vec<_> = mask_group_suffix.split('/').map(Name::new).collect();

            for chain_length in 1..=suffix.len() {
                let animation_target_id = AnimationTargetId::from_names(
                    prefix.iter().chain(suffix[0..chain_length].iter()),
                );
                animation_graph
                    .add_target_to_mask_group(animation_target_id, MaskGroup::ARMS as u32);
            }
        }
        //  Add target masking (torso)
        for (_, (mask_group_prefix, mask_group_suffix)) in
            MASK_GROUP_PATHS_BODY_WITHOUT_ARMS.iter().enumerate()
        {
            let prefix: Vec<_> = mask_group_prefix.split('/').map(Name::new).collect();
            let suffix: Vec<_> = mask_group_suffix.split('/').map(Name::new).collect();

            for chain_length in 0..=suffix.len() {
                let animation_target_id = AnimationTargetId::from_names(
                    prefix.iter().chain(suffix[0..chain_length].iter()),
                );
                animation_graph
                    .add_target_to_mask_group(animation_target_id, MaskGroup::TORSO as u32);
            }
        }

        let graph_handle = graphs.add(animation_graph);
        if let Some(player) = players_without_animators.iter().next() {
            commands.entity(player).insert(Animator {
                animations: node_indices.clone(),
                graph: graph_handle.clone(),
            });
        }
    }

    for handle in asset_handles.gltf_files.iter() {
        println!("found glb me: {:?}", handle);
    } //   walk_tree(&all_entities_with_children, &names, &parent_entity, 0);
    spawn_state.set(SpawnSceneState::Spawned);
}
pub fn insert_nif_animator_links(
    nif_animator_q: Query<(Entity, &NifAnimator), Added<NifAnimator>>,
    player_state_q: Query<(Entity, &PlayerState)>,
    mut commands: Commands,
) {
    for (nif_animator_entity, nif_animator) in nif_animator_q {
        for (player_entity, player_state) in player_state_q {
            if nif_animator.skeleton_id == player_state.id {
                commands
                    .entity(player_entity)
                    .insert(NifAnimatorLink(nif_animator_entity));
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
pub fn switch_mode(
    mut query: Query<&mut Text, With<HudText>>, // Changed to a regular Query with a filter
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<Pause>,
) {
    let mut text = match query.single_mut() {
        Ok(t) => t,
        Err(e) => {
            match e {
                bevy::ecs::query::QuerySingleError::NoEntities(_) => {
                    eprintln!("Warning: No UI text entity found with the HudText component.");
                }
                bevy::ecs::query::QuerySingleError::MultipleEntities(_) => {
                    eprintln!(
                        "Warning: Multiple UI text entities found with the HudText component. Expected only one."
                    );
                }
            }
            return; // Return early if there's an error
        }
    };
    text.clear();

    if keys.just_pressed(KeyCode::Space) {
        pause.0 = !pause.0;
    }
    /*
    if keys.just_pressed(KeyCode::Digit1) {
        *mode = DefaultRenderMode::Deferred;
        default_opaque_renderer_method.set_to_deferred();
        println!("DefaultOpaqueRendererMethod: Deferred");
        for _ in materials.iter_mut() {}
        for camera in &cameras {
            commands.entity(camera).remove::<NormalPrepass>();
            commands.entity(camera).insert(DepthPrepass);
            commands.entity(camera).insert(MotionVectorPrepass);
            commands.entity(camera).insert(DeferredPrepass);
        }
    }
    if keys.just_pressed(KeyCode::Digit2) {
        *mode = DefaultRenderMode::Forward;
        default_opaque_renderer_method.set_to_forward();
        println!("DefaultOpaqueRendererMethod: Forward");
        for _ in materials.iter_mut() {}
        for camera in &cameras {
            commands.entity(camera).remove::<NormalPrepass>();
            commands.entity(camera).remove::<DepthPrepass>();
            commands.entity(camera).remove::<MotionVectorPrepass>();
            commands.entity(camera).remove::<DeferredPrepass>();
        }
    }
    if keys.just_pressed(KeyCode::Digit3) {
        *mode = DefaultRenderMode::ForwardPrepass;
        default_opaque_renderer_method.set_to_forward();
        println!("DefaultOpaqueRendererMethod: Forward + Prepass");
        for _ in materials.iter_mut() {}
        for camera in &cameras {
            commands.entity(camera).insert(NormalPrepass);
            commands.entity(camera).insert(DepthPrepass);
            commands.entity(camera).insert(MotionVectorPrepass);
            commands.entity(camera).remove::<DeferredPrepass>();
        }
    }

    if keys.just_pressed(KeyCode::KeyH) {
        *hide_ui = !*hide_ui;
    }

    if !*hide_ui {
        text.push_str("(H) Hide UI\n");
        text.push_str("(Space) Play/Pause\n\n");
        text.push_str("Rendering Method:\n");

        text.push_str(&format!(
            "(1) {} Deferred\n",
            if let DefaultRenderMode::Deferred = *mode {
                ">"
            } else {
                ""
            }
        ));
        text.push_str(&format!(
            "(2) {} Forward\n",
            if let DefaultRenderMode::Forward = *mode {
                ">"
            } else {
                ""
            }
        ));
        text.push_str(&format!(
            "(3) {} Forward + Prepass\n",
            if let DefaultRenderMode::ForwardPrepass = *mode {
                ">"
            } else {
                ""
            }
        ));
    }*/
}
fn spawn_nif_unattached(mut commands: Commands, asset_server: &Res<AssetServer>) {
    let head_handle = asset_server.load("data/A_Glass_Helmet.nif");
    commands.spawn((
        NifScene(head_handle),
        Visibility::Visible,
        Transform {
            translation: Vec3::new(-0.0, 0.5, -1.0),
            rotation: Quat::from_rotation_y(PI),
            scale: Vec3::splat(0.03), //nifs are huge otherwise
        },
    ));
}
fn spawn_nif_attached(
    mut commands: Commands,
    asset_server: &Res<AssetServer>,
    skeleton_id: u64,
    transform: Transform,
) -> Entity {
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
        (
            "data/base_anim.nif",
            AttachmentType::MainSkeleton {
                skeleton_id: skeleton_id,
            },
        ),
        (
            "data/b_n_dark elf_m_skins.nif",
            AttachmentType::Skinned { skeleton_id },
        ),
        (
            "data/B_N_Dark Elf_M_Head_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Hair_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Neck.nif",
            AttachmentType::Rigid {
                target_bone: "Neck".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Groin.nif",
            AttachmentType::Rigid {
                target_bone: "Groin".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Forearm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Forearm".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Upper Arm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Arm".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Wrist.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Wrist".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Upper Leg.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Leg".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Knee.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Knee".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Ankle.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Ankle".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/B_N_Dark Elf_M_Foot.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Foot".to_string(),
                skeleton_id,
            },
        ),
    ];
    let entity = commands.spawn((Visibility::Inherited, transform)).id();
    spawn_nifs(
        paths_with_target_bone,
        &asset_server,
        entity,
        &mut commands,
        Transform::from_scale(Vec3::splat(1.0)),
    );
    entity
}

fn spawn_nifs(
    paths_with_target_bone: Vec<(&str, AttachmentType)>,
    asset_server: &Res<AssetServer>,
    entity: Entity,
    commands: &mut Commands,
    transform: Transform,
) {
    for (path, mut attachment_type) in paths_with_target_bone {
        let asset_handle = asset_server.load(path);
        if let AttachmentType::DoubleSidedRigid {
            target_bone,
            skeleton_id,
        } = attachment_type.clone()
        {
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Left {}", target_bone),
                skeleton_id,
            };
            let child = commands
                .spawn((
                    NifScene(asset_handle.clone()),
                    attachment_type,
                    transform,
                    Visibility::Inherited,
                ))
                .id();
            commands.entity(entity).add_child(child);
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Right {}", target_bone),
                skeleton_id,
            };
        }
        let child = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type,
                transform,
                Visibility::Inherited,
            ))
            .id();
        commands.entity(entity).add_child(child);
    }
}
pub fn add_nif_animation_events(
    mut commands: Commands,
    mut clips: ResMut<Assets<AnimationClip>>,
    animator_q: Query<(Entity, &NifAnimator, &AnimationGraphHandle), Without<KnockdownEventAdded>>,
    anim_graphs: Res<Assets<AnimationGraph>>,
    player_query: Query<(Entity, &NifAnimatorLink), Added<NifAnimatorLink>>,
) {
    fn get_clip<'a>(
        node: AnimationNodeIndex,
        graph: &AnimationGraph,
        clips: &'a mut Assets<AnimationClip>,
    ) -> &'a mut AnimationClip {
        let node = graph.get(node).unwrap();
        let clip = match &node.node_type {
            AnimationNodeType::Clip(handle) => clips.get_mut(handle),
            _ => unreachable!(),
        };
        clip.unwrap()
    }

    for (entity, nif_animator, graph_handle) in animator_q {
        for (player_entity, nif_animator_link) in player_query {
            println!("nif: {} player: {}", entity, nif_animator_link.0);
            if nif_animator_link.0 != entity {
                continue;
            }
            let anim_graph = anim_graphs.get(graph_handle).unwrap();
            let knockdown_anim = nif_animator.animations.get("knockdown").unwrap();
            let knockdown_clip = get_clip(*knockdown_anim, anim_graph, &mut clips);
            let knockdown_clip_duration = knockdown_clip.duration();
            knockdown_clip.add_event(knockdown_clip_duration, KnockdownFinished(player_entity));
            commands.entity(entity).insert(KnockdownEventAdded);
        }
    }
}
