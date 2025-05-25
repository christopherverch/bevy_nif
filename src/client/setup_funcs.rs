use bevy::render::mesh::Mesh;
use bevy::{
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
        //prepass::NormalPrepass
    },
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver, OpaqueRendererMethod},
    prelude::*,
};
use bevy_kira_audio::SpatialAudioReceiver;
use bevy_nif;
use bevy_nif::nif::animation::{NifAnimator, NifAnimatorAdded};
use bevy_nif::nif::attach_parts::AttachmentType;
use bevy_nif::nif::spawner::NifScene;
use bevy_rapier3d::prelude::*;
use cdda_bevy::client::defs::{
    AnimateableEntity, CreatureStats, CurrentClientId, KnockdownEventAdded, LandscapeLoader,
    LocalPlayer, LocalPlayerMesh, MeshEntityLink, NeedsSetup, NetworkedEntity, NifAnimatorLink,
    Pause, Player, PlayerCamera, PlayerEntityLink, PlayerState, Speed, TestingComponent,
};
use cdda_bevy::client::events::KnockdownFinished;
use cdda_bevy::client::third_person_camera::{ThirdPersonCamera, ThirdPersonCameraTarget};
use std::f32::consts::*;

const PLAYER_COLLIDER_HEIGHT: f32 = 1.2;
#[derive(Component)]
pub struct HudText;

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(Entity, &NeedsSetup), Added<NeedsSetup>>,
    client_id: Res<CurrentClientId>,
    mut local_player_res: ResMut<LocalPlayer>,
) {
    for (player_entity, setup_info) in &mut query {
        commands.spawn((
            LandscapeLoader {
                cell_x: -2,
                cell_y: -9,
            },
            Visibility::Visible,
        ));
        commands.spawn((
            LandscapeLoader {
                cell_x: -2,
                cell_y: -9,
            },
            Visibility::Visible,
        ));

        println!("setting up player");
        println!("client id: {} {}", client_id.0, setup_info.id);

        //spawn the initial player
        commands.entity(player_entity).insert((
            Player,
            Name::new(format!("Player {}", setup_info.id)),
            PlayerState {
                id: setup_info.id,
                ..Default::default()
            },
            CreatureStats::new(setup_info.id),
            AnimateableEntity,
            NetworkedEntity(player_entity),
            Transform::default(),
            Visibility::Visible,
        ));

        //spawn the player's mesh
        let player_mesh = commands
            .spawn((
                PlayerEntityLink(Some(player_entity)),
                Visibility::Visible,
                Transform::from_xyz(0.0, -PLAYER_COLLIDER_HEIGHT / 1.3, 0.0),
            ))
            .id();
        commands
            .entity(player_entity)
            .insert(MeshEntityLink(Some(player_mesh)));

        //if this is the local player
        if client_id.0 == setup_info.id {
            commands.entity(player_mesh).insert(LocalPlayerMesh);
            commands.entity(player_entity).insert((
                ThirdPersonCameraTarget,
                LocalPlayer { id: setup_info.id },
                SpatialAudioReceiver,
                //default to first person, so hide the player
                Visibility::Hidden,
            ));
            local_player_res.id = setup_info.id;
        }
        //rotate because of nif to bevy rotation weirdness
        let mut rotation = Quat::from_rotation_x(-FRAC_PI_2);
        rotation = rotation * Quat::from_rotation_z(PI);
        let player_mesh = spawn_nif_attached(
            commands.reborrow(),
            &asset_server,
            setup_info.id,
            Transform {
                translation: Vec3::new(0.0, -PLAYER_COLLIDER_HEIGHT / 1.3, 0.0),
                rotation,
                scale: Vec3::splat(0.015),
            },
        );

        commands.entity(player_entity).add_child(player_mesh);
        commands.entity(player_entity).insert((
            Transform::from_xyz(0.0, 2.0, 0.0),
            Collider::capsule_y(PLAYER_COLLIDER_HEIGHT / 2.0, 0.3),
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
        commands.entity(player_entity).remove::<NeedsSetup>();
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

    let sphere_color = Color::srgb(10.0, 4.0, 1.0);
    let sphere_pos = Transform::from_xyz(0.4, 0.5, -0.8);
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

pub fn insert_nif_animator_links(
    trigger: Trigger<NifAnimatorAdded>,
    nif_animator_q: Query<(Entity, &NifAnimator)>,
    creature_q: Query<(Entity, &CreatureStats)>,
    mut commands: Commands,
) {
    let Ok((nif_animator_entity, nif_animator)) = nif_animator_q.get(trigger.0) else {
        return;
    };
    for (creature_entity, creature_stats) in creature_q {
        if nif_animator.skeleton_id == creature_stats.id {
            commands
                .entity(creature_entity)
                .insert(NifAnimatorLink(nif_animator_entity));
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
