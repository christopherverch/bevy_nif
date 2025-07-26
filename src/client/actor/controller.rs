use bevy::prelude::*;
use bevy_nif::nif_animation::NifAnimator;
use tes3mp_rust::{
    actor_defs::{ActorState, CharacterState, JumpingStateKind, UpperBodyState},
    defs::{CreatureStats, IdToEntityMap, Player, PlayerInput},
};

use crate::{
    animation::animation_helpers::get_anim_components_for_creature, physics::CreaturePhysicsState,
};

/// This is the new central "brain" for the character. It reads physics state
/// and player input to make a definitive decision on what the character's
/// animation state should be for this frame.
pub fn character_controller_system(
    time: Res<Time>,
    anim_clips: Res<Assets<AnimationClip>>,
    player_input: Res<PlayerInput>,
    id_to_entity_map: Res<IdToEntityMap>,
    mut query: Query<(
        &mut ActorState,
        &CreatureStats,
        &CreaturePhysicsState,
        Option<&Player>,
    )>,
    mut animator_q: Query<(
        &mut AnimationPlayer,
        &mut NifAnimator,
        &AnimationGraphHandle,
    )>,
) {
    for (mut actor_state, creature_stats, physics_state, player_opt) in query.iter_mut() {
        let Some((_, animation_player, animator, _graph_handle)) =
            get_anim_components_for_creature(creature_stats, &id_to_entity_map, &mut animator_q)
        else {
            continue;
        };
        if player_input.attack_just_pressed
            && actor_state.combat.upper_body_state == UpperBodyState::WeaponEquipped
        {
            actor_state.combat.upper_body_state = UpperBodyState::AttackWindUp;
            let attack_type = determine_movement_based_attack_type(player_input.move_direction);
            actor_state.combat.attack_type = attack_type.to_string();
        }

        if (actor_state.jumping.state == JumpingStateKind::InAir
            || actor_state.jumping.state == JumpingStateKind::Jumping)
            && physics_state.on_ground
        {
            actor_state.jumping.state = JumpingStateKind::Landing;
            actor_state.jumping.handled_landing_animation = false;
        } else if (actor_state.jumping.state == JumpingStateKind::None
            || actor_state.jumping.state == JumpingStateKind::Jumping)
            && !physics_state.on_ground
        {
            actor_state.jumping.state = JumpingStateKind::InAir;
        }

        if physics_state.on_ground && actor_state.jumping.state == JumpingStateKind::None {
            // 1. Determine the raw state directly from player input
            let raw_move_state = determine_raw_move_state(&player_input, &actor_state);

            // 2. Apply smoothing logic to get the final state for this frame
            let effective_move_state = determine_effective_move_state(
                &time,
                &anim_clips,
                &animation_player,
                &animator,
                &mut actor_state,
                player_opt.is_some(),
                raw_move_state,
            );

            // 3. Set the final state
            actor_state.movement.state = effective_move_state;
        }
    }
}

// Helper function containing the logic from your old controller
fn determine_raw_move_state(
    player_input: &PlayerInput,
    actor_state: &ActorState,
) -> CharacterState {
    const MOVE_THRESHOLD: f32 = 0.001;
    const TURN_THRESHOLD: f32 = 10.0;
    let move_vec = player_input.move_direction;
    let turn_delta = player_input.rotation_z;

    if actor_state.is_sneaking {
        if move_vec.y > MOVE_THRESHOLD {
            CharacterState::SneakForward
        } else if move_vec.y < -MOVE_THRESHOLD {
            CharacterState::SneakBack
        } else if move_vec.x > MOVE_THRESHOLD {
            CharacterState::SneakRight
        } else if move_vec.x < -MOVE_THRESHOLD {
            CharacterState::SneakLeft
        } else {
            CharacterState::IdleSneak
        }
    } else if actor_state.is_in_water {
        if move_vec.y > MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::SwimRunForward
            } else {
                CharacterState::SwimWalkForward
            }
        } else if move_vec.y < -MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::SwimRunBack
            } else {
                CharacterState::SwimWalkBack
            }
        } else if move_vec.x > MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::SwimRunRight
            } else {
                CharacterState::SwimWalkRight
            }
        } else if move_vec.x < -MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::SwimRunLeft
            } else {
                CharacterState::SwimWalkLeft
            }
        } else if turn_delta.abs() > TURN_THRESHOLD {
            if turn_delta > 0.0 {
                CharacterState::SwimTurnRight
            } else {
                CharacterState::SwimTurnLeft
            }
        } else {
            CharacterState::IdleSwim
        }
    } else {
        if move_vec.y > MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::RunForward
            } else {
                CharacterState::WalkForward
            }
        } else if move_vec.y < -MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::RunBack
            } else {
                CharacterState::WalkBack
            }
        } else if move_vec.x > MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::RunRight
            } else {
                CharacterState::WalkRight
            }
        } else if move_vec.x < -MOVE_THRESHOLD {
            if actor_state.is_running {
                CharacterState::RunLeft
            } else {
                CharacterState::WalkLeft
            }
        } else if turn_delta.abs() > TURN_THRESHOLD {
            if turn_delta > 0.0 {
                CharacterState::TurnRight
            } else {
                CharacterState::TurnLeft
            }
        } else {
            CharacterState::Idle
        }
    }
}

