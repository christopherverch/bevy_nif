use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use cdda_bevy::client::defs::{
    LandscapeCell, LandscapeLoader, LandscapeManagementSettings, LocalPlayer,
};

use cdda_bevy::client::const_defs::MORROWIND_CELL_SIZE;
pub fn spawn_despawn_landscapes(
    player_transform: Single<&Transform, With<LocalPlayer>>,
    landscapes_query: Query<(Entity, &LandscapeCell)>,
    settings: Res<LandscapeManagementSettings>,
    mut commands: Commands,
    mut old_player_cell: Local<Option<(i32, i32)>>,
) {
    // have to negate x because morrowind is left-handed coordinate system
    let current_cell_x = (-player_transform.translation.x / MORROWIND_CELL_SIZE).floor() as i32;
    let current_cell_y = (player_transform.translation.z / MORROWIND_CELL_SIZE).floor() as i32;
    let current_player_cell_coords = (current_cell_x, current_cell_y);
    if *old_player_cell == Some(current_player_cell_coords) {
        return; // Player hasn't changed cells
    }
    *old_player_cell = Some((current_cell_x, current_cell_y));
    let mut desired_cells: HashSet<(i32, i32)> = HashSet::new();
    for dx in -(settings.load_radius_cells as i32)..=(settings.load_radius_cells as i32) {
        for dy in -(settings.load_radius_cells as i32)..=(settings.load_radius_cells as i32) {
            desired_cells.insert((current_cell_x + dx, current_cell_y + dy));
        }
    }
    let mut currently_loaded_cells = HashMap::new();
    for (landscape_entity, landscape) in landscapes_query {
        currently_loaded_cells.insert((landscape.x, landscape.y), landscape_entity);
    }
    for desired_cell in &desired_cells {
        if !currently_loaded_cells.contains_key(&desired_cell) {
            //if one isn't loaded:
            commands.spawn((
                LandscapeLoader {
                    cell_x: desired_cell.0,
                    cell_y: desired_cell.1,
                },
                LandscapeCell {
                    // Add this immediately for next frame's query
                    x: desired_cell.0,
                    y: desired_cell.1,
                },
                Visibility::Visible,
            ));
        }
    }
    for (currently_loaded_cell, landscape_entity) in currently_loaded_cells {
        let dist_x = (currently_loaded_cell.0 - current_cell_x).abs();
        let dist_y = (currently_loaded_cell.1 - current_cell_y).abs();
        let unload_radius = settings.unload_radius_cells as i32;
        if dist_x > unload_radius || dist_y > unload_radius {
            commands.entity(landscape_entity).despawn();
        }
    }
}
