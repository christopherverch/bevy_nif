use bevy::prelude::*;
use bevy_nif::nif_animation::{BlendMask, NifAnimator, REGION_ROOT_LOWER_BODY, SkeletonMap};
use bevy_rapier3d::prelude::KinematicCharacterController;
use cdda_bevy::{
    AnimationConfig, NetworkEvent, RepeatAnim,
    client::{
        animation::get_anim_components_for_creature,
        defs::{
            CreatureStats, IdToEntityMap, IsThirdPerson, LocalPlayer, MW_SCALE_REDUCTION,
            PlayerInput, PlayerState, RootEntityLink,
        },
        events::{RequestAnimation, SoundEvent, TriggerHit},
    },
};

use crate::actor_defs::{ActorState, CharacterState, JumpingStateKind, RootMotionState};

pub fn apply_hit(
    trigger: Trigger<TriggerHit>,
    mut creature_state_q: Query<(&mut CreatureStats, &mut Transform)>,
    mut commands: Commands,
    local_player_q: Query<(Entity, &mut PlayerState), With<LocalPlayer>>,
    id_to_entity_map: Res<IdToEntityMap>,
    mut is_third_person: ResMut<IsThirdPerson>,
) {
    if trigger.networked {
        commands.trigger(NetworkEvent::Hit(trigger.clone()));
    }
    let Some(_attacker_entity) = id_to_entity_map.0.get(&trigger.attacker_id) else {
        return;
    };
    let Some(receiver_entity) = id_to_entity_map.0.get(&trigger.receiver_id) else {
        return;
    };
    let Ok((mut receiver_creature_stats, _receiver_transform)) =
        creature_state_q.get_mut(*receiver_entity)
    else {
        return;
    };
    let (local_player_entity, _) = local_player_q.single().unwrap();
    let local_player_was_hit = local_player_entity == *receiver_entity;

    receiver_creature_stats.health -= trigger.damage;
    if receiver_creature_stats.health <= 0.0 {
        commands.trigger(RequestAnimation {
            skeleton_id: trigger.receiver_id,
            animation_config: AnimationConfig {
                name: "death4".to_string(),
                repeat: RepeatAnim::Never,
                priority: 10,
                mask: BlendMask::all(),
            },
            networked: trigger.networked,
        });
        //if it's the local player that died
        if local_player_was_hit {
            if !is_third_person.0 {
                is_third_person.0 = !is_third_person.0;
            }
            commands
                .entity(local_player_entity)
                .insert(Visibility::Visible);
        }
    }

    println!("player health: {}", receiver_creature_stats.health);
    commands.trigger(SoundEvent {
        sound_anchor_entity: Some(*receiver_entity),
        translation: None,
        sound_path: "sounds/body hit.wav".to_string(),
        is_attached_to_local_player: local_player_was_hit,
    });
    if trigger.damage > 12.0 {
        println!("requesting knockdown for {}", receiver_entity);
        commands.trigger(RequestAnimation {
            skeleton_id: trigger.receiver_id,
            animation_config: AnimationConfig {
                name: "knockdown".to_string(),
                repeat: RepeatAnim::Never,
                priority: 10,
                mask: BlendMask::all(),
            },
            networked: trigger.networked,
        });
    }
}

