use std::f32::consts::PI;

use crate::defs::{LocalPlayerMesh, PlayerInput};
use crate::events::ToggledThirdPerson;
use crate::mechanics::get_local_player_intent;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::MWCamera;
pub struct MWInputPlugin;

impl Plugin for MWInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, orbit_mouse.run_if(orbit_condition))
            .add_systems(Update, zoom_mouse)
            .add_systems(Update, get_local_player_intent)
            .add_systems(Update, mouse_input)
            .add_observer(on_toggled_third_person);
    }
}
pub fn mouse_input(
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut player_input: ResMut<PlayerInput>,
) {
    player_input.attack_is_held = mouse_button_input.pressed(MouseButton::Left);
    player_input.attack_just_pressed = mouse_button_input.just_pressed(MouseButton::Left);
}
pub fn on_toggled_third_person(
    _event: On<ToggledThirdPerson>,
    mut local_player_mesh: Single<&mut Visibility, With<LocalPlayerMesh>>,
    cam_settings: Single<&MWCamera>,
) {
    //TEMPORARY
    //hide the local player's mesh
    if cam_settings.first_person {
        **local_player_mesh = Visibility::Hidden;
    } else {
        **local_player_mesh = Visibility::Inherited;
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
    mut mouse_msgs: MessageReader<MouseMotion>,
) {
    let mut rotation = Vec2::ZERO;
    for ev in mouse_msgs.read() {
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

fn zoom_mouse(mut scroll_msgs: MessageReader<MouseWheel>, mut cam_q: Query<&mut MWCamera>) {
    let mut scroll = 0.0;
    for ev in scroll_msgs.read() {
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
