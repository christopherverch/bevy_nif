use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
#[derive(Component)]
pub struct Speed(pub f32);
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

pub fn player_movement(
    player_input: Res<PlayerInput>,
    time: Res<Time>,
    mut player_query: Query<
        (&mut ExternalImpulse, &mut Transform, &Speed),
        (With<super::Player>, With<super::ThirdPersonCameraTarget>),
    >,
    camera_query: Query<&Transform, (With<Camera3d>, Without<super::Player>)>,
    mut player_state: ResMut<super::PlayerState>,
) {
    for (mut ext_impulse, mut player_transform, player_speed) in player_query.iter_mut() {
        player_state.transform = *player_transform;
        if let Ok(camera_transform) = camera_query.get_single() {
            // Get camera's forward and right vectors (normalized)
            let forward = camera_transform.forward().normalize();
            let right = camera_transform.right().normalize();

            // Project input onto the ground plane (remove y component)
            let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

            // Calculate movement vector relative to camera
            let mut direction = (forward_flat * player_input.move_direction.y
                + right_flat * player_input.move_direction.x)
                * 5.0
                * time.delta_secs();
            direction.y = 0.0;
            let movement = direction.normalize_or_zero() * player_speed.0 * time.delta_secs();
            ext_impulse.impulse += movement;

            player_transform.look_to(direction, Vec3::Y);
            if player_input.jump {
                ext_impulse.impulse += Vec3::new(0.0, 1., 0.0);
            }
        }
    }
}
