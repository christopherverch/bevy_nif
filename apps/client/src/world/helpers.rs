use crate::defs::{MWCell, WorldSettings};
use bevy::prelude::*;

pub fn is_swimming(
    pos: Vec3,
    world_settings: &WorldSettings,
    rendering_half_extents_y_opt: Option<f32>,
    current_cell: &MWCell,
) -> bool {
    let rendering_half_extents_y = rendering_half_extents_y_opt.unwrap_or(0.0);
    is_underwater_f32(
        pos,
        world_settings.swim_height_scale,
        rendering_half_extents_y,
        current_cell,
    )
}
pub fn is_underwater_f32(
    mut pos: Vec3,
    height_ratio: f32,
    rendering_half_extents_y: f32,
    current_cell: &MWCell,
) -> bool {
    pos.y += height_ratio * 2.0 * rendering_half_extents_y;
    if !current_cell.has_water {
        return false;
    }
    return pos.y < current_cell.water_level;
}
