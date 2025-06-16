mod mouse;
use crate::client::defs::{FIRST_PERSON_CAMERA_HEIGHT_OFFSET, IsThirdPerson, MESH_HEIGHT_OFFSET};
use bevy::{
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_rapier3d::plugin::PhysicsSet;
use mouse::MousePlugin;

use super::defs::{LocalPlayerMesh, TestingComponent};

pub struct ThirdPersonCameraPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSyncSet;
#[derive(Event)]
pub struct ToggleCameraMode;

impl Plugin for ThirdPersonCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MousePlugin)
            .insert_resource(IsThirdPerson::default())
            .add_event::<ToggleCameraMode>()
            .configure_sets(PostUpdate, CameraSyncSet.after(PhysicsSet::StepSimulation)) // DO THIS!
            .add_systems(
                Update,
                (
                    toggle_x_offset.run_if(toggle_x_offset_condition),
                    toggle_cursor.run_if(toggle_cursor_condition),
                ),
            )
            .add_systems(
                PostUpdate,
                sync_camera_and_player
                    .before(TransformSystem::TransformPropagate)
                    .in_set(CameraSyncSet),
            );
    }
}
#[derive(Component)]
pub struct ThirdPersonCamera {
    /// Flag to indicate if the cursor lock toggle functionality is turned on.
    /// When enabled and the cursor lock is NOT active, the mouse can freely move about the window without the camera's transform changing.
    /// Example usage: Browsing a character inventory without moving the camera.
    /// Default is true
    pub cursor_lock_toggle_enabled: bool,
    /// Flag to indicate if the cursor is in a locked state or not.
    /// Default is true
    pub cursor_lock_active: bool,
    /// The cursor lock toggle key binding.
    /// Default is KeyCode::Space
    pub cursor_lock_key: KeyCode,
    /// Custom gamepad settings.
    /// Mouse x/y sensitivity
    /// Default is Vec2::new(1.0, 1.0)
    pub sensitivity: Vec2,
    /// Flag to indicate if the orbiting functionality is controlled when a mouse button is being pressed.
    /// Default is false
    pub mouse_orbit_button_enabled: bool,
    /// The mouse button binding to control when the orbiting occurs.
    /// Default is MouseButton:Middle
    pub mouse_orbit_button: MouseButton,
    /// Flag to indicate whether there is a camera offset applied or not.
    /// Default is false
    pub offset_enabled: bool,
    /// The camera offset relative to the camera target.
    /// Offset is a tuple. The first value corresponds to the x offset, the second value to the y offset.
    /// Example: offset: Offset::new(<my_x_value>, <my_y_value>);
    /// Default is Offset::new(0.5, 0.4)
    pub offset: Offset,
    /// Inverts the x value of the offset.
    /// Example: If the x offset is set to 5.0, then the x offset will be inverted to -5.0 if this is set to true.
    /// Default is false
    pub offset_toggle_enabled: bool,
    /// The key binding of the offset toggle
    /// Default is KeyCode::KeyE
    pub offset_toggle_key: KeyCode,
    /// The speed at which the x offset will transition.
    /// Default is 5.0
    pub offset_toggle_speed: f32,
    /// Flag to indicate whether a camera zoom is applied or not.
    /// Default is true
    pub zoom_enabled: bool,
    /// The min/max bounds of the camera.
    /// This is different than the aim zoom. This zoom is the position the camera stays in relation to the camera target.
    /// Zoom is a tuple. The first value is the minimum zoom. The smaller the value, the closer the camera can come to it's target. The second value is the maximum zoom. The greater the maximum zoom, the farther away the camera can go from it's target.
    /// The zoom is adjusted using the scroll wheel.
    /// Default is Zoom::new(1.5, 3.0);
    pub zoom: Zoom,
    /// The speed at which the scroll wheel zooms in or out.
    /// Default is 1.0
    pub zoom_sensitivity: f32,
}

impl Default for ThirdPersonCamera {
    fn default() -> Self {
        ThirdPersonCamera {
            cursor_lock_key: KeyCode::Escape,
            cursor_lock_toggle_enabled: true,
            cursor_lock_active: true,
            sensitivity: Vec2::new(1.0, 1.0),
            mouse_orbit_button_enabled: false,
            mouse_orbit_button: MouseButton::Middle,
            offset_enabled: true,
            offset: Offset::new(0.0, 1.6),
            offset_toggle_enabled: false,
            offset_toggle_speed: 5.0,
            offset_toggle_key: KeyCode::KeyE,
            zoom_enabled: true,
            zoom: Zoom::new(3.0, 5.0),
            zoom_sensitivity: 1.0,
        }
    }
}

/// Sets the zoom bounds (min & max)
pub struct Zoom {
    pub min: f32,
    pub max: f32,
    radius: f32,
}

impl Zoom {
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            min,
            max,
            radius: min,
        }
    }
}

