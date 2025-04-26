use cdda_bevy::client::defs::{
    AnimateableEntity, AnimationNodeConfig, Animations, Animator, CurrentClientId, LocalPlayer,
    MaskGroup, NeedsSetup, NetworkedEntity, Pause, Player, PlayerCamera, PlayerRoot, PlayerState,
    SceneEntitiesByName, SceneName, SpawnSceneState, Speed, Weapon,
};
use cdda_bevy::client::third_person_camera::{ThirdPersonCamera, ThirdPersonCameraTarget};

use bevy::gltf::Gltf;
use bevy::render::mesh::Mesh;
use bevy::{
    animation::AnimationTargetId,
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
        //prepass::NormalPrepass
    },
    image::ImageLoaderSettings,
    math::ops,
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver, OpaqueRendererMethod},
    prelude::*,
    scene::SceneInstanceReady,
};
use bevy_rapier3d::prelude::*;
use cdda_bevy::AnimationState;
use cdda_bevy::client::modular_characters::*;
use std::collections::HashMap;
use std::f32::consts::*;
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
        "CharacterArmature.001/Root/Body/Hips/Abdomen/Torso/Chest",
        "Shoulder.R/UpperArm.R/LowerArm.R/Wrist.R",
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
const PLAYER_ASSETS: [&str; 5] = [
    "models/main_skeleton.glb",
    "models/casual_torso.glb",
    "models/casual_legs.glb",
    "models/casual_head.glb",
    "models/casual_feet.glb",
];
pub fn spawn_gltf_as_child(
    gltfs: &Res<Assets<Gltf>>,
    asset_handles: &Res<GameAssets>,
    scene_entities_by_name: &mut ResMut<SceneEntitiesByName>,
    entity: Entity,
    commands: &mut Commands,
    client_id: u64,
    transform: Transform,
    asset_names: Vec<&str>,
) {
    for asset_name in asset_names {
        println!("trying to spawn asset: {}", asset_name);
        if let Some(handle) = asset_handles.gltf_files.get(asset_name) {
            if let Some(gltf) = gltfs.get(handle) {
                let scene = gltf.default_scene.clone().unwrap();
                println!("player id: {}", entity);

                commands.entity(entity).with_children(|parent| {
                    let player_skeleton_entity = parent
                        .spawn((
                            SceneRoot(scene.clone()),
                            transform,
                            SceneName {
                                scene_name: asset_name.to_string(),
                                id: client_id,
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &NeedsSetup)>,
    client_id: Res<CurrentClientId>,
    gltfs: Res<Assets<Gltf>>,
    asset_handles: Res<GameAssets>,
    mut scene_entities_by_name: ResMut<SceneEntitiesByName>,
    mut player_state_local: ResMut<PlayerState>,
    mut player_entity: ResMut<LocalPlayer>,
) {
    let collider_height = 1.2;
    for (entity, setup_info) in &mut query {
        println!("setting up");
        commands.entity(entity).insert((
            PlayerRoot,
            Transform::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
        ));
        let player = commands.spawn((Player, AnimateableEntity)).id();
        spawn_gltf_as_child(
            &gltfs,
            &asset_handles,
            &mut scene_entities_by_name,
            player,
            &mut commands,
            setup_info.id,
            Transform::from_xyz(0.0, -collider_height / 1.3, 0.0), //shift down half height of collider
            PLAYER_ASSETS.to_vec(),
        );
        commands.entity(entity).add_child(player);
        //add player as a child so we won't rotate the camera when rotating the player

        commands.entity(player).insert((
            PlayerState {
                id: setup_info.id,
                ..Default::default()
            },
            InheritedVisibility::VISIBLE,
            NetworkedEntity(entity),
        ));
        println!("client id: {} {}", client_id.0, setup_info.id);
        if client_id.0 == setup_info.id {
            //if this is the local player
            commands.entity(player).insert(ThirdPersonCameraTarget);
            commands
                .entity(player)
                .insert(LocalPlayer { 0: Some(player) });
            player_entity.0 = Some(player);
            player_state_local.id = setup_info.id;
        }

        // The normal map. Note that to generate it in the GIMP image editor, you should
        // open the depth map, and do Filters → Generic → Normal Map
        // You should enable the "flip X" checkbox.
        let normal_handle = asset_server.load_with_settings(
            "textures/parallax_example/cube_normal.png",
            // The normal map texture is in linear color space. Lighting won't look correct
            // if `is_srgb` is `true`, which is the default.
            |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
        );

        let mut cube = Mesh::from(Cuboid::new(0.15, 0.15, 0.15));

        // NOTE: for normal maps and depth maps to work, the mesh
        // needs tangents generated.
        cube.generate_tangents().unwrap();

        let parallax_material = materials.add(StandardMaterial {
            perceptual_roughness: 0.4,
            base_color_texture: Some(asset_server.load("textures/parallax_example/cube_color.png")),
            normal_map_texture: Some(normal_handle),
            // The depth map is a grayscale texture where black is the highest level and
            // white the lowest.
            depth_map: Some(asset_server.load("textures/parallax_example/cube_depth.png")),
            parallax_depth_scale: 0.09,
            parallax_mapping_method: ParallaxMappingMethod::Relief { max_steps: 4 },
            max_parallax_layer_count: ops::exp2(5.0f32),
            ..default()
        });
        commands.entity(player).insert((
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
        let cube_weapon = Mesh::from(Cuboid::new(0.04, 0.2, 0.04));
        commands.entity(player).with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(cube_weapon)),
                Transform::from_xyz(0.1, 0.0, -0.2),
                MeshMaterial3d(parallax_material),
                TransformInterpolation::default(),
                Weapon,
            ));
        });
        println!(
            "player id and transform: {}{}",
            setup_info.id, setup_info.player_transform.translation
        );
        commands.entity(entity).remove::<NeedsSetup>();
    }
}
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
    asset_handles: ResMut<GameAssets>,
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
    setup_assets(PLAYER_ASSETS.to_vec(), asset_handles, asset_server);
}
#[derive(Resource, Default)]
enum DefaultRenderMode {
    #[default]
    Deferred,
}
pub fn setup_scene_once_loaded(
    mut commands: Commands,
    player_query: Query<(&Animator, &AnimationEntityLink)>,
    mut animation_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (entity, mut player) in &mut animation_players {
        for (animator, animation_entity_link) in player_query.iter() {
            if animation_entity_link.0 == entity {
                println!("animation player added on entity: {}", entity);

                if animator.animations.len() == 0 {
                    return;
                }
                for node_index in 0..animator.animations.len() {
                    player.play(animator.animations[node_index]).repeat();
                }

                println!("adding animationntransitions on entity: {}", entity);
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(animator.graph.clone()));
            }
        }
    }
}
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
    trigger: Trigger<SceneInstanceReady>,
    all_entities_with_children: Query<&Children>,
    players_without_animators: Query<Entity, (With<Player>, Without<Animator>)>,
    assets_gltf: Res<Assets<Gltf>>,
    mesh_query: Query<&Mesh3d>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    global_transforms: Query<&GlobalTransform>,
    mut commands: Commands,
    asset_handles: Res<GameAssets>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut spawn_state: ResMut<NextState<SpawnSceneState>>,
) {
    info!(
        "SceneInstanceReady event triggered for entity: {:?}",
        trigger.entity()
    );
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
        let animation_names = ["Idle", "Run", "Swing_Weapon", "Jump"];
        // Collect relevant animation clips
        let clips_to_process: Vec<(Box<str>, Handle<AnimationClip>)> = gltf
            .named_animations
            .iter()
            .filter_map(|(name, clip)| {
                let name_str: &str = &name;
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
                println!(
                    "animation target id name: {:?}",
                    prefix.iter().chain(suffix[0..chain_length].iter())
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
    let parent_entity = trigger.entity();
    paint_cubes_on_joints(
        &all_entities_with_children,
        //   &names,
        &parent_entity,
        &mesh_query,
        meshes,
        materials,
        &global_transforms,
        commands,
    );
    spawn_state.set(SpawnSceneState::Spawned);
}
#[allow(clippy::too_many_arguments)]
pub fn switch_mode(
    mut query: Query<&mut Text, With<HudText>>, // Changed to a regular Query with a filter
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<Pause>,
) {
    let mut text = match query.get_single_mut() {
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
