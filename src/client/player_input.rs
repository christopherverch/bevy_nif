use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct PlayerInput {
    pub move_direction: Vec2,
    pub jump: bool,
    // Add other input states as needed
}
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput>,
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

    player_input.move_direction = move_direction.normalize_or_zero();

    player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
}
