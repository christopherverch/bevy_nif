use std::collections::{HashMap, HashSet};

use crate::{
    const_defs::CELL_SIZE_METERS,
    defs::{ESM_READER, LandscapeManagementSettings, LocalPlayer},
    landscape::{ALL_LANDSCAPES, LandscapeCell, LandscapeLoader},
    setup_scene::spawn_nif_unattached,
};
use bevy::prelude::*;
use bevy_nif::{check_exists, clean_path_common, resolve_bevy_asset_path};
use esm_parser_lib::{
    ESM3ExteriorCellRefId, EsmReader, RefId,
    types::{cell::Cell, cell_ref::CellRef},
};
use esm_store::EsmStore;
#[derive(Component)]
pub struct LandscapeContainedEntities(pub Vec<Entity>);

pub fn spawn_despawn_landscapes(
    player_transform: Single<&Transform, With<LocalPlayer>>,
    landscapes_query: Query<
        (Entity, &LandscapeCell, &LandscapeContainedEntities),
        (With<LandscapeCell>, Without<LandscapeLoader>),
    >,
    already_loading_landscapes: Query<
        (Entity, &LandscapeLoader),
        (Without<LandscapeCell>, With<LandscapeLoader>),
    >,
    settings: Res<LandscapeManagementSettings>,
    store: Res<EsmStore>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut old_player_cell: Local<Option<(i32, i32)>>,
    mut last_player_pos: Local<Option<Vec2>>,
) {
    // So we don't have to do all the cell spawning and despawning checks
    // if the player hasn't moved much
    if let Some(last_player_pos) = *last_player_pos {
        if last_player_pos.distance_squared(player_transform.translation.xz()) < 2.0 {
            return;
        }
    }
    *last_player_pos = Some(player_transform.translation.xz());

    let Ok(landscapes) = ALL_LANDSCAPES.get().unwrap().try_read() else {
        return;
    };
    // have to negate x because morrowind is left-handed coordinate system
    let current_cell_x = (-player_transform.translation.x / CELL_SIZE_METERS).floor() as i32;
    let current_cell_y = (player_transform.translation.z / CELL_SIZE_METERS).floor() as i32;
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
    for (landscape_entity, landscape, contained_entities) in landscapes_query {
        currently_loaded_cells.insert(
            (landscape.x, landscape.y),
            (landscape_entity, contained_entities),
        );
    }
    let mut loading_cells = HashMap::new();
    for (landscape_entity, loading_cell) in already_loading_landscapes {
        loading_cells.insert((loading_cell.cell_x, loading_cell.cell_y), landscape_entity);
    }

    for desired_cell in &desired_cells {
        // Don't try to spawn cells that we have no data for
        if landscapes.get(desired_cell).is_none() {
            continue;
        }
        if !currently_loaded_cells.contains_key(&desired_cell)
            && !loading_cells.contains_key(&desired_cell)
        {
            let mut esm_reader = ESM_READER.get().unwrap().write().unwrap();
            let loaded_entities = load_setup_exterior(
                &store,
                &mut commands,
                &mut esm_reader,
                &asset_server,
                IVec2::new(desired_cell.0, desired_cell.1),
            );
            //if one isn't loaded:
            commands.spawn((
                LandscapeLoader {
                    cell_x: desired_cell.0,
                    cell_y: desired_cell.1,
                },
                LandscapeContainedEntities(loaded_entities),
                Visibility::Visible,
            ));
        }
    }
    for (currently_loaded_cell, (landscape_entity, contained_entities)) in currently_loaded_cells {
        let dist_x = (currently_loaded_cell.0 - current_cell_x).abs();
        let dist_y = (currently_loaded_cell.1 - current_cell_y).abs();
        let unload_radius = settings.unload_radius_cells as i32;
        if dist_x > unload_radius || dist_y > unload_radius {
            for entity in &contained_entities.0 {
                commands.entity(*entity).despawn();
            }
            commands.entity(landscape_entity).despawn();
        }
    }
}
fn load_setup_exterior(
    store: &EsmStore,
    commands: &mut Commands,
    esm_reader: &mut EsmReader,
    asset_server: &AssetServer,
    pos: IVec2,
) -> Vec<Entity> {
    let mut spawned_entities = Vec::new();
    let cell = store
        .cells
        .get(&RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(
            pos.x, pos.y,
        )))
        .unwrap();

    cell.restore(esm_reader, 0).unwrap();
    let mut cell_ref = CellRef::default();
    let mut is_deleted = false;
    while let Ok(has_ref) = Cell::get_next_ref(esm_reader, &mut cell_ref, &mut is_deleted) {
        if !has_ref {
            return spawned_entities;
        }
        if let Some(stat) = store.statics.get(&cell_ref.id) {
            let mut pos = cell_ref.pos.pos;
            if !stat.model.contains("nord") {
                continue;
            }
            let path = format!("data/Data Files/Meshes/{}", stat.model);
            let path = clean_path_common(&path);
            let path = if check_exists(&path) {
                path
            } else {
                resolve_bevy_asset_path(&path).unwrap()
            };
            let handle = asset_server.load(path);
            pos.x += CELL_SIZE_METERS / 2.0;
            pos.z -= CELL_SIZE_METERS / 2.0;
            let rot = cell_ref.pos.rot;
            let nif = spawn_nif_unattached(commands, handle, pos, rot);
            commands.entity(nif).insert(Name::new(stat.model.clone()));
            spawned_entities.push(nif);
        }
    }
    spawned_entities
}
