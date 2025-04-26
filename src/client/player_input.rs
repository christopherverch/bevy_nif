use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use cdda_bevy::AnimationState;
use cdda_bevy::client::defs::{IsThirdPerson, LocalPlayer, PlayerInput, PlayerState, Speed};
use cdda_bevy::client::events::RequestAnimation;
use std::time::Instant;
const GROUND_TIMER: f32 = 0.5;
const JUMP_SPEED: f32 = 0.06;
const GRAVITY: f32 = -0.04;
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
    player_entity: Res<LocalPlayer>,
    mut commands: Commands,
    mut player_state: ResMut<PlayerState>,
) {
    let mut move_direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) {
        move_direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        move_direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        move_direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        move_direction.x += 1.0;
    }
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        player_state.is_ready_weapon = !player_state.is_ready_weapon;
        commands.trigger(RequestAnimation {
            id: player_state.id,
            animation_state: AnimationState::Swinging,
        });
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        if let Some(player) = player_entity.0 {
            if is_third_person.0 {
                commands.entity(player).insert(Visibility::Visible);
            } else {
                commands.entity(player).insert(Visibility::Hidden);
            }
        }
        is_third_person.0 = !is_third_person.0;
    }
    player_input.move_direction = move_direction.normalize_or_zero();
    if !player_input.jump {
        player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
    }
    if buttons.just_pressed(MouseButton::Left) {
        player_state.started_attacking = Instant::now();
        player_state.is_attacking = true;
        commands.trigger(RequestAnimation {
            id: player_state.id,
            animation_state: AnimationState::Swinging,
        });
    }
    if buttons.pressed(MouseButton::Left) {}
    if buttons.just_released(MouseButton::Left) {
        player_state.is_attacking = false;
        //TEMPORARY, JUST TO MAKE IT SHOW THE ANIMATION ONLINE
        player_state.secondary_animation = AnimationState::Idle;
        commands.trigger(RequestAnimation {
            id: player_state.id,
            animation_state: AnimationState::Idle,
        });
        println!("attack!");
    }
}
pub fn player_movement(
    mut player_input: ResMut<PlayerInput>,
    time: Res<Time<Fixed>>,
    mut player_query: Query<
        (
            &mut Transform,
            &mut KinematicCharacterController,
            Option<&KinematicCharacterControllerOutput>,
            &Speed,
        ),
        With<super::LocalPlayer>,
    >,
    mut commands: Commands,
    mut player_state: ResMut<super::PlayerState>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<super::LocalPlayer>)>,
    mut vertical_movement: Local<f32>,
    mut grounded_timer: Local<f32>,
    mut last_ground_height: Local<f32>,
    //   rapier_context: ReadRapierContext,
    mut highest_air_height: Local<f32>,
) {
    for (mut player_transform, mut controller, output, player_speed) in player_query.iter_mut() {
        player_state.transform = *player_transform;
        //handle animations
        //if we're on the ground
        if output.map(|o| o.grounded).unwrap_or(false) {
            //if we just transitioned to idle:
            if player_input.move_direction.x == 0.0 && player_input.move_direction.y == 0.0 {
                if player_state.current_animation != AnimationState::Idle {
                    player_state.current_animation = AnimationState::Idle;
                    commands.trigger(RequestAnimation {
                        id: player_state.id,
                        animation_state: AnimationState::Idle,
                    });
                }
            } else {
                //if we just transitioned to running:
                if player_state.current_animation != AnimationState::Running {
                    player_state.current_animation = AnimationState::Running;

                    commands.trigger(RequestAnimation {
                        id: player_state.id,
                        animation_state: AnimationState::Running,
                    });
                }
            }
        }
        if let Ok(camera_transform) = camera_query.get_single() {
            let delta_time = time.delta_secs();
            let forward = camera_transform.forward().normalize();
            let right = camera_transform.right().normalize();

            // Project input onto the ground plane (remove y component)
            let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

            // Calculate movement vector relative to camera
            let mut direction = (forward_flat * player_input.move_direction.y
                + right_flat * player_input.move_direction.x)
                * 5.0
                * delta_time;
            direction.y = 0.0;
            let mut movement = direction.normalize_or_zero() * player_speed.0 / 100.0;
            let jump_speed = if player_input.jump { 1.0 } else { 0.0 } * JUMP_SPEED;
            //clear input
            player_input.jump = false;
            *vertical_movement += GRAVITY * delta_time * controller.custom_mass.unwrap_or(1.0);
            let was_grounded = *grounded_timer > 0.0;
            if output.map(|o| o.grounded).unwrap_or(false) {
                *grounded_timer = GROUND_TIMER;
                *vertical_movement = 0.0;
                if !was_grounded {}
            }
            // If we are grounded we can jump
            if *grounded_timer > 0.0 {
                *grounded_timer -= delta_time;
                /*
                // Perform a raycast downwards from the player's position
                let shape = Collider::cuboid(0.25, 0.5, 0.25);
                let shape_pos = player_transform.translation + Vec3::new(0.0, 0.0, 0.0);
                let shape_rot = Quat::from_rotation_z(0.0);
                let shape_vel = Vec3::new(0.0, -0.005, 0.0);
                let filter = QueryFilter::default().exclude_collider(entity);
                let options = ShapeCastOptions {
                    max_time_of_impact: 10.0,
                    target_distance: 0.0,
                    stop_at_penetration: false,
                    compute_impact_geometry_on_penetration: false,
                };
                let context = rapier_context.single();
                if let Some((entity, hit)) =
                    context.cast_shape(shape_pos, shape_rot, shape_vel, &shape, options, filter)
                {
                    println!(
                        "Hit the entity {:?} with the configuration: {:?}",
                        entity, hit
                    );
                    // The first collider hit has the entity `entity`. The `hit` is a
                    // structure containing details about the hit configuration.
                    if let Some(h) = hit.details {
                        player_transform.translation.y = h.witness1.y + 0.53;
                    }
                }*/
                //record our height when we landed so we know if we should take fall damage
                *last_ground_height = player_transform.translation.y;
                // If we jump we clear the grounded tolerance
                if jump_speed > 0.0 {
                    commands.trigger(RequestAnimation {
                        id: player_state.id,
                        animation_state: AnimationState::Jumping,
                    });
                    *vertical_movement = jump_speed;
                    *grounded_timer = 0.0;
                }
            }
            movement.y = *vertical_movement;
            if *vertical_movement < -0.4 {
                *vertical_movement = -0.4;
            }

            controller.translation = Some(movement);
            player_state.transform.translation = player_transform.translation;
            // apply the offset if offset_enabled is true

            let camera_forward = camera_transform.forward().normalize();

            // Project the camera's forward vector onto the ground plane (remove y component)
            let target_forward =
                Vec3::new(-camera_forward.x, 0.0, -camera_forward.z).normalize_or_zero();
            let target_rotation = Quat::from_rotation_arc(Vec3::NEG_Z, target_forward);
            player_transform.rotation = target_rotation;
            if *highest_air_height < player_transform.translation.y {
                *highest_air_height = player_transform.translation.y;
            }
            //println!("Highest air height: {:.4}", *highest_air_height); // Printing with 4 decimal places
        }
    }
}
