use bevy::prelude::*;
use common::mechanics::{CharacterController, CharacterState};

use crate::{
    camera::MWCamera,
    defs::LocalPlayer,
    events::{PlayerReadyWeapon, ToggledThirdPerson},
};
#[derive(Component, Debug, Default, Clone)]
pub struct ActorIntent {
    /// Desired horizontal movement vector (X = strafe, Y = forward/back)
    pub movement: Vec2,
    pub pitch_change: f32,
    pub yaw_change: f32,
    pub jump: bool,
    pub run: bool,
    pub sneak: bool,
    pub attack_intent: AttackIntentKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AttackIntentKind {
    #[default]
    None,
    Any,
    Slash,
    Chop,
    Thrust,
}
pub fn get_local_player_intent(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_entity_intent: Single<
        (Entity, &mut ActorIntent, &mut CharacterController),
        With<LocalPlayer>,
    >,
    mut cam: Single<&mut MWCamera>,
    mut current_weapon_index: Local<usize>,
) {
    let (player_entity, mut player_intent, mut character_controller) =
        player_entity_intent.into_inner();
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
        player_intent.run = !player_intent.run;
    }
    player_intent.movement = move_direction.normalize_or_zero();
    if keyboard_input.just_pressed(KeyCode::ControlLeft) {
        player_intent.sneak = !player_intent.sneak;
    }

    // DEBUG
    if keyboard_input.just_pressed(KeyCode::KeyQ) {
        // Increment the index, wrapping around if it goes past the end of the array.
        *current_weapon_index = (*current_weapon_index + 1) % WEAPON_TYPES.len();

        let new_weapon_type = WEAPON_TYPES[*current_weapon_index];
        character_controller.combat.weapon_type = new_weapon_type;
        info!("Switched to weapon type: {}", new_weapon_type);
    }
    // DEBUG
    if keyboard_input.just_pressed(KeyCode::KeyH) {
        // Set the state directly. This is what a real combat system would do.
        character_controller.vitals.hit_state = CharacterState::Hit;
        println!("Debug: Forcing Hit state");
    }
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        commands.trigger(PlayerReadyWeapon { player_entity });
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        cam.first_person = !cam.first_person;
        commands.trigger(ToggledThirdPerson);
    }
    if keyboard_input.just_pressed(KeyCode::Space) {
        player_intent.jump = true;
    }
}
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
