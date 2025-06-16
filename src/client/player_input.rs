use std::time::Instant;

use bevy::prelude::*;
use cdda_bevy::client::defs::{AttackState, IsThirdPerson, LocalPlayer, PlayerInput, PlayerState};
use cdda_bevy::client::events::ToggledThirdPerson;

use crate::actor_defs::ActorState;
use crate::combat::execute_attack;

pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
    mut commands: Commands,
    mut player_state_query: Query<(Entity, &mut PlayerState, &mut ActorState), With<LocalPlayer>>,
    cameras: Query<(&Camera, &Transform)>,
) {
    let Some((player_entity, mut player_state, mut actor_state)) =
        player_state_query.iter_mut().next()
    else {
        return; // Exit early if no player is found
    };

    let mut first_person_entities = Vec::new();
    first_person_entities.push(player_entity);
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
    if keyboard_input.just_pressed(KeyCode::CapsLock) {
        player_input.run = !player_input.run;
        actor_state.is_running = player_input.run;
    }
    player_input.move_direction = move_direction.normalize_or_zero();
    if keyboard_input.just_pressed(KeyCode::ControlLeft) {
        player_input.sneak = !player_input.sneak;
        actor_state.is_sneaking = player_input.sneak;
    }
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        if player_state.attack_state == AttackState::Ready {
            player_state.attack_state = AttackState::Inactive;
        } else {
            player_state.attack_state = AttackState::Ready;
        }
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        is_third_person.0 = !is_third_person.0;
        commands.trigger(ToggledThirdPerson {
            entity: player_entity,
            is_third_person: is_third_person.0,
        });
        hide_player_conditionally(
            &first_person_entities,
            is_third_person.0,
            //&player_state,
            //&mesh_query,
            commands.reborrow(),
        );
    }
    // do this since physics runs at 60fps but player input checks run faster
    if !player_input.jump {
        player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
    }
    handle_player_attack_system(
        buttons,
        &player_entity,
        &mut player_state,
        cameras,
        commands,
    );
}
fn hide_player_conditionally(
    first_person_entities: &Vec<Entity>,
    is_third_person: bool,
    //player_state: &PlayerState,
    //  mesh_query: &Query<Entity, With<LocalPlayerMesh>>,
    mut commands: Commands,
) {
    //TEMPORARY
    for entity in first_person_entities {
        if !is_third_person {
            commands.entity(*entity).insert(Visibility::Hidden);
        } else {
            commands.entity(*entity).insert(Visibility::Visible);
        }
    }
}
pub fn handle_player_attack_system(
    buttons: Res<ButtonInput<MouseButton>>,
    player_entity: &Entity,
    player_state: &mut PlayerState,
    camera_query: Query<(&Camera, &Transform)>, // For raycasting
    mut commands: Commands,
) {
    if buttons.just_pressed(MouseButton::Left) {
        if player_state.attack_state == AttackState::Ready {
            player_state.started_attacking = Instant::now();
            player_state.attack_state = AttackState::Charging;
            // Consider triggering an "AttackChargeStart" animation/sound event here
        }
    }

    // Logic for cooldown after releasing an attack
    if player_state.attack_state == AttackState::Releasing
        && player_state.started_attacking.elapsed().as_millis() >= 300
    {
        player_state.attack_state = AttackState::Ready;
    }

    // Logic for ChargeImmediateRelease
    if player_state.attack_state == AttackState::ChargeImmediateRelease {
        // Min charge time
        if player_state.started_attacking.elapsed().as_millis() >= 300 {
            for (_camera, camera_transform) in &camera_query {
                execute_attack(
                    *player_entity,
                    player_state,
                    camera_transform,
                    &mut commands,
                );
            }
        }
    } else {
        // This 'else' ensures this block doesn't run if we just transitioned ChargeImmediateRelease -> Charging
        if buttons.just_released(MouseButton::Left) {
            if player_state.attack_state == AttackState::Charging {
                if player_state.started_attacking.elapsed().as_millis() < 300 {
                    // Quick tap resulted in release
                    player_state.attack_state = AttackState::ChargeImmediateRelease;
                } else {
                    // Normal release after proper charge
                    if let Ok((_camera, camera_transform)) = camera_query.single() {
                        execute_attack(
                            *player_entity,
                            player_state,
                            camera_transform,
                            &mut commands,
                        );
                        player_state.started_attacking = Instant::now(); // Reset for cooldown timing 
                        // Consider triggering an "AttackRelease" animation/sound event here
                    }
                }
            }
        }
    }
}
