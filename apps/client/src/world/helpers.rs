use crate::defs::Cell;
use bevy::prelude::*;

pub fn is_swimming(
    pos: Vec3,
    f_swim_height_scale: f32,
    rendering_half_extents_y: f32,
    current_cell: &Cell,
) -> bool {
    is_underwater(
        pos,
        f_swim_height_scale,
        rendering_half_extents_y,
        current_cell,
    )
}
pub fn is_underwater(
    mut pos: Vec3,
    height_ratio: f32,
    rendering_half_extents_y: f32,
    current_cell: &Cell,
) -> bool {
    pos.y += height_ratio * 2.0 * rendering_half_extents_y;
    if !current_cell.has_water {
        return false;
    }
    return pos.y < current_cell.water_level;
}
