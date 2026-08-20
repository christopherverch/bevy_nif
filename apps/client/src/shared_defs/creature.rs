use esm_parser_lib::{
    RefId,
    types::{creature::Creature, enums::Specialization},
};
use esm_store::EsmStore;

use crate::live_cell_ref::{EsmPtr, LiveCellRef};

pub fn get_skill(esm_store: &EsmStore, skill: RefId, creature_ptr: EsmPtr<Creature>) -> f32 {
    let skill_record = esm_store.skills.get(&skill).unwrap();
    let skill_level = match skill_record.data.specialization {
        Specialization::Combat => creature_ptr.data.combat,
        Specialization::Magic => creature_ptr.data.magic,
        Specialization::Stealth => creature_ptr.data.magic,
    };
    skill_level as f32
}
