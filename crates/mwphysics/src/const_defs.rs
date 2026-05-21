pub const SOLVER_GROUND_OFFSET: f32 = 1.0 / MW_UNITS_PER_METER;
pub const MW_UNITS_PER_METER: f32 = 69.99125109;
pub const SOLVER_LARGE_DISTANCE: f32 = 35.0 / MW_UNITS_PER_METER * 35.0 / MW_UNITS_PER_METER;

// Arbitrary number. To prevent infinite loops. They shouldn't happen but it's good to be prepared.
pub const MAX_ITERATIONS: u8 = 8;

// Allows for more precise movement solving without getting stuck or snagging too easily.
pub const COLLISION_MARGIN: f32 = 0.2 / MW_UNITS_PER_METER;

// Allow for a small amount of penetration to prevent numerical precision issues from causing the "unstuck"ing code
// to run unnecessarily Currently set to 0 because having the "unstuck"ing code run whenever possible prevents some
// glitchy snagging issues
pub const ALLOWED_PENETRATION: f32 = 0.0;
