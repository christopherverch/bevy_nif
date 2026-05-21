use avian3d::prelude::Collider;
use bevy::prelude::*;
#[derive(Component, Debug)]
pub struct CreaturePhysicsState {
    /// The final velocity vector calculated by the preparation system for this frame.
    /// The main movement solver will consume this value.
    pub velocity_for_frame: Vec3,

    /// The persistent velocity from external forces, primarily gravity.
    pub inertia: Vec3,

    /// Is the character currently on a surface considered walkable ground?
    pub on_ground: bool,
    pub on_slope: bool,

    // --- Transient State for the Iterative Solver ---
    // These fields are reset each frame but used between iterations of the movement loop.
    /// The normal of the last surface the character slid against.
    pub last_slide_normal: Vec3,

    /// The normal of the slide before the most recent one. Used for crevice logic.
    pub last_slide_normal_fallback: Vec3,

    /// The number of times the character has slid against a surface in the current frame.
    pub times_slid: u32,

    // The single, compound collider shape for this character.
    // We clone it from the entity when the component is added.
    pub collider: Collider,
    /// Counts consecutive frames where significant geometry penetration is detected.
    pub stuck_frames: u32,
    pub last_stuck_position: Vec3,
    /// The vertical offset from the character's root transform to the center
    /// of the collision shape. This is needed for vanilla-accurate physics.
    pub vertical_offset: f32,
}

impl Default for CreaturePhysicsState {
    fn default() -> Self {
        Self {
            velocity_for_frame: Vec3::ZERO,
            inertia: Vec3::ZERO,
            on_ground: true, // Start assuming on the ground
            on_slope: false,
            last_slide_normal: Vec3::ZERO,
            last_slide_normal_fallback: Vec3::ZERO,
            times_slid: 0,
            collider: Collider::default(),
            stuck_frames: 0,
            last_stuck_position: Vec3::ZERO,
            vertical_offset: 0.0,
        }
    }
}
