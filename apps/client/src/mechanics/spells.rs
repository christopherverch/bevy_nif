use std::collections::HashMap;

use esm_parser_lib::{RefId, types::spell::Spell};
use esm_store::EsmStore;

use crate::{live_cell_ref::EsmPtr, world::timestamp::TimeStamp};

#[derive(Debug, Clone, Default)]
pub struct Spells {
    /// A reference to the spell list this instance's spells are based off of,
    /// i.e. the default spells for this race/class/creature, or whatever instance it's based off of
    pub spell_list_ref: RefId,
    /// Note: this is the spell that's about to be cast, *not* the spell selected in the GUI (which may be different)
    pub selected_spell: Option<RefId>,
    /// pointers to the actual esm data for the spells this instance has
    pub spells: Vec<EsmPtr<Spell>>,

    /// Tracks when powers were last used
    pub used_powers: HashMap<RefId, TimeStamp>,
}
impl Spells {
    pub fn new() -> Self {
        Self {
            spell_list_ref: RefId::default(),
            selected_spell: None,
            used_powers: HashMap::new(),
            spells: Vec::new(),
        }
    }

    pub fn has_spell_id(&self, spell_id: &RefId, esm_store: &EsmStore) -> bool {
        let spell_opt = esm_store.find::<Spell>(spell_id);
        let Some(spell) = spell_opt else {
            panic!("failed to find {} in esmstore", spell_id)
        };
        self.has_spell(spell)
    }
    pub fn has_spell(&self, spell: &Spell) -> bool {
        for my_spell in &self.spells {
            if my_spell.0 == spell as *const Spell {
                return true;
            }
        }
        false
    }

    // Adding a spell
    pub fn add_spell(&mut self, spell_id: RefId) {}

    // Removing a spell
    pub fn remove(&mut self, spell_id: RefId) {}

    // The "Purge" logic: Use a closure to filter
    // This replaces the C++ "SpellFilter" function pointer approach
    pub fn purge<F>(&mut self, mut filter: F)
    where
        F: FnMut(&RefId) -> bool,
    {
    }
    pub fn clear(&mut self) {}
}
