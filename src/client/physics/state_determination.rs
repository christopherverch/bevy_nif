use bevy_ecs::{
    query::With,
    system::{Query, ResMut},
};
use cdda_bevy::client::{
    actor_defs::{ActorState, CharacterState},
    defs::{LocalPlayer, PlayerInput},
};

use crate::physics::CreaturePhysicsState;

/// This system reads player input and physics state to determine the
/// high-level `CharacterState` required by the animation system.
pub fn determine_character_state_system(
    mut player_input: ResMut<PlayerInput>,
    mut query: Query<(&mut ActorState, &CreaturePhysicsState), With<LocalPlayer>>,
) {
    for (mut actor_state, physics_state) in query.iter_mut() {
        if !physics_state.on_ground {
            // No state changes while in the air.
            continue;
        }

        const MOVE_THRESHOLD: f32 = 0.001;
        const TURN_THRESHOLD: f32 = 0.001;

        let move_vec = player_input.move_direction;
        let turn_delta = player_input.rotation_z;

        // --- Corrected Logic ---
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
            } // Fallback for sneaking is IdleSneak
        } else if actor_state.is_in_water {
            // Assuming you have a water check
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
            }
            // Only check for turning if there is no linear movement input.
            else if turn_delta.abs() > TURN_THRESHOLD {
                if turn_delta > 0.0 {
                    CharacterState::TurnRight
                } else {
                    CharacterState::TurnLeft
                }
            }
            // If all other checks fail, the character is idle.
            else {
                CharacterState::Idle
            }
        };
        player_input.rotation_z = 0.0;
        actor_state.movement.state = new_state;
    }
}
