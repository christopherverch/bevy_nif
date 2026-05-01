use bevy::prelude::*;
use esm_parser_lib::{
    RefId, StringRefId, Weapon,
    esm_record::EsmRecord,
    find_interned,
    types::{
        bodypart::Bodypart, gmst::GameSetting, magic_effect::MagicEffect, npc::Npc, race::Race,
        skill::Skill, spell::Spell,
    },
};
use std::collections::{HashMap, hash_map::Entry};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    Npc,
    Spell,
    Item,
    Cell,
    // ... all 40+ Morrowind record types
}
#[derive(Resource)]
pub struct EsmStore {
    pub weapons: Store<Weapon>,
    pub races: Store<Race>,
    pub bodyparts: Store<Bodypart>,
    pub npcs: Store<Npc>,
    pub spells: Store<Spell>,
    pub magic_effects: Store<MagicEffect>,
    pub gamesettings: Store<GameSetting>,
    pub skills: Store<Skill>,
    // A master map for the global "find"
    pub id_to_type: HashMap<RefId, RecordType>,
}

pub struct Store<T: EsmRecord> {
    pub records: Vec<T>,
    indices: HashMap<RefId, usize>,
}
impl<T: EsmRecord> Store<T> {
    pub fn search(&self, id: &str) -> Option<&T> {
        // 1. Convert &str to interned pointer
        dbg!(id);
        let interned = find_interned(id)?;

        // 2. Wrap in RefId (Note: You may need to check UnsizedString too
        // if your ESM parser uses both, but usually SizedString is the standard)
        let search_key = RefId::SizedString(StringRefId(interned));
        println!(
            "Searching for: {} | Generated Hash: 0x{:X}",
            id,
            search_key.debug_hash()
        );
        // 3. O(1) Lookup
        let index = self.indices.get(&search_key)?;

        Some(&self.records[*index])
    }
}
impl EsmStore {
    pub fn find_type(&self, id: &RefId) -> Option<RecordType> {
        self.id_to_type.get(id).copied()
    }
}
impl<T: EsmRecord> Store<T> {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            indices: HashMap::new(),
        }
    }
    pub fn get(&self, id: &RefId) -> Option<&T> {
        self.indices.get(id).map(|&i| &self.records[i])
    }
    pub fn insert(&mut self, id: RefId, record: T) {
        match self.indices.entry(id) {
            Entry::Occupied(entry) => {
                // id exists, overwrite the old data in the vector at that index
                let index = *entry.get();
                self.records[index] = record;
            }
            Entry::Vacant(entry) => {
                // new id, push to the end and record the new index
                let index = self.records.len();
                self.records.push(record);
                entry.insert(index);
            }
        }
    }
    pub fn remove(&mut self, id: &RefId) {
        if let Some(index) = self.indices.remove(id) {
            // 1. Remove the record from the vector
            // This moves the last element into 'index'
            self.records.swap_remove(index);

            // 2. If the vector wasn't empty and we didn't just remove the last element,
            // we need to update the index of the element that was moved.
            if index < self.records.len() {
                // We need to find which ID belongs to the moved record.
                // TODO:: get the refid from the record itself, maybe with a trait?
                let moved_record_id = self.records[index].id();
                self.indices.insert(moved_record_id.clone(), index);
            }
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.records.iter()
    }
}
