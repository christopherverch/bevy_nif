use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use cdda_bevy::client::animation::trigger_request_animation;
use cdda_bevy::client::defs::{
    AttackState, FirstPersonMeshEntityLinks, IsThirdPerson, LocalPlayer, LocalPlayerMesh,
    PlayerInput, PlayerState, Speed,
};
use cdda_bevy::client::events::ToggledThirdPerson;
use cdda_bevy::{AnimationState, RequestedAnimation};
use std::time::Instant;
const GROUND_TIMER: f32 = 0.5;
const JUMP_SPEED: f32 = 0.06;
const GRAVITY: f32 = -0.04;
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
    mut commands: Commands,
    mut player_state_query: Query<
        (Entity, &mut PlayerState, &FirstPersonMeshEntityLinks),
        With<LocalPlayer>,
    >,
    mesh_query: Query<Entity, With<LocalPlayerMesh>>,
) {
    let Some((player_entity, mut player_state, first_person_entities)) =
        player_state_query.iter_mut().next()
    else {
        return; // Exit early if no player is found
    };
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
        let requested_anim: RequestedAnimation;
        if player_state.is_ready_weapon {
            player_state.attack_state = AttackState::Ready;
            requested_anim = RequestedAnimation::SwingReady;
        } else {
            player_state.attack_state = AttackState::Inactive;
            requested_anim = RequestedAnimation::SwingInactive;
        }
        hide_player_conditionally(
            &first_person_entities.0,
            is_third_person.0,
            &player_state,
            &mesh_query,
            commands.reborrow(),
        );
        trigger_request_animation(
            commands.reborrow(),
            player_entity,
            requested_anim,
            &mut player_state,
        );
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        is_third_person.0 = !is_third_person.0;
        commands.trigger(ToggledThirdPerson {
            entity: player_entity,
            is_third_person: is_third_person.0,
        });
        hide_player_conditionally(
            &first_person_entities.0,
            is_third_person.0,
            &player_state,
            &mesh_query,
            commands.reborrow(),
        );
    }
    player_input.move_direction = move_direction.normalize_or_zero();
    if !player_input.jump {
        player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
    }
    if buttons.just_pressed(MouseButton::Left) {
        if player_state.attack_state == AttackState::Ready {
            player_state.started_attacking = Instant::now();
            player_state.attack_state = AttackState::Charging;
            trigger_request_animation(
                commands.reborrow(),
                player_entity,
                RequestedAnimation::SwingCharging,
                &mut player_state,
            );
        }
    }
    if buttons.pressed(MouseButton::Left) {}
    if player_state.attack_state == AttackState::ChargeImmediateRelease {
        if player_state.started_attacking.elapsed().as_millis() >= 300 {
            player_state.attack_state = AttackState::Charging;
            trigger_attack(player_entity, &mut player_state, commands);
        }
    } else {
        if buttons.just_released(MouseButton::Left) {
            if player_state.started_attacking.elapsed().as_millis() < 300 {
                //if we released it faster than the minimum attack delay
                player_state.attack_state = AttackState::ChargeImmediateRelease;
            } else {
                trigger_attack(player_entity, &mut player_state, commands);
            }
        }
    }
}
fn trigger_attack(player_entity: Entity, player_state: &mut PlayerState, mut commands: Commands) {
    if player_state.attack_state == AttackState::Charging {
        player_state.attack_state = AttackState::Releasing;
        trigger_request_animation(
            commands.reborrow(),
            player_entity,
            RequestedAnimation::SwingReleasing,
            player_state,
        );
        println!("attack!");
    }
}
pub fn player_movement(
    mut player_input: ResMut<PlayerInput>,
    time: Res<Time<Fixed>>,
    mut player_query: Query<
        (
            Entity,
            &mut Transform,
            &mut KinematicCharacterController,
            Option<&KinematicCharacterControllerOutput>,
            &mut PlayerState,
            &Speed,
        ),
        (With<super::LocalPlayer>, Without<LocalPlayerMesh>),
    >,
    camera_query: Query<
        &Transform,
        (
            With<Camera3d>,
            Without<super::LocalPlayer>,
            Without<LocalPlayerMesh>,
        ),
    >,
    mut commands: Commands,
    mut vertical_movement: Local<f32>,
    mut grounded_timer: Local<f32>,
    mut last_ground_height: Local<f32>,
    //   rapier_context: ReadRapierContext,
    mut highest_air_height: Local<f32>,
) {
    for (
        player_entity,
        mut player_transform,
        mut controller,
        output,
        mut player_state,
        player_speed,
    ) in player_query.iter_mut()
    {
        player_state.transform = *player_transform;
        //handle animations
        //if we're on the ground
        if output.map(|o| o.grounded).unwrap_or(false) {
            //if we just transitioned to idle:
            if player_input.move_direction.x == 0.0 && player_input.move_direction.y == 0.0 {
                if player_state.current_animation != AnimationState::Idle {
                    player_state.current_animation = AnimationState::Idle;
                    trigger_request_animation(
                        commands.reborrow(),
                        player_entity,
                        AnimationState::Idle.into(),
                        &mut player_state,
                    );
                }
            } else {
                //if we just transitioned to running:
                if player_state.current_animation != AnimationState::Running {
                    player_state.current_animation = AnimationState::Running;

                    trigger_request_animation(
                        commands.reborrow(),
                        player_entity,
                        AnimationState::Running.into(),
                        &mut player_state,
                    );
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
                //record our height when we landed so we know if we should take fall damage
                *last_ground_height = player_transform.translation.y;
                // If we jump we clear the grounded tolerance
                if jump_speed > 0.0 {
                    trigger_request_animation(
                        commands.reborrow(),
                        player_entity,
                        AnimationState::Jumping.into(),
                        &mut player_state,
                    );
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

            if *highest_air_height < player_transform.translation.y {
                *highest_air_height = player_transform.translation.y;
            }
            //println!("Highest air height: {:.4}", *highest_air_height); // Printing with 4 decimal places
        }
    }
}
fn hide_player_conditionally(
    first_person_entities: &Vec<Entity>,
    is_third_person: bool,
    player_state: &PlayerState,
    mesh_query: &Query<Entity, With<LocalPlayerMesh>>,
    mut commands: Commands,
) {
    if let Some(torso_entity) = mesh_query.iter().next() {
        let (first_person_visibility, torso_visibility) = if is_third_person {
            (Visibility::Visible, Visibility::Visible)
        } else if player_state.is_ready_weapon {
            (Visibility::Visible, Visibility::Hidden)
        } else {
            (Visibility::Hidden, Visibility::Hidden)
        };

        for entity in first_person_entities {
            commands.entity(*entity).insert(first_person_visibility);
        }

        commands.entity(torso_entity).insert(torso_visibility);
    }
}
