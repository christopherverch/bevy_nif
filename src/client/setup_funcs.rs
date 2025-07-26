use bevy::prelude::*;
use bevy_kira_audio::{EmitterSettings, SpatialAudioEmitter, SpatialAudioReceiver};
use bevy_nif::nif::attach_parts::AttachmentType;
use bevy_nif::nif::spawner::NifScene;
use bevy_nif::nif_animation::{NifAnimator, NifAnimatorAdded};
use bevy_rapier3d::prelude::*;
use kira::Easing;
use std::time::Instant;
use tes3mp_rust::defs::{
    AnimateableEntity, ClientEntityId, CreatureStats, LocalPlayer, LocalPlayerMesh, MeshEntityLink,
    NeedsSetup, NetworkedEntity, Player, RootEntityLink,
};
use tes3mp_rust::serializer::SerializableTransform;

use crate::camera::MWCameraTarget;
use crate::const_defs::NIF_SCALE_MULTIPLIER;
use crate::physics::physics_setup::NeedsNifPhysics;
use tes3mp_rust::actor_defs::{
    ActorState, AnimationDisplacement, CharacterState, CombatState, JumpingState, MovementState,
    RootMotionState, VitalsState,
};

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut needs_setup_query: Query<(Entity, &NeedsSetup), Added<NeedsSetup>>,
    client_id: Res<ClientEntityId>,
) {
    for (player_entity, setup_info) in &mut needs_setup_query {
        info!(
            "setting up player with client id: {} {}",
            client_id.0, setup_info.entity_id
        );

        // spawn the root of the player
        let player_start_location = Vec3::new(0.0, 2.0, 0.0);
        commands.entity(player_entity).insert((
            Player,
            Name::new(format!("Player {}", setup_info.entity_id)),
            Transform::from_translation(player_start_location),
            Visibility::Visible,
            CreatureStats {
                entity_id: setup_info.entity_id,
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
                is_flying: false,
                is_in_water: false,
                last_stuck_position: Vec3::default(),
                stuck_frames: 0,
            },
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
            RigidBody::KinematicPositionBased,
            // Audio
            SpatialAudioEmitter::default(),
            EmitterSettings {
                distances: (2.0..=14.0).into(),
                // We will fully hear this entity within a distance of 2 units, and not hear it at
                // all by 14 units.
                attenuation_function: Easing::Linear,
            },
        ));

        //spawn the player's mesh
        let player_mesh = spawn_player_mesh(
            player_entity,
            commands.reborrow(),
            &asset_server,
            setup_info.entity_id,
            Transform {
                translation: Vec3::new(0.0, 0.0, 0.0),
                rotation: Quat::default(),
                scale: Vec3::splat(NIF_SCALE_MULTIPLIER),
            },
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

        //if this is the local player
        if client_id.0 == setup_info.entity_id {
            commands.entity(player_mesh).insert((
                //Default to first person, so hide the player's mesh
                LocalPlayerMesh,
            ));
            commands.entity(player_entity).insert((
                MWCameraTarget,
                LocalPlayer {
                    entity_id: setup_info.entity_id,
                },
                SpatialAudioReceiver,
            ));
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
    trigger: Trigger<NifAnimatorAdded>,
    nif_animator_q: Query<(Entity, &mut NifAnimator)>,
    mut creature_q: Query<&mut CreatureStats>,
) {
    let Ok((nif_animator_entity, nif_animator)) = nif_animator_q.get(trigger.0) else {
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
    let child = commands
        .spawn((
            NifScene(asset_handle),
            AttachmentType::MainSkeleton { skeleton_id },
            transform,
            InheritedVisibility::VISIBLE,
            NeedsNifPhysics(player_entity),
        ))
        .id();
    commands.entity(player_entity).add_child(child);
    let paths_with_target_bone: Vec<(&str, AttachmentType)> = vec![
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
    spawn_nifs(
        paths_with_target_bone,
        &asset_server,
        child,
        &mut commands,
        Transform::from_scale(Vec3::splat(1.0)),
    );
    child
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