// This is a conceptual example
pub fn get_current_speed(actor_state: &ActorState) -> f32 {
    let base = 2.0;

    if actor_state.is_running {
        // Return a higher value for running
        return base * 2.5; // Or whatever multiplier feels right
    } else {
        // Return a lower value for walking
        return base;
    }
    // This would also be modified by magic effects, skills, etc.
}
pub fn apply_root_motion_system(
    // ADD these two parameters to your system's signature
    player_input: Res<PlayerInput>,
    camera_q: Query<&Transform, With<Camera3d>>,
    is_third_person: Res<IsThirdPerson>,

    // The rest of your query structure is correct
    mut creature_mesh_query: Query<(&mut RootMotionState, &RootEntityLink)>,
    mut actor_query: Query<(&mut KinematicCharacterController, &ActorState)>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
    mut bone_transform_query: Query<&mut Transform, Without<Camera3d>>,
    creature_stats_q: Query<&CreatureStats>,
    id_to_entity_map: Res<IdToEntityMap>,
    skeleton_map: Res<SkeletonMap>,
) {
    if !is_third_person.0 {
        return;
    }

    // Iterate over the entities that have the visual mesh and root motion state.
    for (mut root_motion_state, root_entity_link) in creature_mesh_query.iter_mut() {
        let Some(root_entity) = root_entity_link.0 else {
            continue;
        };

        // Use the link to get the components from the parent root entity.
        if let Ok((mut controller, actor_state)) = actor_query.get_mut(root_entity) {
            // Use your helper to get the animation components, which exist on a different entity.
            let Ok(creature_stats) = creature_stats_q.get(root_entity) else {
                continue;
            };
            if actor_state.jumping.state == JumpingStateKind::InAir {
                // We must still clear any horizontal movement instruction from the last frame.
                let vertical_movement = controller.translation.map_or(0.0, |t| t.y);
                controller.translation = Some(Vec3::new(0.0, vertical_movement, 0.0));
                continue; // Skip the rest of the system for this actor
            }
            let Some((_, player, animator, _)) = get_anim_components_for_creature(
                &creature_stats,
                &id_to_entity_map,
                &mut animator_q,
            ) else {
                continue;
            };
            if let Some(skeleton) = skeleton_map.skeletons.get(&animator.skeleton_id) {
                if let Some(root_bone_data) = skeleton.get_bone_by_name(REGION_ROOT_LOWER_BODY) {
                    if let Ok(mut bone_transform) =
                        bone_transform_query.get_mut(root_bone_data.entity)
                    {
                        // Reset the local translation AFTER we have used it for calculations.
                        bone_transform.translation = Vec3::ZERO;
                    }
                }
            }

            let mut final_delta = Vec3::ZERO;
            let current_move_group = &actor_state.movement.current_animation_group;

            if !current_move_group.is_empty() {
                if let Some(definition) = animator.animation_definitions.get(current_move_group) {
                    // Check if this animation has a root motion curve to sample.
                    if let Some(curve) = &definition.root_translation_curve {
                        if let Some(anim_state) = player.animation(definition.node_index) {
                            // --- THE FIX: Use the modulo operator to get the time within the current loop ---
                            let current_time = anim_state.elapsed() % definition.duration;

                            // If the animation group has changed, OR if a loop occurred (detected by
                            // current_time being smaller than previous_time), we must reset the
                            // previous_time to ensure the delta is calculated from a valid starting point.
                            if current_move_group != &root_motion_state.last_animation_group
                                || current_time < root_motion_state.previous_time
                            {
                                root_motion_state.previous_time = current_time;
                            } // Correctly handle when an animation loops while safely unwrapping the Option<Vec3>.
                            let delta = if current_time < root_motion_state.previous_time {
                                // A loop occurred.
                                let pos_at_end_opt = curve.sample(definition.duration);
                                let pos_at_prev_opt = curve.sample(root_motion_state.previous_time);
                                let delta1 = pos_at_end_opt
                                    .zip(pos_at_prev_opt)
                                    .map_or(Vec3::ZERO, |(pos_at_end, pos_at_prev)| {
                                        pos_at_end - pos_at_prev
                                    });

                                let pos_at_start_opt = curve.sample(0.0);
                                let pos_at_now_opt = curve.sample(current_time);
                                let delta2 = pos_at_start_opt
                                    .zip(pos_at_now_opt)
                                    .map_or(Vec3::ZERO, |(pos_at_start, pos_at_now)| {
                                        pos_at_now - pos_at_start
                                    });

                                delta1 + delta2
                            } else {
                                // Normal frame, no loop.
                                let pos_now_opt = curve.sample(current_time);
                                let pos_prev_opt = curve.sample(root_motion_state.previous_time);
                                pos_now_opt
                                    .zip(pos_prev_opt)
                                    .map_or(Vec3::ZERO, |(pos_now, pos_prev)| pos_now - pos_prev)
                            };
                            let Ok(camera_transform) = camera_q.single() else {
                                return;
                            };
                            let scaled_delta = (delta / MW_SCALE_REDUCTION) * anim_state.speed();

                            // --- START: DEFINITIVE ROTATION FIX ---

                            // 1. The animation's displacement is always "forward" in its local space.
                            // We map the NIF's local Y-axis movement to Bevy's -Z axis for "forward".
                            let local_space_delta = Vec3::new(scaled_delta.x, 0.0, -scaled_delta.y);

                            // 2. Get the player's desired world-space direction vector from camera and input.
                            let forward = Vec3::new(
                                camera_transform.forward().x,
                                0.0,
                                camera_transform.forward().z,
                            )
                            .normalize_or_zero();
                            let right = Vec3::new(
                                camera_transform.right().x,
                                0.0,
                                camera_transform.right().z,
                            )
                            .normalize_or_zero();
                            let target_direction = (forward * player_input.move_direction.y
                                + right * player_input.move_direction.x)
                                .normalize_or_zero();

                            // 3. Get the angle of that direction in the world (from -PI to PI).
                            // Using atan2(x, z) is crucial for a Y-up coordinate system.
                            let target_angle = if target_direction.length_squared() > 0.0 {
                                target_direction.x.atan2(target_direction.z)
                            } else {
                                // If no input, use the character's current facing direction.
                                let facing_vec =
                                    bone_transform_query.get(root_entity).unwrap().forward();
                                facing_vec.x.atan2(facing_vec.z)
                            };

                            // 4. Get the animation's "semantic" angle (e.g., PI/2 for left).
                            let anim_angle =
                                get_animation_movement_direction(actor_state.movement.state);

                            // 5. Calculate the final rotation needed.
                            let rotation_angle = target_angle - anim_angle;
                            let rotation = Quat::from_rotation_y(rotation_angle);

                            // 6. Rotate the local-space delta to get the final world-space delta.
                            final_delta = rotation * local_space_delta;

                            // --- END OF DEFINITIVE ROTATION FIX ---

                            root_motion_state.previous_time = current_time;
                            root_motion_state.last_animation_group = current_move_group.clone();
                        }
                    }
                }
            }

            // The bone treadmill reset is still correct and necessary.
            if let Some(skeleton) = skeleton_map.skeletons.get(&animator.skeleton_id) {
                if let Some(root_bone_data) = skeleton.get_bone_by_name(REGION_ROOT_LOWER_BODY) {
                    if let Ok(mut bone_transform) =
                        bone_transform_query.get_mut(root_bone_data.entity)
                    {
                        bone_transform.translation = Vec3::ZERO;
                    }
                }
            }

            let vertical_movement = controller.translation.map_or(0.0, |t| t.y);
            // Apply the final, correctly rotated delta.
            controller.translation =
                Some(Vec3::new(final_delta.x, vertical_movement, final_delta.z));
        }
    }
}
fn get_animation_movement_direction(state: CharacterState) -> f32 {
    use std::f32::consts::PI;
    match state {
        CharacterState::RunLeft
        | CharacterState::SneakLeft
        | CharacterState::SwimWalkLeft
        | CharacterState::SwimRunLeft
        | CharacterState::WalkLeft => -PI / 2.0,

        CharacterState::RunRight
        | CharacterState::SneakRight
        | CharacterState::SwimWalkRight
        | CharacterState::SwimRunRight
        | CharacterState::WalkRight => PI / 2.0,

        CharacterState::RunForward
        | CharacterState::SneakForward
        | CharacterState::SwimWalkForward
        | CharacterState::SwimRunForward
        | CharacterState::WalkForward => PI,

        // All other states, including Forward, are considered to have a base direction of 0 degrees.
        _ => 0.0,
    }
}
