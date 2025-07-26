use bevy::prelude::*;

use tes3mp_rust::{
    actor_defs::{ActorState, AnimationDisplacement, CharacterState, JumpingStateKind},
    defs::{IsThirdPerson, LocalPlayer, PlayerInput},
};

use crate::actor::actor_funcs::get_current_speed;

use super::components::CreaturePhysicsState;

const JUMP_IMPULSE: f32 = 8.0;

/// This system runs before the main movement solver.
/// Its job is to determine the character's desired velocity for this frame
/// based on player input, gravity, and current state (like flying or swimming).
pub fn prepare_character_velocity_system(
    time: Res<Time<Fixed>>,
    mut player_input: ResMut<PlayerInput>,
    mut query: Query<
        (
            &mut ActorState, // Changed to mutable
            &mut CreaturePhysicsState,
            &AnimationDisplacement,
            &Transform,
        ),
        With<LocalPlayer>,
    >,
    camera_q: Query<&Transform, With<Camera3d>>,
    is_third_person: Res<IsThirdPerson>,
) {
    let delta_time = time.delta_secs();
    if delta_time == 0.0 {
        return;
    }

    if let Ok((mut actor_state, mut physics_state, local_anim_displacement, root_transform)) =
        query.single_mut()
    {
        let Ok(cam_transform) = camera_q.single() else {
            return;
        };
        let forward = cam_transform.forward().xz().normalize_or_zero();
        let right = cam_transform.right().xz().normalize_or_zero();

        // Step 1: Determine the desired horizontal velocity.
        let horizontal_vel = if physics_state.on_ground {
            // --- ON-GROUND LOGIC ---
            if is_third_person.0 {
                // 3rd Person: Use root motion from the animation.
                // Discard y movement since we handle it with gravity/jumping
                let local_space_delta = Vec3::new(
                    local_anim_displacement.0.x,
                    0.0,
                    local_anim_displacement.0.z,
                );

                // Get the desired world-space direction from the camera and player input.
                let target_direction = (forward * player_input.move_direction.y
                    + right * player_input.move_direction.x)
                    .normalize_or_zero();
                // Convert that direction to a rotation angle.
                let target_angle = if target_direction.length_squared() > 0.0 {
                    target_direction.x.atan2(target_direction.y)
                } else {
                    let facing_vec = root_transform.forward();
                    facing_vec.x.atan2(facing_vec.z)
                };
                // Get the inherent angle of the animation itself (e.g., a "WalkLeft" anim moves at -PI/2).
                let anim_angle = get_animation_movement_direction(actor_state.movement.state);
                // Find the rotational difference to align the animation's motion with the desired world direction.
                let rotation = Quat::from_rotation_y(target_angle - anim_angle);

                let world_space_displacement = rotation * local_space_delta;
                if delta_time > 0.0 {
                    world_space_displacement / delta_time
                } else {
                    Vec3::ZERO
                }
            } else {
                // 1st Person: Use player input directly.
                let move_speed = get_current_speed(&actor_state);
                let vel_2d = (forward * player_input.move_direction.y
                    + right * player_input.move_direction.x)
                    .normalize_or_zero()
                    * move_speed;
                Vec3::new(vel_2d.x, 0.0, vel_2d.y)
            }
        } else {
            // --- IN-AIR LOGIC (for both 1st and 3rd person) ---
            // Air control is always derived from player input, not root motion.
            let move_speed = get_current_speed(&actor_state);
            let vel_2d = (forward * player_input.move_direction.y
                + right * player_input.move_direction.x)
                .normalize_or_zero()
                * move_speed;
            Vec3::new(vel_2d.x, 0.0, vel_2d.y)
        };

        // Step 2: Handle jump input to get the full intended movement for this frame.
        let mut intended_movement = horizontal_vel;
        if player_input.jump {
            if physics_state.on_ground && !physics_state.on_slope {
                intended_movement.y = JUMP_IMPULSE;
                actor_state.jumping.handled_jump_animation = false;
                actor_state.jumping.state = JumpingStateKind::Jumping;
            }
            player_input.jump = false;
        }

        // Step 3: Combine intended movement with inertia based on the physics state.
        let final_vel_for_frame =
            if actor_state.jumping.state == JumpingStateKind::Jumping && physics_state.on_ground {
                // Condition 2: In the air or sliding. Player input provides air control,
                // which is added to the existing inertia (from gravity and momentum).

                physics_state.inertia = intended_movement;
                physics_state.inertia
            } else if !physics_state.on_ground || physics_state.on_slope {
                // Condition 2: In the air or sliding. Player input provides air control,
                // which is added to the existing inertia (from gravity and momentum).

                let air_control_velocity = Vec3::new(intended_movement.x, 0.0, intended_movement.z);
                air_control_velocity + physics_state.inertia
            } else {
                // Condition 3: On solid ground and not jumping. Velocity is determined
                // entirely by player input and animation for this frame
                intended_movement
            };

        physics_state.velocity_for_frame = final_vel_for_frame;
    }
}
/// Returns the inherent angle of motion for an animation in a right-handed Y-up coordinate
/// system where +X is right and -Z is forward. This is used to counteract the animation's
/// built-in direction so it can be rotated to match player input.
/// For example, a "WalkForward" animation moves in the engine's -Z direction, which corresponds
/// to an angle of PI radians (180 degrees).
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
