use std::collections::HashMap;

use crate::{
    defs::{DrawState, IdToEntityMap, Player, Stance},
    magic_effects::{EffectKey, MagicEffects},
    mechanics::spells::Spells,
    stat::{AttributeValue, DynamicStat},
};
use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bitflags::bitflags;
use common::EntityId;
use esm_parser_lib::{
    RefId,
    types::{attribute::Attribute, gmst::GameSetting, magic_effect::MagicEffect},
};
use esm_store::Store;

#[derive(Component, Clone, Debug)]
#[component(on_add = add_creature_to_id_map)]
pub struct CreatureStats {
    pub entity_id: EntityId,
    pub attributes: HashMap<RefId, AttributeValue>,
    pub level: i16,
    pub spells: Spells,
    pub nif_animator_link: Option<Entity>,
    pub stats: DynamicStats,
    pub magic_effects: MagicEffects,
    pub draw_state: DrawState,
    pub movement_flags: CreatureStatsFlags,
    pub fall_height: f32,
    /// Updated every frame before other systems that use it run
    pub max_speed: f32,
    pub side_movement_angle: f32,
    pub knocked_down: bool,
    pub death_animation_finished: bool,
    pub block: bool,
    pub dead: bool,
    pub hit_recovery: bool,
    pub teleported: bool,
}
impl CreatureStats {
    fn health(&self) -> &DynamicStat<f32> {
        &self.stats.health
    }
    fn set_health(&mut self, value: f32) {
        self.stats.health.set_current(value, false, false);
    }
    pub fn recalculate_magicka(&mut self, is_player: bool, game_settings: &Store<GameSetting>) {
        let intelligence = self
            .attributes
            .get(&Attribute::INTELLIGENCE)
            .unwrap()
            .get_modified();
        let base = if is_player {
            game_settings.find("fPCbaseMagickaMult").value.0.as_float()
        } else {
            game_settings.find("fNPCbaseMagickaMult").value.0.as_float()
        };
        let magicka_factor = base
            + self
                .magic_effects
                .get_or_default_by_key(&EffectKey {
                    id: MagicEffect::FORTIFYMAXIMUMMAGICKA,
                    arg: None,
                })
                .get_magnitude()
                * 0.1;
        let magicka = &mut self.stats.magicka;
        let current_to_base_ratio = if magicka.current > 0.0 {
            magicka.current / magicka.base
        } else {
            0.0
        };
        magicka.base = magicka_factor * intelligence;
        magicka.set_current(magicka.base * current_to_base_ratio, false, true);
    }
    pub fn land(&mut self, player_opt: Option<&mut Player>) -> f32 {
        if let Some(player) = player_opt {
            player.jumping = false;
        }
        let height = self.fall_height;
        self.fall_height = 0.0;
        return height;
    }
    pub fn is_stance_active(&self, stance: Stance) -> bool {
        match stance {
            Stance::Run => self
                .movement_flags
                .intersects(CreatureStatsFlags::RUN | CreatureStatsFlags::FORCE_RUN),
            Stance::Sneak => self
                .movement_flags
                .intersects(CreatureStatsFlags::SNEAK | CreatureStatsFlags::FORCE_SNEAK),
        }
    }
    pub fn set_movement_flag(&mut self, flag: CreatureStatsFlags, state: bool) {
        if state {
            self.movement_flags |= flag;
        } else {
            self.movement_flags &= !flag;
        }
    }
}
bitflags! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CreatureStatsFlags: u8 {
    const FORCE_RUN       = 1 << 0; // 1
    const FORCE_SNEAK     = 1 << 1; // 2
    const RUN             = 1 << 2; // 4
    const SNEAK           = 1 << 3; // 8
    const FORCE_JUMP      = 1 << 4; // 16
    const FORCE_MOVE_JUMP = 1 << 5; // 32
}}
// add this creature's id to the id->Entity hashmap, so we can get the entity by its id
fn add_creature_to_id_map(
    mut world: DeferredWorld,
    HookContext {
        entity: creature_entity,
        ..
    }: HookContext,
) {
    let entity_id_opt = world
        .get::<CreatureStats>(creature_entity)
        .map(|stats| stats.entity_id);

    if let Some(actual_id) = entity_id_opt {
        world
            .resource_mut::<IdToEntityMap>()
            .0
            .insert(actual_id, creature_entity);
    } else {
        panic!(
            "CreatureStats not found on entity {:?} for IdToEntityMap insertion",
            creature_entity
        );
    }
}
#[derive(Clone, Debug, Default)]
pub struct DynamicStats {
    // Health is private since setting it can cause outside changes, like the creature dying
    health: DynamicStat<f32>,
    pub magicka: DynamicStat<f32>,
    pub fatigue: DynamicStat<f32>,
}
