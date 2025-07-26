use std::f32::consts::PI;

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use tes3mp_rust::actor_defs::{ActorState, CharacterState};
use tes3mp_rust::defs::{IsThirdPerson, LocalPlayer, PlayerInput};
use tes3mp_rust::events::{PlayerReadyWeapon, ToggledThirdPerson};

use crate::camera::MWCamera;
pub struct MWInputPlugin;

// A constant array of weapon types to cycle through for testing.
const WEAPON_TYPES: [i32; 8] = [
    0,  // Hand-to-hand
    1,  // 1h (Shortblade)
    5,  // 2h (Longblade)
    8,  // Spear
    9,  // Bow
    10, // Crossbow
    11, // Throwing
    12, // Spell
];
impl Plugin for MWInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, orbit_mouse.run_if(orbit_condition))
            .add_systems(Update, zoom_mouse)
            .insert_resource(IsThirdPerson::default())
            .add_systems(Update, keyboard_input)
            .add_systems(Update, mouse_input);
    }
}
pub fn mouse_input(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
) {
    player_input.attack_is_held = mouse_button_input.pressed(MouseButton::Left);
    player_input.attack_just_pressed = mouse_button_input.just_pressed(MouseButton::Left);
}
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
    mut commands: Commands,
    mut player_state_query: Query<(Entity, &mut ActorState), With<LocalPlayer>>,
    mut current_weapon_index: Local<usize>,
) {
    let Some((player_entity, mut actor_state)) = player_state_query.iter_mut().next() else {
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

    // DEBUG
    if keyboard_input.just_pressed(KeyCode::KeyQ) {
        // Increment the index, wrapping around if it goes past the end of the array.
        *current_weapon_index = (*current_weapon_index + 1) % WEAPON_TYPES.len();

        let new_weapon_type = WEAPON_TYPES[*current_weapon_index];
        actor_state.combat.weapon_type = new_weapon_type;
        info!("Switched to weapon type: {}", new_weapon_type);
    }
    // DEBUG
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        // Set the state directly. This is what a real combat system would do.
        actor_state.vitals.hit_state = CharacterState::Hit;
        println!("Debug: Forcing Hit state");
    }
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        commands.trigger(PlayerReadyWeapon { player_entity });
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
            commands.reborrow(),
        );
    }
    // do this since physics runs at 60fps but player input checks run faster
    if !player_input.jump {
        player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
    }
}
fn hide_player_conditionally(
    first_person_entities: &Vec<Entity>,
    is_third_person: bool,
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

// only run the orbit system if the cursor lock is disabled
fn orbit_condition(cam_q: Query<&MWCamera>) -> bool {
    let Ok(cam) = cam_q.single() else {
        return true;
    };
    return cam.cursor_lock_active;
}

// heavily referenced https://bevy-cheatbook.github.io/cookbook/pan-orbit-camera.html
pub fn orbit_mouse(
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut cam_q: Query<(&MWCamera, &mut Transform), With<MWCamera>>,
    mut player_input: ResMut<PlayerInput>,
    mut mouse_evr: EventReader<MouseMotion>,
) {
    let mut rotation = Vec2::ZERO;
    for ev in mouse_evr.read() {
        rotation = ev.delta;
        // Track how much the player rotated so we can play the turn animation
        player_input.rotation_z += rotation.x;
    }

    let Ok((cam, mut cam_transform)) = cam_q.single_mut() else {
        return;
    };

    rotation *= cam.sensitivity;

    if rotation.length_squared() > 0.0 {
        let window = window_q.single().unwrap();
        let delta_x = rotation.x / window.width() * std::f32::consts::PI * cam.sensitivity.x;
        let delta_y = rotation.y / window.height() * PI * cam.sensitivity.y;
        let yaw = Quat::from_rotation_y(-delta_x);

        cam_transform.rotation = yaw * cam_transform.rotation;

        let pitch = Quat::from_rotation_x(-delta_y);
        let new_rotation = cam_transform.rotation * pitch;

        let up_vector = new_rotation * Vec3::Y;
        if up_vector.y < 0.2 {
            // Calculate the desired pitch to clamp up_vector.y at 0.2
            let current_up = cam_transform.rotation * Vec3::Y;
            let target_up = Vec3::new(current_up.x, 0.2, current_up.z).normalize(); //clamp y to 0.2

            let rotation_to_target = Quat::from_rotation_arc(current_up, target_up);

            // Apply only the necessary pitch to reach the target
            cam_transform.rotation = rotation_to_target * cam_transform.rotation;
        } else {
            cam_transform.rotation = new_rotation; // Apply the original new rotation
        }
    }
}
pub fn clear_player_rotation_input_system(mut player_input: ResMut<PlayerInput>) {
    player_input.rotation_z = 0.0;
}

fn zoom_mouse(mut scroll_evr: EventReader<MouseWheel>, mut cam_q: Query<&mut MWCamera>) {
    let mut scroll = 0.0;
    for ev in scroll_evr.read() {
        scroll += ev.y;
    }

    if let Ok(mut cam) = cam_q.single_mut() {
        if scroll.abs() > 0.0 {
            let new_radius =
                cam.zoom.radius - scroll * cam.zoom.radius * 0.1 * cam.zoom_sensitivity;
            cam.zoom.radius = new_radius.clamp(cam.zoom.min, cam.zoom.max);
        }
    }
}
