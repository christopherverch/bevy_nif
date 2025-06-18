use bevy::render::mesh::Mesh;
use bevy::{
    core_pipeline::{
        fxaa::Fxaa,
        prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass},
    },
    pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver, OpaqueRendererMethod},
    prelude::*,
};
use bevy_kira_audio::SpatialAudioReceiver;
use bevy_nif::nif::attach_parts::AttachmentType;
use bevy_nif::nif::spawner::NifScene;
use bevy_nif::nif_animation::{NifAnimator, NifAnimatorAdded};
use bevy_rapier3d::prelude::*;
use cdda_bevy::client::defs::{
    AnimateableEntity, CreatureStats, CurrentClientId, KnockdownEventAdded, LocalPlayer,
    LocalPlayerMesh, NeedsSetup, NetworkedEntity, NifAnimatorLink, Player, PlayerCamera,
    PlayerState, RootEntityLink, TestingComponent,
};
use cdda_bevy::client::events::KnockdownFinished;
use std::f32::consts::*;

use crate::camera::{MWCamera, MWCameraTarget};
use crate::const_defs::PLAYER_COLLIDER_HEIGHT;
use cdda_bevy::client::actor_defs::{
    ActorState, CharacterMovement, CharacterState, CombatState, JumpingState, MovementState,
    RootMotionState, VitalsState,
};

#[derive(Component)]
pub struct HudText;

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut needs_setup_query: Query<(Entity, &NeedsSetup), Added<NeedsSetup>>,
    client_id: Res<CurrentClientId>,
    mut local_player_res: ResMut<LocalPlayer>,
) {
    for (player_entity, setup_info) in &mut needs_setup_query {
        info!(
            "setting up player with client id: {} {}",
            client_id.0, setup_info.id
        );
        //spawn the initial player
        commands.entity(player_entity).insert((
            Player,
            Name::new(format!("Player {}", setup_info.id)),
            PlayerState {
                id: setup_info.id,
                ..Default::default()
            },
            CreatureStats {
                id: setup_info.id,
                health: 50.0,
                nif_animator_link: None,
            },
            ActorState {
                movement: MovementState::default(),
                combat: CombatState::default(),
                jumping: JumpingState::default(),
                vitals: VitalsState::default(),
                idle_state: CharacterState::default(),
                current_idle_animation: "idle1".to_string(),
                skip_anim: false,
                is_running: true,
                is_sneaking: false,
                is_in_water: false,
            },
            CharacterMovement::default(),
            AnimateableEntity,
            NetworkedEntity(player_entity),
            Transform::from_xyz(0.0, 2.0, 0.0),
            Visibility::Visible,
        ));

        //spawn the player's mesh
        let player_mesh = spawn_nif_attached(
            commands.reborrow(),
            &asset_server,
            setup_info.id,
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                rotation: Quat::default(),
                scale: Vec3::splat(0.02),
            },
        );
        commands.entity(player_mesh).insert((
            // for tracking animation times with the animation system
            RootMotionState::default(),
            RootEntityLink(Some(player_entity)),
            Name::new("player mesh"),
        ));

        //if this is the local player
        if client_id.0 == setup_info.id {
            commands.entity(player_mesh).insert(LocalPlayerMesh);
            commands.entity(player_entity).insert((
                MWCameraTarget,
                LocalPlayer { id: setup_info.id },
                SpatialAudioReceiver,
                //default to first person, so hide the player
                Visibility::Hidden,
            ));
            local_player_res.id = setup_info.id;
        }
        commands.entity(player_entity).add_child(player_mesh);

        // add physics to player
        commands.entity(player_entity).insert((
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
        ));

        commands.entity(player_entity).remove::<NeedsSetup>();
    }
}
pub fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
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
        MWCamera::default(),
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
pub fn spawn_player_offline(mut commands: Commands) {
    info!("spawning player offline");
    commands.spawn(NeedsSetup {
        id: 0,
        player_transform: Transform::from_xyz(0.0, 0.0, 0.0),
    });

    //let player_transform_server =
    //  Transform::from_xyz(translation[0], translation[1], translation[2]);
}
pub fn insert_nif_animator_links(
    trigger: Trigger<NifAnimatorAdded>,
    nif_animator_q: Query<(Entity, &mut NifAnimator)>,
    mut creature_q: Query<&mut CreatureStats>,
) {
    let Ok((nif_animator_entity, nif_animator)) = nif_animator_q.get(trigger.0) else {
        return;
    };
    for mut creature_stats in creature_q.iter_mut() {
        if nif_animator.skeleton_id == creature_stats.id {
            creature_stats.nif_animator_link = Some(nif_animator_entity);
        }
    }
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
            info!(
                "adding knockdown event to nif entity: {} with animator link: {}",
                entity, nif_animator_link.0
            );
            if nif_animator_link.0 != entity {
                continue;
            }
            let Some(anim_graph) = anim_graphs.get(graph_handle) else {
                warn!("missing animation graph for nif animator!");
                commands.entity(entity).insert(KnockdownEventAdded);
                return;
            };
            let Some(knockdown_anim) = nif_animator.animation_definitions.get("knockdown") else {
                warn!("missing knockdown anim for nif animator!");
                commands.entity(entity).insert(KnockdownEventAdded);
                return;
            };
            let knockdown_clip = get_clip(knockdown_anim.node_index, anim_graph, &mut clips);
            let knockdown_clip_duration = knockdown_clip.duration();
            knockdown_clip.add_event(knockdown_clip_duration, KnockdownFinished(player_entity));
            commands.entity(entity).insert(KnockdownEventAdded);
        }
    }
}
