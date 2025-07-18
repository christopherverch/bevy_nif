use bevy_ecs::{
    query::With,
    system::{Commands, Query, ResMut},
};
use cdda_bevy::{
    AnimationEvent, NetworkEvent,
    client::{
        actor_defs::{ActorState, CharacterState, JumpingStateKind},
        defs::{LocalPlayer, PlayerInput},
    },
};

use crate::physics::CreaturePhysicsState;

/// This system reads player input and physics state to determine the high-level
/// `CharacterState` and `JumpingStateKind` required by the animation system.
/// It acts as the bridge between the physics simulation and animation playback.
pub fn determine_character_state_system(
    mut player_input: ResMut<PlayerInput>,
    mut query: Query<(&mut ActorState, &CreaturePhysicsState), With<LocalPlayer>>,
    mut commands: Commands,
) {
    for (mut actor_state, physics_state) in query.iter_mut() {
        // --- Jumping and Landing State Transitions ---

        // Check for landing: If we were in the air, but now we're on the ground, we have landed.
        if (actor_state.jumping.state == JumpingStateKind::InAir
            || actor_state.jumping.state == JumpingStateKind::Jumping)
            && physics_state.on_ground
        {
            actor_state.jumping.state = JumpingStateKind::Landing;
            // Signal the animation system to play the landing animation.
            actor_state.jumping.handled_landing_animation = false;
            commands.trigger(NetworkEvent::Animation(AnimationEvent::Landed));
        }
        // Check for liftoff: If we were on the ground and are now not, we are in the air.
        else if (actor_state.jumping.state == JumpingStateKind::None
            || actor_state.jumping.state == JumpingStateKind::Jumping)
            && !physics_state.on_ground
        {
            actor_state.jumping.state = JumpingStateKind::InAir;
        }

        // --- Ground Movement State Determination ---

        // This logic only runs if we are on the ground and not in the process of landing.
        if physics_state.on_ground {
            const MOVE_THRESHOLD: f32 = 0.001;
            const TURN_THRESHOLD: f32 = 0.001;
            let move_vec = player_input.move_direction;
            let turn_delta = player_input.rotation_z;

            // This logic is the same as your original file, determining the correct
            // walk/run/sneak/idle/turn state based on input.
            let new_state = if actor_state.is_sneaking {
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
                // ... (your existing water logic)
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
                // Default ground movement
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
            };
            // Consume the turn input that was used to determine the TurnLeft/TurnRight state.
            // This prevents it from being processed again in the next frame.
            player_input.rotation_z = 0.0;
            actor_state.movement.state = new_state;
        }
    }
}
