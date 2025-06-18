use bevy::prelude::*;
use bevy_nif::nif_animation::{NifAnimator, REGION_ROOT_LOWER_BODY, SkeletonMap};
use bevy_rapier3d::prelude::KinematicCharacterController;
use bevy_rapier3d::prelude::*;
use cdda_bevy::{
    AnimationEvent, NetworkEvent,
    client::{
        actor_defs::{
            ActorState, CharacterMovement, CharacterState, JumpingStateKind, RootMotionState,
        },
        animation::get_anim_components_for_creature,
        defs::{
            CreatureStats, IdToEntityMap, IsThirdPerson, LocalPlayer, Player, PlayerInput,
            RootEntityLink,
        },
        events::SoundEvent,
    },
};

use crate::{actor_funcs::get_current_speed, const_defs::MW_SCALE_REDUCTION};
const GROUND_TIMER: f32 = 0.1;
const JUMP_SPEED: f32 = 0.06;
const GRAVITY: f32 = -0.04;
pub fn player_movement(
    mut player_input: ResMut<PlayerInput>,
    time: Res<Time<Fixed>>,
    mut player_query: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            Option<&KinematicCharacterControllerOutput>,
            &mut ActorState,
            &mut CharacterMovement,
        ),
        With<super::LocalPlayer>,
    >,
    mut commands: Commands,
    cam_q: Query<&Transform, With<Camera3d>>,
    mut vertical_movement: Local<f32>,
    mut grounded_timer: Local<f32>,
    mut was_airborne: Local<bool>,
    is_third_person: Res<IsThirdPerson>,
) {
    for (player_entity, mut controller, output, mut actor_state, mut character_movement) in
        player_query.iter_mut()
    {
        let desired_movement_state = determine_movement_state(
            player_input.move_direction,
            actor_state.is_running,
            actor_state.is_sneaking,
            actor_state.is_in_water,
            player_input.rotation_z,
        );
        player_input.rotation_z = 0.0; //so we know how much we turned since the last physics frame
        // Update the central ActorState component so the animation system knows what to do.
        actor_state.movement.state = desired_movement_state;

        // The rest of the function now ONLY deals with vertical movement (gravity/jump)
        // because horizontal movement is entirely handled by the root motion system.
        let delta_time = time.delta_secs();
        *vertical_movement += GRAVITY * delta_time * controller.custom_mass.unwrap_or(1.0);
        let mut final_horizontal_movement = Vec3::ZERO;
        if let Ok(camera_transform) = cam_q.single() {
            if !is_third_person.0 || actor_state.jumping.state == JumpingStateKind::InAir {
                let forward = Vec3::new(
                    camera_transform.forward().x,
                    0.0,
                    camera_transform.forward().z,
                )
                .normalize_or_zero();
                let right = Vec3::new(camera_transform.right().x, 0.0, camera_transform.right().z)
                    .normalize_or_zero();

                // This is the standard physics-driven movement calculation
                let direction =
                    forward * player_input.move_direction.y + right * player_input.move_direction.x;
                final_horizontal_movement =
                    direction.normalize_or_zero() * get_current_speed(&actor_state) * delta_time;
            }
        }
        let was_grounded = *grounded_timer > 0.0;
        if output.map(|o| o.grounded).unwrap_or(false) {
            *grounded_timer = GROUND_TIMER;
            *vertical_movement = 0.0;
            if !was_grounded {
                let sound_event = SoundEvent {
                    sound_anchor_entity: Some(player_entity),
                    translation: None,
                    sound_path: "sounds/bodyfallMED.wav".to_string(),
                    is_attached_to_local_player: true,
                };
                commands.trigger(sound_event.clone());
                commands.trigger(NetworkEvent::Sound(sound_event));
                //TODO:: maybe move this sound generation to the animation system, it's here
                //because otherwise it doesn't trigger if the player jumps into a sloped wall
            }
        }
        // 1. Set the most basic physical state first.
        let is_on_ground = output.map(|o| o.grounded).unwrap_or(true);
        if is_on_ground {
            // We are on the ground, so we can't be InAir.
            // We might be Landing or just idly on the ground (None).
            if *was_airborne {
                // We just landed.
                actor_state.jumping.state = JumpingStateKind::Landing;
                actor_state.jumping.handled_landing_animation = false;
                commands.trigger(NetworkEvent::Animation(AnimationEvent::Landed));
            } else {
                // We are solidly on the ground.
                actor_state.jumping.state = JumpingStateKind::None;
            }
        } else {
            // We are not on the ground, so we must be in the air.
            // This correctly handles falling off ledges.
            actor_state.jumping.state = JumpingStateKind::InAir;
            if !*was_airborne {
                actor_state.jumping.handled_jump_animation = false;
                commands.trigger(NetworkEvent::Animation(AnimationEvent::JumpStarted));
            }
        }

        // 3. Handle Jump Input, which can override the state.
        // This allows for coyote time jumps.
        if player_input.jump && *grounded_timer > 0.0 {
            *vertical_movement = JUMP_SPEED;
            *grounded_timer = 0.0; // Consume timer

            // Explicitly set state and flag for the animation system
            actor_state.jumping.state = JumpingStateKind::InAir;
            actor_state.jumping.handled_jump_animation = false;
            commands.trigger(NetworkEvent::Animation(AnimationEvent::JumpStarted));
        }

        // 4. Update tracker for the next frame.
        *was_airborne = !is_on_ground;

        if *grounded_timer > 0.0 {
            *grounded_timer -= delta_time;
            if player_input.jump {
                *vertical_movement = JUMP_SPEED;
                *grounded_timer = 0.0;
            }
        }

        player_input.jump = false;

        if *vertical_movement < -0.4 {
            *vertical_movement = -0.4;
        }

        let mut final_movement = controller.translation.unwrap_or(Vec3::ZERO);
        if !is_third_person.0 || actor_state.jumping.state == JumpingStateKind::InAir {
            final_movement.x = final_horizontal_movement.x;
            final_movement.z = final_horizontal_movement.z;
        }
        final_movement.y = *vertical_movement;
        controller.translation = Some(final_movement);
        // Reset the per-frame rotation delta
        character_movement.rotation_z = 0.0;
    }
}
pub fn apply_root_motion_system(
    player_input: Res<PlayerInput>,
    camera_q: Query<&Transform, With<Camera3d>>,
    is_third_person: Res<IsThirdPerson>,

    mut creature_mesh_query: Query<(&mut RootMotionState, &RootEntityLink)>,
    mut actor_query: Query<(&mut KinematicCharacterController, &ActorState), With<LocalPlayer>>,
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

                            root_motion_state.previous_time = current_time;
                            root_motion_state.last_animation_group = current_move_group.clone();
                        }
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
fn determine_movement_state(
    movement_vector: Vec2,
    is_running: bool,
    is_sneaking: bool,
    is_in_water: bool,
    turn_delta: f32,
) -> CharacterState {
    const MOVE_THRESHOLD: f32 = 0.001;
    if is_sneaking {
        if movement_vector.y > MOVE_THRESHOLD {
            return CharacterState::SneakForward;
        }
        if movement_vector.y <= -MOVE_THRESHOLD {
            return CharacterState::SneakBack;
        }
        if movement_vector.x > MOVE_THRESHOLD {
            return CharacterState::SneakRight;
        }
        if movement_vector.x <= -MOVE_THRESHOLD {
            return CharacterState::SneakLeft;
        }
    }

    if is_in_water {
        if movement_vector.y > MOVE_THRESHOLD {
            return if is_running {
                CharacterState::SwimRunForward
            } else {
                CharacterState::SwimWalkForward
            };
        }
        if movement_vector.y <= -MOVE_THRESHOLD {
            return if is_running {
                CharacterState::SwimRunBack
            } else {
                CharacterState::SwimWalkBack
            };
        }
        if movement_vector.x > MOVE_THRESHOLD {
            return if is_running {
                CharacterState::SwimRunRight
            } else {
                CharacterState::SwimWalkRight
            };
        }
        if movement_vector.x <= -MOVE_THRESHOLD {
            return if is_running {
                CharacterState::SwimRunLeft
            } else {
                CharacterState::SwimWalkLeft
            };
        }

        const TURN_THRESHOLD: f32 = 0.001;
        if turn_delta > TURN_THRESHOLD {
            return CharacterState::SwimTurnRight;
        }
        if turn_delta <= -TURN_THRESHOLD {
            return CharacterState::SwimTurnLeft;
        }
    }

    if movement_vector.y > MOVE_THRESHOLD {
        return if is_running {
            CharacterState::RunForward
        } else {
            CharacterState::WalkForward
        };
    }
    if movement_vector.y <= -MOVE_THRESHOLD {
        return if is_running {
            CharacterState::RunBack
        } else {
            CharacterState::WalkBack
        };
    }
    if movement_vector.x > MOVE_THRESHOLD {
        return if is_running {
            CharacterState::RunRight
        } else {
            CharacterState::WalkRight
        };
    }
    if movement_vector.x <= -MOVE_THRESHOLD {
        return if is_running {
            CharacterState::RunLeft
        } else {
            CharacterState::WalkLeft
        };
    }

    const TURN_THRESHOLD: f32 = 0.001;
    if turn_delta > TURN_THRESHOLD {
        return CharacterState::TurnRight;
    }
    if turn_delta <= -TURN_THRESHOLD {
        return CharacterState::TurnLeft;
    }

    CharacterState::Idle
}
