use bevy::prelude::*;
use cdda_bevy::client::{
    actor_defs::{ActorState, AnimationDisplacement, CharacterState},
    defs::{IsThirdPerson, LocalPlayer, PlayerInput},
};

use cdda_bevy::client::actor_funcs::get_current_speed;

use super::components::CreaturePhysicsState;

/// This system runs before the main movement solver.
/// Its job is to determine the character's desired velocity for this frame
/// based on player input, gravity, and current state (like flying or swimming).
pub fn prepare_character_velocity_system(
    time: Res<Time<Fixed>>,
    mut player_input: ResMut<PlayerInput>,
    mut query: Query<
        (
            &ActorState,
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

    if let Ok((actor_state, mut physics_state, local_anim_displacement, root_transform)) =
        query.single_mut()
    {
        // Get camera facing vector (2d)
        let Ok(cam_transform) = camera_q.single() else {
            return;
        };
        let forward = cam_transform.forward().xz().normalize_or_zero();
        let right = cam_transform.right().xz().normalize_or_zero();

        // Step 1: Determine the desired horizontal velocity based on view mode.
        let horizontal_vel = if is_third_person.0 {
            // --- 3rd Person Logic ---
            let local_space_delta = Vec3::new(
                local_anim_displacement.0.x,
                0.0,
                local_anim_displacement.0.z,
            );

            let target_direction = (forward * player_input.move_direction.y
                + right * player_input.move_direction.x)
                .normalize_or_zero();
            // 3. Get the angle of that direction in the world (from -PI to PI).
            // Using atan2(x, z) is crucial for a Y-up coordinate system.
            let target_angle = if target_direction.length_squared() > 0.0 {
                target_direction.x.atan2(target_direction.y)
            } else {
                // If no input, use the character's current facing direction.
                let facing_vec = root_transform.forward();
                facing_vec.x.atan2(facing_vec.z)
            };
            let anim_angle = get_animation_movement_direction(actor_state.movement.state);
            let rotation = Quat::from_rotation_y(target_angle - anim_angle);

            let world_space_displacement = rotation * local_space_delta;

            if delta_time > 0.0 {
                world_space_displacement / delta_time
            } else {
                Vec3::ZERO
            }
        } else {
            // --- 1st Person Logic ---
            let move_speed = get_current_speed(actor_state);
            let vel_2d = (forward * player_input.move_direction.y
                + right * player_input.move_direction.x)
                .normalize_or_zero()
                * move_speed;

            Vec3::new(vel_2d.x, 0.0, vel_2d.y)
        };

        // Step 2: Combine horizontal velocity with vertical movement (jumping).
        let mut intended_movement = horizontal_vel;
        if player_input.jump {
            intended_movement.y = 8.0; // Example jump impulse
            player_input.jump = false;
        }

        // =======================================================================
        // START: REVISED VELOCITY LOGIC
        // =======================================================================
        let final_vel_for_frame;

        // Condition 1: Jumping from walkable ground.
        // The jump impulse itself becomes the new inertia for this frame[cite: 226].
        if intended_movement.y > 0.0 && physics_state.on_ground && !physics_state.on_slope {
            physics_state.inertia = intended_movement;
            final_vel_for_frame = physics_state.inertia;

        // Condition 2: In the air or on a steep, unwalkable slope.
        // Velocity is a combination of existing inertia (gravity) and new horizontal input[cite: 227].
        } else if !physics_state.on_ground || physics_state.on_slope {
            // Start with the existing inertia from the last frame.
            let mut new_velocity = physics_state.inertia;
            // Override horizontal components with new player input.
            new_velocity.x = intended_movement.x;
            new_velocity.z = intended_movement.z;
            // The vertical component (inertia.y) is preserved, letting gravity continue.
            final_vel_for_frame = new_velocity;

        // Condition 3: Default case - walking on flat ground.
        // The velocity is simply the player's intended movement. Inertia is not involved.
        } else {
            final_vel_for_frame = intended_movement;
        }

        // This is a simplified check for being underwater or flying [cite: 224-225]
        if actor_state.is_flying || actor_state.is_in_water {
            // For flying/swimming, the full 3D vector from input is used directly, ignoring inertia.
            // This requires more complex input handling (e.g., camera pitch).
            // final_vel_for_frame = get_3d_movement_from_input(&cam_transform, &player_input);
        }

        // Store the final calculated velocity for the main solver to use.
        physics_state.velocity_for_frame = final_vel_for_frame;
        // =======================================================================
        // END: REVISED VELOCITY LOGIC
        // =======================================================================
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