/// Determines the character's animation state, applying special logic to
/// smooth out turning animations by enforcing a minimum duration. This prevents
/// visual flickering if the player taps a turn key.
fn determine_effective_move_state(
    time: &Time,
    anim_clips: &Assets<AnimationClip>,
    animation_player: &AnimationPlayer,
    animator: &NifAnimator,
    actor_state: &mut ActorState,
    is_player: bool,
    initial_move_state: CharacterState,
) -> CharacterState {
    let last_frame_state = actor_state.movement.last_state;

    let is_currently_turning = matches!(
        last_frame_state,
        CharacterState::TurnLeft
            | CharacterState::TurnRight
            | CharacterState::SwimTurnLeft
            | CharacterState::SwimTurnRight
    );
    let new_state_is_idle = matches!(
        initial_move_state,
        CharacterState::Idle | CharacterState::IdleSneak | CharacterState::IdleSwim
    );

    // If we were turning last frame and are now idle, check if we should
    // continue the turn animation for a short period to make it look smoother.
    if is_currently_turning && new_state_is_idle {
        // Player logic: continue turning until the animation reaches a certain progress threshold.
        if is_player {
            let current_anim_name = &actor_state.movement.current_animation_group;
            let (intro_anim_name, is_in_loop_now) =
                if let Some(base) = current_anim_name.strip_suffix("_loop") {
                    (base.to_string(), true)
                } else {
                    (current_anim_name.clone(), false)
                };

            if let Some(intro_def) = animator.animation_definitions.get(&intro_anim_name) {
                if let Some(loop_anim_name) = &intro_def.next_clip_name {
                    if let Some(loop_def) = animator.animation_definitions.get(loop_anim_name) {
                        if let (Some(intro_clip), Some(loop_clip)) = (
                            anim_clips.get(&intro_def.clip_handle),
                            anim_clips.get(&loop_def.clip_handle),
                        ) {
                            let total_turn_duration = intro_clip.duration() + loop_clip.duration();
                            let mut total_elapsed_time = 0.0;
                            if is_in_loop_now {
                                if let Some(loop_anim_state) =
                                    animation_player.animation(loop_def.node_index)
                                {
                                    total_elapsed_time =
                                        intro_clip.duration() + loop_anim_state.elapsed();
                                }
                            } else if let Some(intro_anim_state) =
                                animation_player.animation(intro_def.node_index)
                            {
                                total_elapsed_time = intro_anim_state.elapsed();
                            }

                            if total_turn_duration > 0.0
                                && (total_elapsed_time / total_turn_duration) < 0.4
                            {
                                return last_frame_state; // Override state to continue turning.
                            }
                        }
                    }
                }
            }
        // NPC logic: use a simple timer.
        } else {
            if actor_state.movement.turn_animation_timer > 0.0 {
                actor_state.movement.turn_animation_timer -= time.delta_secs();
            }
            if matches!(
                initial_move_state,
                CharacterState::TurnLeft | CharacterState::TurnRight
            ) {
                actor_state.movement.turn_animation_timer = 0.05; // Reset timer on new turn.
            } else if actor_state.movement.turn_animation_timer > 0.0 {
                return last_frame_state; // Override state to continue turning.
            }
        }
    }

    // In all other cases, the effective state is just the current input state.
    initial_move_state
}
/// Determines the attack type based on player movement, replicating OpenMW's logic.
fn determine_movement_based_attack_type(move_vec: Vec2) -> &'static str {
    if move_vec.y.abs() > move_vec.x.abs() + 0.2 {
        // Forward-backward
        "thrust"
    } else if move_vec.x.abs() > move_vec.y.abs() + 0.2 {
        // Sideways
        "slash"
    } else {
        // Stationary or diagonal
        "chop"
    }
}