/// Offset the camera behind the player. For example, an offset value of (0.5, 0.25) will
/// place the camera closer the player's right shoulder
pub struct Offset {
    pub offset: (f32, f32),
    offset_copy: (f32, f32),
    is_transitioning: bool,
}

impl Offset {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            offset: (x, y),
            offset_copy: (x, y),
            is_transitioning: false,
        }
    }
}

#[derive(Component)]
pub struct ThirdPersonCameraTarget;
fn sync_camera_and_player(
    mut player_q: Query<
        &mut Transform,
        (
            With<ThirdPersonCameraTarget>,
            Without<LocalPlayerMesh>,
            Without<TestingComponent>,
        ),
    >,
    mut cam_q: Query<
        (&mut ThirdPersonCamera, &mut Transform),
        (
            Without<ThirdPersonCameraTarget>,
            Without<LocalPlayerMesh>,
            Without<TestingComponent>,
        ),
    >,
    testing_query: Query<&Transform, (Without<LocalPlayerMesh>, With<TestingComponent>)>,
    is_third_person: Res<IsThirdPerson>,
    mut mesh_query: Query<&mut Transform, (With<LocalPlayerMesh>, Without<TestingComponent>)>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    let Ok((cam, mut cam_transform)) = cam_q.single_mut() else {
        return;
    };

    // Calculate the desired camera translation based, radius, and xy_offset
    let rotation_matrix = Mat3::from_quat(cam_transform.rotation);

    // apply the offset if offset_enabled is true
    let mut offset = Vec3::ZERO;
    if cam.offset_enabled {
        offset = rotation_matrix.mul_vec3(Vec3::new(cam.offset.offset.0, cam.offset.offset.1, 0.0));
    }
    let cam_zoom_radius = if is_third_person.0 {
        cam.zoom.radius
    } else {
        0.4
    };
    let desired_translation =
        rotation_matrix.mul_vec3(Vec3::new(0.0, 0.0, cam_zoom_radius)) + offset;
    const Y_OFFSET: f32 = 1.0;

    // Define the local offset from the mesh's origin (bottom) to its center
    let Ok(_testing_transform) = testing_query.single() else {
        return;
    };
    let target_forward =
        Vec3::new(-cam_transform.forward().x, 0.0, -cam_transform.forward().z).normalize_or_zero();
    player.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, target_forward);

    if is_third_person.0 {
        cam_transform.translation = player.translation + desired_translation;
    } else {
        //const XZ_OFFSET: f32 = 0.07;

        let camera_offset = Vec3::new(0.0, Y_OFFSET, 0.0);
        cam_transform.translation = player.translation
            + camera_offset
            + Vec3::new(0.0, FIRST_PERSON_CAMERA_HEIGHT_OFFSET, 0.0);
    }
}

/*fn zoom_mouse(mut scroll_evr: EventReader<MouseWheel>, mut cam_q: Query<&mut ThirdPersonCamera>) {
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
}*/

// only run toggle_x_offset if `offset_toggle_enabled` is true
fn toggle_x_offset_condition(cam_q: Query<&ThirdPersonCamera, With<ThirdPersonCamera>>) -> bool {
    let Ok(cam) = cam_q.single() else {
        return false;
    };
    cam.offset_toggle_enabled
}

// inverts the x offset. Example: left shoulder view -> right shoulder view & vice versa
fn toggle_x_offset(
    mut cam_q: Query<&mut ThirdPersonCamera, With<ThirdPersonCamera>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let Ok(mut cam) = cam_q.single_mut() else {
        return;
    };

    let toggle_btn: bool = keys.just_pressed(cam.offset_toggle_key);

    if toggle_btn {
        // Switch direction by inverting the offset_flag
        cam.offset.is_transitioning = !cam.offset.is_transitioning;
    }

    // Determine the transition speed based on direction
    let transition_speed = if cam.offset.is_transitioning {
        -cam.offset_toggle_speed
    } else {
        cam.offset_toggle_speed
    };

    // Update the offset based on the direction and time
    cam.offset.offset.0 = (cam.offset.offset.0 + transition_speed * time.delta_secs())
        .clamp(-cam.offset.offset_copy.0, cam.offset.offset_copy.0);
}

fn toggle_cursor(
    mut cam_q: Query<&mut ThirdPersonCamera>,
    keys: Res<ButtonInput<KeyCode>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut cam) = cam_q.single_mut() else {
        return;
    };

    if keys.just_pressed(cam.cursor_lock_key) {
        cam.cursor_lock_active = !cam.cursor_lock_active;
    }

    if let Ok(mut window) = window_q.single_mut() {
        if cam.cursor_lock_active {
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
            window.cursor_options.visible = false;
        } else {
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
        }
    }
}

// checks if the toggle cursor functionality is enabled
fn toggle_cursor_condition(cam_q: Query<&ThirdPersonCamera>) -> bool {
    let Ok(cam) = cam_q.single() else {
        return true;
    };
    cam.cursor_lock_toggle_enabled
}
