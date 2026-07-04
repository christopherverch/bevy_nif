use esm_parser_lib::{
    HasSpells, RefId,
    esm_record::EsmRecord,
    types::{creature::Creature, enums::RecInt, npc::Npc},
};
use esm_store::{EsmStore, GetStoreMut};

use crate::mechanics::spells::Spells;

/// Multiple instances of the same actor share the same spell list in Morrowind.
/// The most obvious result of this is that adding a spell or ability to one instance adds it to all instances.
/// @note The original game will only update visual effects associated with any added abilities for the originally
/// targeted actor,
///       changing cells applies the update to all actors.
/// Aside from sharing the same active spell list, changes made to this list are also written to the actor's base
/// record. Interestingly, it is not just scripted changes that are persisted to the base record. Curing one
/// instance's disease will cure all instances.
/// @note The original game is inconsistent in persisting this example;
///       saving and loading the game might reapply the cured disease depending on which instance was cured.
pub struct SpellList {
    id: RefId,
    spell_type: i32,
    listeners: Vec<Spells>,
}
impl SpellList {
    pub fn with_base_record(&self, esm_store: &mut EsmStore, f: fn(&Vec<RefId>) -> bool) -> bool {
        let Ok(rec) = RecInt::try_from(self.spell_type as u32) else {
            return false;
        };
        match rec {
            RecInt::Crea => with_base_record::<Creature>(&self.id, esm_store, f),
            RecInt::Npc_ => with_base_record::<Npc>(&self.id, esm_store, f),
            _ => panic!("failed to update base record for {}", self.id),
        }
    }
}
fn with_base_record<T>(id: &RefId, esm_store: &mut EsmStore, f: fn(&Vec<RefId>) -> bool) -> bool
where
    T: EsmRecord + HasSpells + Clone,
    EsmStore: GetStoreMut<T>,
{
    let copy = esm_store.find::<T>(id).cloned();
    let changed = copy
        .as_ref()
        .map_or(false, |record| f(&record.spells().list));
    if changed {
        // Can unwrap because otherwise changed would always be false
        esm_store.override_record(&copy.unwrap());
    }

    changed
}
