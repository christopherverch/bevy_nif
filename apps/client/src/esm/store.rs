use bevy::prelude::*;
use esm_parser_lib::{
    RefId, StringRefId, Weapon,
    esm_record::EsmRecord,
    find_interned,
    types::{
        bodypart::Bodypart, creature::Creature, gmst::GameSetting, magic_effect::MagicEffect,
        npc::Npc, race::Race, skill::Skill, spell::Spell,
    },
};
use unordered_map::UnorderedMap;

#[derive(Resource, Default)]
pub struct EsmStore {
    pub weapons: Store<Weapon>,
    pub races: Store<Race>,
    pub bodyparts: Store<Bodypart>,
    pub npcs: Store<Npc>,
    pub spells: Store<Spell>,
    pub magic_effects: Store<MagicEffect>,
    pub gamesettings: Store<GameSetting>,
    pub creatures: Store<Creature>,
    pub skills: Store<Skill>,
}

// SAFETY: We only populate these stores during loading, never during gameplay
// The pointers are stable because they point into an unordered_map, which never moves the memory.
unsafe impl<T: EsmRecord> Send for Store<T> {}
unsafe impl<T: EsmRecord> Sync for Store<T> {}

#[derive(Debug)]
pub struct Store<T: EsmRecord> {
    pub static_rec: UnorderedMap<RefId, T>,

    pub shared: Vec<*const T>,

    // For save-games/other dynamic loading
    pub dynamic: UnorderedMap<RefId, T>,
}

impl<T: EsmRecord> Default for Store<T> {
    fn default() -> Self {
        Self {
            static_rec: UnorderedMap::default(),
            shared: Vec::new(),
            dynamic: UnorderedMap::default(),
        }
    }
}

impl<T: EsmRecord> Store<T> {
    pub fn get_raw(&self, id: &RefId) -> Option<*const T> {
        self.dynamic
            .get_unsafe(id)
            .or_else(|| self.static_rec.get_unsafe(id))
    }

    /// Public use: returns a safe reference for game systems
    pub fn get(&self, id: &RefId) -> Option<&T> {
        self.get_raw(id).map(|ptr| unsafe {
            // SAFETY: We guarantee that during gameplay,
            // these records are immutable and the pointers are stable.
            &*ptr
        })
    }
    /// Searches dynamic overrides and static records, returning an Option containing
    /// a raw pointer. Safe because nodes are frozen during gameplay
    pub fn search_raw(&self, id: &str) -> Option<*const T> {
        let interned = find_interned(id)?;
        let search_key = RefId::SizedString(StringRefId(interned));
        self.get_raw(&search_key)
    }

    pub fn search(&self, id: &str) -> Option<&T> {
        let interned = find_interned(id)?;
        let search_key = RefId::SizedString(StringRefId(interned));
        self.get(&search_key)
    }

    /// Adds a record to the static store during loading phases, Should never be used during
    /// active gameplay
    pub fn insert_static(&mut self, id: RefId, record: T) {
        // Because it's a UnorderedMap, this memory slot is permanent.
        // Only insert into shared if this wasn't an update on an existing record
        if self.static_rec.insert_unsafe(id.clone(), record) == true {
            let ref_to_inserted = self.static_rec.get_unsafe(&id).unwrap();
            let ptr = ref_to_inserted as *const T;
            self.shared.push(ptr);
        }
    }
    /// Purges a record from the static tree map if a plugin requests a deletion
    pub fn remove_static(&mut self, id: &RefId) {
        self.static_rec.remove_unsafe(id);
        // TODO:: self.shared.remove(id);
    }

    /// Adds/Overrides a record at runtime (for Save Games or script modifications)
    /// Returns a raw pointer to the dynamic override record.
    pub fn insert_dynamic(&mut self, id: RefId, record: T) -> *const T {
        self.dynamic.insert_unsafe(id.clone(), record);

        // Grab the reference back out
        let ref_to_inserted = self.dynamic.get_unsafe(&id).unwrap();

        // Return the stable pointer to this dynamic instance
        ref_to_inserted as *const T
    }
    pub fn remove_dynamic(&mut self, id: &RefId) {
        self.dynamic.remove_unsafe(id);
        // TODO:: self.shared.remove(id);
    }

    pub fn iter_shared(&self) -> impl Iterator<Item = &T> {
        self.shared.iter().map(|&ptr| unsafe {
            // SAFETY: We assume these pointers were verified during
            // registration and the records have not been removed.
            &*ptr
        })
    }
}

impl Store<GameSetting> {
    pub fn find(&self, str: &str) -> &GameSetting {
        self.search(str)
            .expect(&format!("\"{}\" not found in gamesettings", str))
    }
}
