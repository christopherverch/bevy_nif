use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow, Window};
use cdda_bevy::client::defs::{IsThirdPerson, LocalPlayerMesh, TestingComponent};

use cdda_bevy::client::const_defs::FIRST_PERSON_CAMERA_HEIGHT_OFFSET;
pub struct MWCameraPlugin;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CameraSyncSet;
#[derive(Event)]
pub struct ToggleCameraMode;

impl Plugin for MWCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IsThirdPerson::default())
            .add_event::<ToggleCameraMode>()
            .configure_sets(PostUpdate, CameraSyncSet) // DO THIS!
            .add_systems(Update, (toggle_cursor.run_if(toggle_cursor_condition),))
            .add_systems(
                PostUpdate,
                sync_camera_and_player
                    .before(TransformSystem::TransformPropagate)
                    .in_set(CameraSyncSet),
            );
    }
}
#[derive(Component)]
pub struct MWCamera {
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
    /// Mouse x/y sensitivity
    /// Default is Vec2::new(1.0, 1.0)
    pub sensitivity: Vec2,
    /// The camera offset relative to the camera target.
    /// Offset is a tuple. The first value corresponds to the x offset, the second value to the y offset.
    /// Example: offset: Offset::new(<my_x_value>, <my_y_value>);
    /// Default is Offset::new(0.5, 0.4)
    pub offset: Offset,
    /// The min/max bounds of the camera.
    /// This zoom is the position the camera stays in relation to the camera target.
    /// Zoom is a tuple. The first value is the minimum zoom. The smaller the value, the closer the camera can come to it's target. The second value is the maximum zoom. The greater the maximum zoom, the farther away the camera can go from it's target.
    /// The zoom is adjusted using the scroll wheel.
    /// Default is Zoom::new(1.5, 3.0);
    pub zoom: Zoom,
    /// The speed at which the scroll wheel zooms in or out.
    /// Default is 1.0
    pub zoom_sensitivity: f32,
}

impl Default for MWCamera {
    fn default() -> Self {
        MWCamera {
            cursor_lock_key: KeyCode::Escape,
            cursor_lock_toggle_enabled: true,
            cursor_lock_active: true,
            sensitivity: Vec2::new(1.0, 1.0),
            offset: Offset(0.0, 1.6),
            zoom: Zoom::new(3.0, 5.0),
            zoom_sensitivity: 1.0,
        }
    }
}

/// Sets the zoom bounds (min & max)
pub struct Zoom {
    pub min: f32,
    pub max: f32,
    pub radius: f32,
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
pub struct Offset(f32, f32);

#[derive(Component)]
pub struct MWCameraTarget;
fn sync_camera_and_player(
    mut player_q: Query<
        &mut Transform,
        (
            With<MWCameraTarget>,
            Without<LocalPlayerMesh>,
            Without<TestingComponent>,
        ),
    >,
    mut cam_q: Query<
        (&mut MWCamera, &mut Transform),
        (
            Without<MWCameraTarget>,
            Without<LocalPlayerMesh>,
            Without<TestingComponent>,
        ),
    >,
    testing_query: Query<&Transform, (Without<LocalPlayerMesh>, With<TestingComponent>)>,
    is_third_person: Res<IsThirdPerson>,
) {
    let Ok(mut player) = player_q.single_mut() else {
        return;
    };
    let Ok((cam, mut cam_transform)) = cam_q.single_mut() else {
        return;
    };

    // Calculate the desired camera translation based, radius, and xy_offset
    let rotation_matrix = Mat3::from_quat(cam_transform.rotation);

    let offset = rotation_matrix.mul_vec3(Vec3::new(cam.offset.0, cam.offset.1, 0.0));
    let cam_zoom_radius = if is_third_person.0 {
        cam.zoom.radius
    } else {
        0.4
    };
    let desired_translation =
        rotation_matrix.mul_vec3(Vec3::new(0.0, 0.0, cam_zoom_radius)) + offset;

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

        cam_transform.translation =
            player.translation + Vec3::new(0.0, FIRST_PERSON_CAMERA_HEIGHT_OFFSET, 0.0);
    }
}
fn toggle_cursor(
    mut cam_q: Query<&mut MWCamera>,
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
fn toggle_cursor_condition(cam_q: Query<&MWCamera>) -> bool {
    let Ok(cam) = cam_q.single() else {
        return true;
    };
    cam.cursor_lock_toggle_enabled
}
