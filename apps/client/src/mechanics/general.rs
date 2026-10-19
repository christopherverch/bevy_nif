use bevy::prelude::*;
use esm_parser_lib::{RefId, types::npc::Npc};
use esm_store::EsmStore;

use crate::{
    defs::LocalPlayer,
    esm_ptr::EsmPtr,
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
    let player = unsafe { player_cell_ref.base.extract_unchecked() };
    unsafe {
        update_player_visuals(player, new_race_info, &mut esm_store);
    }
    commands.run_system_cached(build_player);
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

pub struct SetPlayerRaceInfo {
    race: RefId,
    male: bool,
    head: RefId,
    hair: RefId,
}

pub unsafe fn update_player_visuals(
    ptr: EsmPtr<Npc>,
    new_race_info: SetPlayerRaceInfo,
    esm_store: &mut EsmStore,
) {
    let player = ptr.as_mut(esm_store);

    // Only update if something changed (prevents unnecessary re-inserts)
    if player.race != new_race_info.race
        || player.head != new_race_info.head
        || player.hair != new_race_info.hair
        || player.is_male() != new_race_info.male
    {
        player.race = new_race_info.race;
        player.head = new_race_info.head;
        player.hair = new_race_info.hair;
        player.set_is_male(new_race_info.male);
    }
}
