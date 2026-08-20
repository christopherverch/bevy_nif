use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use common::stat::SkillValue;
use esm_parser_lib::RefId;

#[derive(Component, Clone, Debug, Default)]
pub struct NpcStats {
    // Basic shared stats
    pub disposition: i32,
    pub crime_disposition_modifier: i32,
    pub skills: HashMap<RefId, SkillValue>,
    pub reputation: i32,
    pub crime_id: i32,

    // Faction/Rank logic (Applies to ranked NPCs + Player)
    pub faction_rank: HashMap<RefId, i32>,
    pub expelled: HashSet<RefId>,
    pub faction_reputation: HashMap<RefId, i32>,

    // Leveling logic (Applies to ranked NPCs + Player)
    pub level_progress: i32,
    pub skill_increases: HashMap<RefId, i32>,
    pub spec_increases: Vec<i32>,
    pub used_ids: HashSet<RefId>,

    // State
    pub time_to_start_drowning: f32,
    is_werewolf: bool,

    // Player-only stats
    pub bounty: i32,
    werewolf_kills: i32,
}
impl NpcStats {
    pub fn set_werewolf(&mut self, set: bool) {
        if self.is_werewolf == set {
            return;
        }
        if set != false {
            self.werewolf_kills = 0;
        }
        self.is_werewolf = set;
    }
    pub fn is_werewolf(&self) -> bool {
        self.is_werewolf
    }
}
