use std::f32::consts::PI;

use super::ThirdPersonCamera;
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    window::PrimaryWindow,
};

pub struct MousePlugin;

impl Plugin for MousePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, orbit_mouse.run_if(orbit_condition))
            .add_systems(Update, zoom_mouse);
    }
}

// only run the orbit system if the cursor lock is disabled
fn orbit_condition(cam_q: Query<&ThirdPersonCamera>) -> bool {
    let Ok(cam) = cam_q.single() else {
        return true;
    };
    return cam.cursor_lock_active;
}

// heavily referenced https://bevy-cheatbook.github.io/cookbook/pan-orbit-camera.html
pub fn orbit_mouse(
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut cam_q: Query<(&ThirdPersonCamera, &mut Transform), With<ThirdPersonCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_evr: EventReader<MouseMotion>,
) {
    let mut rotation = Vec2::ZERO;
    for ev in mouse_evr.read() {
        rotation = ev.delta;
    }

    let Ok((cam, mut cam_transform)) = cam_q.single_mut() else {
        return;
    };

    if cam.mouse_orbit_button_enabled && !mouse.pressed(cam.mouse_orbit_button) {
        return;
    }

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

            // Use from_rotation_arc instead of from_two_vectors
            let rotation_to_target = Quat::from_rotation_arc(current_up, target_up);

            // Apply only the necessary pitch to reach the target
            cam_transform.rotation = rotation_to_target * cam_transform.rotation;
        } else {
            cam_transform.rotation = new_rotation; // Apply the original new rotation
        }
    }
}

fn zoom_mouse(mut scroll_evr: EventReader<MouseWheel>, mut cam_q: Query<&mut ThirdPersonCamera>) {
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
