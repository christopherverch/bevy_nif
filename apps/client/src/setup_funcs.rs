use crate::animation::{NifAnimatorLink, NifSkeletonId};
use crate::const_defs::CELL_SIZE_METERS;
use crate::defs::{
    AnimateableEntity, ClientNetworkId, CurrentCell, LocalPlayer, LocalPlayerMesh, MeshEntityLink,
    Movement, NeedsSetup, NetworkedEntity, Player,
};
use crate::esm_ptr::EsmPtr;
use crate::live_cell_ref::{CellRefBase, LiveCellRef};
use crate::mechanics::ActorIntent;
use crate::mechanics::creature_stats::CreatureStats;
use crate::mechanics::npc_stats::NpcStats;
use crate::physics::actor::ActorPhysics;
use crate::physics::collision::CollisionShapeType;
use crate::physics::position_interpolate::PositionInterpolation;
use crate::setup::setup_npc::NpcMesh;
use crate::shared_defs::container::Container;
use crate::shared_defs::skeletons::NextAvailableSkeletonId;
use avian3d::prelude::{CollisionLayers, RigidBody};
use bevy::prelude::*;
use bevy_kira_audio::{SpatialAudioEmitter, SpatialAudioReceiver, SpatialRadius};
use bevy_nif::NIF_TO_BEVY_ROTATION;
use bevy_nif::attach_parts::AttachmentType;
use bevy_nif::nif_animation::{NifAnimator, NifAnimatorAdded};
use bevy_nif::spawner::NifScene;
use common::mechanics::{CharacterController, CharacterState};
use common::stat::{AttributeValue, SkillValue};
use common::state::CharCreationState;
use esm_parser_lib::RefId;
use esm_parser_lib::types::attribute::Attribute;
use esm_parser_lib::types::skill::SkillIndex;
use serializer::SerializableTransform;
use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Instant;

use crate::camera::MWCameraTarget;
use crate::physics::{AnimationDisplacement, CreatureControllerLink, RootMotionState};
use common::consts::NIF_SCALE_MULTIPLIER;
use esm_store::EsmStore;

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut needs_setup_query: Query<(Entity, &NeedsSetup), Added<NeedsSetup>>,
    esm_store: Res<EsmStore>,
    client_id: Res<ClientNetworkId>,
    mut char_creation_state: ResMut<NextState<CharCreationState>>,
    mut next_available_skeleton_id: ResMut<NextAvailableSkeletonId>,
) {
    for (player_entity, setup_info) in &mut needs_setup_query {
        info!(
            "setting up player with client id: {} {}",
            client_id.0, setup_info.network_id
        );

        // spawn the root of the player
        let player_start_location =
            Vec3::new(2.0 * CELL_SIZE_METERS, 100.8, -9.0 * CELL_SIZE_METERS);
        let can_water_walk = true;
        let on_ground = true;
        let collision_shape_type = CollisionShapeType::RotatingBox;
        let rotationally_invariant = true;
        let collision_shape_center = Vec3::ZERO;
        let collision_shape_extents = Vec3::new(1.0, 5.0, 1.0);
        let mut character_controller = CharacterController::default();
        character_controller.vitals.current_idle_group = "idle1".to_string();
        character_controller.movement.state = CharacterState::RunForward;
        // Sets the character's animations to be based on their movement speed, like walking running
        character_controller.movement.adjust_anim_speed = true;
        let mut creature_stats = CreatureStats::new(setup_info.network_id);
        creature_stats.attributes.insert(
            Attribute::STRENGTH,
            AttributeValue {
                base: 10.0,
                modifier: 0.0,
                damage: 0.0,
            },
        );
        creature_stats.attributes.insert(
            Attribute::SPEED,
            AttributeValue {
                base: 10.0,
                modifier: 0.0,
                damage: 0.0,
            },
        );

        let mut npc_stats = NpcStats::default();
        npc_stats.skills.insert(
            SkillIndex::Athletics.to_refid(),
            SkillValue {
                attribute: AttributeValue {
                    base: 10.0,
                    modifier: 0.0,
                    damage: 0.0,
                },
                progress: 0.0,
            },
        );
        let player_esm_ref = esm_store
            .npcs
            .get_raw(&RefId::from_str("Player"))
            .expect("Missing player data in esm!");
        commands
            .entity(player_entity)
            .insert((
                Player { jumping: false },
                Name::new(format!("Player {}", setup_info.network_id)),
                PositionInterpolation::new(Vec3::ZERO, player_start_location),
                Transform {
                    translation: player_start_location,
                    rotation: NIF_TO_BEVY_ROTATION,
                    ..default()
                },
                Movement::default(),
                creature_stats,
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
                    network_id: setup_info.network_id,
                    entity: player_entity,
                    serializable_transform: SerializableTransform {
                        translation: player_start_location,
                        rotation: NIF_TO_BEVY_ROTATION,
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
                Container::default(),
                npc_stats,
                LiveCellRef {
                    base: CellRefBase::Npc(EsmPtr(player_esm_ref)),
                },
            ));

        let skeleton_id = next_available_skeleton_id.next();

        //spawn the player's mesh
        let player_mesh = spawn_player_mesh(
            player_entity,
            commands.reborrow(),
            &asset_server,
            skeleton_id,
            Transform {
                rotation: NIF_TO_BEVY_ROTATION,
                scale: Vec3::splat(NIF_SCALE_MULTIPLIER),
                ..default()
            },
        );

        commands.entity(player_mesh).insert((
            // for tracking animation times with the animation system
            RootMotionState::default(),
            Name::new("player mesh"),
        ));
        // Add a link to the mesh entity so we can reference it from the player
        commands
            .entity(player_entity)
            .insert(MeshEntityLink(player_mesh));

        //if this is the local player, add camera and marker component
        if client_id.0 == setup_info.network_id {
            commands.entity(player_mesh).insert((
                //Default to first person, so hide the player's mesh
                Visibility::Hidden,
                LocalPlayerMesh,
            ));
            commands.entity(player_entity).insert((
                MWCameraTarget,
                LocalPlayer {
                    network_id: setup_info.network_id,
                },
                SpatialAudioReceiver,
            ));

            // Change the state so we set up the race select UI
            char_creation_state.set(CharCreationState::ChoosingRace);
        }
        commands.entity(player_entity).add_child(player_mesh);
        commands.entity(player_entity).remove::<NeedsSetup>();
    }
}

pub fn spawn_player_offline(mut commands: Commands) {
    info!("spawning player offline");
    commands.spawn(NeedsSetup {
        network_id: 0,
        player_transform: Transform::from_xyz(0.0, 0.0, 0.0),
    });
}
pub fn insert_nif_animator_links(
    event: On<NifAnimatorAdded>,
    nif_animator_q: Query<(Entity, &mut NifAnimator)>,
    creature_q: Query<(Entity, &NifSkeletonId)>,
    mut commands: Commands,
) {
    let Ok((nif_animator_entity, nif_animator)) = nif_animator_q.get(event.0) else {
        return;
    };
    for (creature_entity, creature_skeleton_id) in creature_q.iter() {
        if nif_animator.skeleton_id == creature_skeleton_id.0 {
            commands
                .entity(creature_entity)
                .insert(NifAnimatorLink(nif_animator_entity));
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
            NpcMesh,
            AttachmentType::MainSkeleton { skeleton_id },
            CreatureControllerLink(player_entity),
            transform,
            Visibility::Inherited,
        ))
        .id();
    commands.entity(player_entity).add_child(skeleton_entity);
    commands
        .entity(player_entity)
        .insert(NifSkeletonId(skeleton_id));
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
