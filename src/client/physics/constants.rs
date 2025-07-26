// src/physics/constants.rs

//! This module contains the exact physics constants ported from the OpenMW C++
//! source code.

use crate::const_defs::UNITS_PER_METER;

pub const GRAVITY_CONST: f32 = 8.96;
// The steepest slope, in degrees, that is considered "walkable ground".
pub const MAX_SLOPE_DEGREES: f32 = 46.0;

/// The maximum number of iterations the movement solver will run in a single frame
/// to resolve collisions.
pub const MAX_ITERATIONS: u8 = 8;

// --- Collision Margins ---
/// A small distance to keep the character from surfaces to prevent snagging.
pub const COLLISION_MARGIN: f32 = 0.2 / UNITS_PER_METER;
/// The amount of penetration allowed before the "unstuck" logic kicks in.
/// Setting this to 0 means the unstuck code runs on any penetration.
pub const ALLOWED_PENETRATION: f32 = 0.0;

// --- Ground and Stepping ---
/// The ideal distance to keep the character's collision shape above the ground.
pub const GROUND_OFFSET: f32 = 1.0 / UNITS_PER_METER;
/// The maximum distance the character can "fall" downwards when trying to step
/// onto a new surface.
pub const STEP_SIZE_DOWN: f32 = 62.0 / UNITS_PER_METER;
/// The maximum height the character can step up onto.
pub const STEP_SIZE_UP: f32 = 34.0 / UNITS_PER_METER;
// --- Stair Stepping Hacks ---
/// These are for working around bad Morrowind assets. Disabling them improves
/// performance but breaks movement in some areas.
pub const DO_EXTRA_STAIR_HACKS: bool = true;
/// A fixed-distance forward move used in the first stair-stepping hack.
pub const S_MIN_STEP: f32 = 10.0 / UNITS_PER_METER;
/// A further, less-tall fixed-distance move for the second hack.
pub const S_MIN_STEP_2: f32 = 20.0 / UNITS_PER_METER;
// A final, last-resort vertical distance to push the character upwards if it's severely stuck.
pub const FAILSAFE_UNSTUCK_UPWARD_DISTANCE: f32 = 10.0 / UNITS_PER_METER;
