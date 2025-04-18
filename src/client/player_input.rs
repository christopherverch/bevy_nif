use bevy::input::mouse::MouseWheel;
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
#[derive(Resource, Default)]
pub struct IsThirdPerson(pub bool);
const GROUND_TIMER: f32 = 0.5;
const JUMP_SPEED: f32 = 0.1;
const GRAVITY: f32 = -0.07;
pub fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_input: ResMut<PlayerInput>,
    mut is_third_person: ResMut<IsThirdPerson>,
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
    if keyboard_input.just_pressed(KeyCode::Tab) {
        is_third_person.0 = !is_third_person.0;
    }
    player_input.move_direction = move_direction.normalize_or_zero();
    player_input.jump = keyboard_input.just_pressed(KeyCode::Space);
}
pub fn player_movement(
    player_input: Res<PlayerInput>,
    time: Res<Time<Fixed>>,
    mut player_query: Query<
        (
            Entity,
            &mut Transform,
            &mut KinematicCharacterController,
            Option<&KinematicCharacterControllerOutput>,
            &Speed,
        ),
        With<super::Player>,
    >,

    camera_query: Query<&Transform, (With<Camera3d>, Without<super::Player>)>,
    mut player_state: ResMut<super::PlayerState>,
    mut vertical_movement: Local<f32>,
    mut grounded_timer: Local<f32>,
    mut last_ground_height: Local<f32>,
    rapier_context: ReadRapierContext,
    mut highest_air_height: Local<f32>,
) {
    for (entity, mut player_transform, mut controller, output, player_speed) in
        player_query.iter_mut()
    {
        player_state.transform = *player_transform;
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
            // Clear input
            // Check physics ground check
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
                }
                //record our height when we landed so we know if we should take fall damage
                *last_ground_height = player_transform.translation.y;
                // If we jump we clear the grounded tolerance
                if jump_speed > 0.0 {
                    *vertical_movement = jump_speed;
                    *grounded_timer = 0.0;
                }
            }
            println!("player transform: {}", player_transform.translation.y);
            movement.y = *vertical_movement;
            if *vertical_movement < -0.4 {
                *vertical_movement = -0.4;
            }

            if player_transform.translation.y == *highest_air_height {
                println!("reached highest height");
            }
            controller.translation = Some(movement);
            if *highest_air_height < player_transform.translation.y {
                *highest_air_height = player_transform.translation.y;
            }
            println!("Highest air height: {:.4}", *highest_air_height); // Printing with 4 decimal places
        }
    }
}
