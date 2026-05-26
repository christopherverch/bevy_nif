use crate::defs::{
    AnimateableEntity, ClientEntityId, CreatureStats, CreatureStatsFlags, CurrentCell, DrawState,
    LocalPlayer, LocalPlayerMesh, MeshEntityLink, Movement, NeedsSetup, NetworkedEntity, Player,
    RootEntityLink,
};
use crate::esm::EsmStore;
use crate::gui::setup_race_select;
use crate::live_cell_ref::{EsmPtr, LiveCellRef};
use crate::magic_effects::MagicEffects;
use crate::mechanics::ActorIntent;
use crate::stat::DynamicStat;
use avian3d::prelude::{CollisionLayers, RigidBody};
use bevy::prelude::*;
use bevy_kira_audio::{SpatialAudioEmitter, SpatialAudioReceiver, SpatialRadius};
use bevy_nif::attach_parts::AttachmentType;
use bevy_nif::nif_animation::{NifAnimator, NifAnimatorAdded};
use bevy_nif::spawner::NifScene;
use common::mechanics::{CharacterController, CharacterState};
use esm_parser_lib::RefId;
use mwphysics::{ActorPhysics, PositionInterpolation};
use serializer::SerializableTransform;
use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Instant;

use crate::camera::MWCameraTarget;
use crate::const_defs::NIF_SCALE_MULTIPLIER;
use crate::physics::{AnimationDisplacement, CreatureControllerLink, RootMotionState};

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut needs_setup_query: Query<(Entity, &NeedsSetup), Added<NeedsSetup>>,
    esm_store: Res<EsmStore>,
    client_id: Res<ClientEntityId>,
) {
    for (player_entity, setup_info) in &mut needs_setup_query {
        info!(
            "setting up player with client id: {} {}",
            client_id.0, setup_info.entity_id
        );

        // spawn the root of the player
        let player_start_location = Vec3::new(190.0, 2.8, -890.0);
        let rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -PI, -PI);
        let can_water_walk = true;
        let on_ground = true;
        let collision_shape_type = mwphysics::CollisionShapeType::RotatingBox;
        let rotationally_invariant = true;
        let collision_shape_center = Vec3::ZERO;
        let collision_shape_extents = Vec3::new(1.0, 5.0, 1.0);
        let mut character_controller = CharacterController::default();
        character_controller.vitals.current_idle_group = "idle1".to_string();
        character_controller.movement.state = CharacterState::RunForward;
        // Sets the character's animations to be based on their movement speed, like walking running
        character_controller.movement.adjust_anim_speed = true;
        let player_esm_ref = esm_store
            .npcs
            .get_raw(&RefId::from_str("Player"))
            .expect("Missing player data in esm!");
        commands
            .entity(player_entity)
            .insert((
                Player { jumping: false },
                Name::new(format!("Player {}", setup_info.entity_id)),
                PositionInterpolation::new(Vec3::ZERO, player_start_location),
                Transform {
                    translation: player_start_location,
                    rotation,
                    ..default()
                },
                CreatureStats {
                    entity_id: setup_info.entity_id,
                    // Filled in later by insert_nif_animator_links
                    nif_animator_link: None,
                    magic_effects: MagicEffects::default(),
                    fall_height: 0.0,
                    dead: false,
                    teleported: false,
                    dynamic_stats: [
                        DynamicStat::default(),
                        DynamicStat::default(),
                        DynamicStat::default(),
                    ],
                    death_animation_finished: false,
                    movement_flags: CreatureStatsFlags::RUN,
                    max_speed: 2.0,
                    side_movement_angle: 0.0,
                    draw_state: DrawState::Nothing,
                },
                Movement::default(),
                ActorPhysics::new(
                    can_water_walk,
                    on_ground,
                    collision_shape_type,
                    rotationally_invariant,
                    collision_shape_center,
                    collision_shape_extents,
                ),
                CollisionLayers::default(),
                character_controller,
                NetworkedEntity {
                    entity_id: setup_info.entity_id,
                    entity: player_entity,
                    serializable_transform: SerializableTransform {
                        translation: player_start_location,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::splat(1.0),
                    },
                    last_packet_time: Instant::now(),
                },
                // Animation / physics
                AnimationDisplacement::default(),
                AnimateableEntity,
                RigidBody::Kinematic,
                // Audio
                SpatialAudioEmitter::default(),
                SpatialRadius { radius: 12.0 },
            ))
            .insert((
                ActorIntent::default(),
                CurrentCell {
                    coords: UVec2::ZERO,
                },
                LiveCellRef::Npc(EsmPtr(player_esm_ref)),
            ));

        //spawn the player's mesh
        let player_mesh = spawn_player_mesh(
            player_entity,
            commands.reborrow(),
            &asset_server,
            setup_info.entity_id,
            Transform::from_scale(Vec3::splat(NIF_SCALE_MULTIPLIER)),
        );

        commands.entity(player_mesh).insert((
            // for tracking animation times with the animation system
            RootMotionState::default(),
            RootEntityLink(Some(player_entity)),
            Name::new("player mesh"),
        ));
        // Add a link to the mesh entity so we can reference it from the player
        commands
            .entity(player_entity)
            .insert(MeshEntityLink(player_mesh));

        //if this is the local player, add camera and marker component
        if client_id.0 == setup_info.entity_id {
            commands.entity(player_mesh).insert((
                //Default to first person, so hide the player's mesh
                Visibility::Hidden,
                LocalPlayerMesh,
            ));
            commands.entity(player_entity).insert((
                MWCameraTarget,
                LocalPlayer {
                    entity_id: setup_info.entity_id,
                },
                SpatialAudioReceiver,
            ));
            // Set up the race select UI

            // TODO:: put this somewhere better
            commands.run_system_cached(setup_race_select);
        }
        commands.entity(player_entity).add_child(player_mesh);
        commands.entity(player_entity).remove::<NeedsSetup>();
    }
}

