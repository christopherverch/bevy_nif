use bevy::prelude::*;
use esm_store::{EsmStore, SetPlayerRaceInfo};

use crate::{
    defs::LocalPlayer,
    live_cell_ref::{CellRefBase, LiveCellRef},
    mechanics::{creature_stats::CreatureStats, npc_stats::NpcStats},
};

pub fn set_player_race(
    In(new_race_info): In<SetPlayerRaceInfo>,
    player: Single<(&mut LiveCellRef, &CreatureStats, &NpcStats), With<LocalPlayer>>,
    mut esm_store: ResMut<EsmStore>,
    mut commands: Commands,
) {
    let (player_cell_ref, _player_creature_stats, _player_npc_stats) = player.into_inner();
    let player = match &player_cell_ref.base {
        CellRefBase::Npc(player) => player,
        _ => return,
    };
    esm_store
        .npcs
        .update_player_visuals(player.0, new_race_info);
    //mraceselected=true
    commands.run_system_cached(build_player);
    ////mupdateplayer = true
}
fn build_player(
    player: Single<(&LiveCellRef, &mut CreatureStats, &NpcStats), With<LocalPlayer>>,
    esm_store: Res<EsmStore>,
) {
    let (player_cell_ref, mut creature_stats, _npc_stats) = player.into_inner();
    let player = match &player_cell_ref.base {
        CellRefBase::Npc(player) => player,
        _ => return,
    };
    creature_stats.recalculate_magicka(true, &esm_store.gamesettings);
    creature_stats.level = player.data.level;
    creature_stats.spells.clear();
}
