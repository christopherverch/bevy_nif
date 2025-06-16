use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use cdda_bevy::{
    NetworkEvent,
    client::{
        defs::{IsThirdPerson, PlayerInput},
        events::SoundEvent,
    },
};

use crate::{
    actor_defs::{ActorState, CharacterMovement, CharacterState, JumpingStateKind},
    actor_funcs::get_current_speed,
};
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
        dbg!(*grounded_timer);
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