pub fn spawn_player_offline(mut commands: Commands) {
    info!("spawning player offline");
    commands.spawn(NeedsSetup {
        entity_id: 0,
        player_transform: Transform::from_xyz(0.0, 0.0, 0.0),
    });
}
pub fn insert_nif_animator_links(
    event: On<NifAnimatorAdded>,
    nif_animator_q: Query<(Entity, &mut NifAnimator)>,
    mut creature_q: Query<&mut CreatureStats>,
) {
    let Ok((nif_animator_entity, nif_animator)) = nif_animator_q.get(event.0) else {
        return;
    };
    for mut creature_stats in creature_q.iter_mut() {
        if nif_animator.skeleton_id == creature_stats.entity_id {
            creature_stats.nif_animator_link = Some(nif_animator_entity);
        }
    }
}

fn spawn_player_mesh(
    player_entity: Entity,
    mut commands: Commands,
    asset_server: &Res<AssetServer>,
    skeleton_id: u64,
    transform: Transform,
) -> Entity {
    let asset_handle = asset_server.load("data/base_anim.nif");
    let skeleton_entity = commands
        .spawn((
            NifScene(asset_handle),
            AttachmentType::MainSkeleton { skeleton_id },
            CreatureControllerLink(player_entity),
            transform,
            Visibility::Inherited,
        ))
        .id();
    commands.entity(player_entity).add_child(skeleton_entity);
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Skins.nif",
            AttachmentType::Skinned { skeleton_id },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Dark Elf_M_Head_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Dark Elf_M_Hair_01.nif",
            AttachmentType::Rigid {
                target_bone: "Head".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Neck.nif",
            AttachmentType::Rigid {
                target_bone: "Neck".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Groin.nif",
            AttachmentType::Rigid {
                target_bone: "Groin".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Forearm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Forearm".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Upper Arm.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Arm".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Wrist.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Wrist".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Upper Leg.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Upper Leg".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Knee.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Knee".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Ankle.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Ankle".to_string(),
                skeleton_id,
            },
        ),
        (
            "data/Data Files/Meshes/b/B_N_Orc_M_Foot.nif",
            AttachmentType::DoubleSidedRigid {
                target_bone: "Foot".to_string(),
                skeleton_id,
            },
        ),
    ];
    spawn_nifs(
        paths_with_target_bone,
        &asset_server,
        skeleton_entity,
        &mut commands,
        Transform::from_scale(Vec3::splat(1.0)),
    );
    skeleton_entity
}

fn spawn_nifs(
    paths_with_target_bone: Vec<(&str, AttachmentType)>,
    asset_server: &Res<AssetServer>,
    skeleton_entity: Entity,
    commands: &mut Commands,
    transform: Transform,
) {
    for (path, mut attachment_type) in paths_with_target_bone {
        let asset_handle = asset_server.load(path.to_string());
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
            commands.entity(skeleton_entity).add_child(child);
            attachment_type = AttachmentType::Rigid {
                target_bone: format!("Right {}", target_bone),
                skeleton_id,
            };
        }
        let child = commands
            .spawn((
                NifScene(asset_handle),
                attachment_type.clone(),
                transform,
                Visibility::Inherited,
            ))
            .id();
        if matches!(attachment_type, AttachmentType::Skinned { skeleton_id: _ }) {
            // Needs to rotate skinned attachments since these aren't attached to the pre-rotated
            // skeleton, they're just a child of the mesh
            commands.entity(child).insert(Transform::from_rotation(
                Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(PI),
            ));
        }
        commands.entity(skeleton_entity).add_child(child);
    }
}
/*
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
}*/
