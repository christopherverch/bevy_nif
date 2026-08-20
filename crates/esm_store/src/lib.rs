use bevy_ecs::resource::Resource;
use esm_parser_lib::{
    GeneratedRefId, RefId, StringRefId, Weapon,
    esm_record::EsmRecord,
    find_interned,
    types::{
        bodypart::Bodypart, cell::Cell, creature::Creature, enums::RecInt, gmst::GameSetting,
        magic_effect::MagicEffect, npc::Npc, race::Race, skill::Skill, spell::Spell,
        static_object::Static,
    },
};
use unordered_map::UnorderedMap;
/// Defines if the store can have values overwritten

#[derive(Resource, Default)]
pub struct EsmStore {
    /// Finds which store to look in for this record type
    /// maps refid -> record name (bytes), i.e. daedric_longsword -> WEAP (in 4 ascii bytes)
    pub ids: UnorderedMap<RefId, RecInt>,
    /// same as ids, just for specifically static records
    pub static_ids: UnorderedMap<RefId, u32>,
    pub cells: Store<Cell>,
    pub statics: Store<Static>,
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
impl EsmStore {
    pub fn find<T>(&self, refid: &RefId) -> Option<&T> {
        let Some(type_tag) = self.ids.get(refid) else {
            return None;
        };
        self.get_by_type_tag(*type_tag, refid)
    }
    pub fn find_static(&self, refid: &RefId) -> u32 {
        *self.static_ids.get(refid).unwrap_or(&0)
    }

    /// Insert a custom record (i.e. with a generated ID that will not clash will pre-existing records)
    /// returns a pointer to the created record
    pub fn insert<T>(&mut self, to_insert: T) -> *const T
    where
        T: EsmRecord + MaybeDynamic + Clone,
        Self: GetStoreMut<T>,
    {
        let store = self.get_writable();
        let id = store.generate_id();
        if store.search(&id.resolve_string()).is_some() {
            panic!("Try to override existing record: {}", id);
        }
        // fine to unwrap because insert would only return None if this wasn't new,
        // which we just checked it was.
        let ptr = store.insert(&to_insert as *const T, false).unwrap();
        if T::is_dynamic() {
            // unwrap is ok because we create RECORD_ID from the RecInt enum itself.
            self.set_id_type(to_insert.id(), T::RECORD_ID.try_into().unwrap());
        }
        ptr
    }

    pub fn get_by_type_tag<T>(&self, type_tag: RecInt, id: &RefId) -> Option<&T> {
        // The ugliness of the unsafe is just to have it map to a T, so we can make a generic
        // function. Maybe make this a macro instead?
        match type_tag {
            RecInt::Crea => {
                // Explicitly map to Option<&T>
                self.creatures
                    .get(id)
                    .map(|val| unsafe { &*(val as *const Creature as *const T) })
            }
            RecInt::Npc_ => self
                .npcs
                .get(id)
                .map(|val| unsafe { &*(val as *const Npc as *const T) }),
            _ => None,
        }
    }
    /// Insert a record with set ID, and allow it to override a pre-existing static record.
    pub fn override_record<T>(&mut self, record: &T) -> *const T
    where
        T: EsmRecord + Clone,
        Self: GetStoreMut<T>, // The compiler uses this to find the right field
    {
        let store = self.get_writable();

        // Can only fail with override_only = true, so just unwrap
        let ptr = store.insert(&record.clone() as *const T, false).unwrap();

        ptr
    }
    pub fn set_id_type(&mut self, id: &RefId, type_tag: RecInt) {
        self.ids.insert(id.clone(), type_tag);
    }
}
pub trait MaybeDynamic {
    fn is_dynamic() -> bool {
        false
    }
}
impl<T: EsmRecord> MaybeDynamic for Store<T> {}
impl MaybeDynamic for Npc {
    fn is_dynamic() -> bool {
        true
    }
}
impl MaybeDynamic for Creature {
    fn is_dynamic() -> bool {
        true
    }
}
pub trait GetStoreMut<T: EsmRecord> {
    fn get_writable(&mut self) -> &mut Store<T>;
}
macro_rules! register_stores {
    ($($t:ty => $field:ident),* $(,)?) => {
        $(
            impl GetStoreMut<$t> for EsmStore {
                fn get_writable(&mut self) -> &mut Store<$t> {
                    &mut self.$field
                }
            }
        )*
    };
}
register_stores!(
    Creature => creatures,
    Npc => npcs,
    Weapon => weapons,
);

// SAFETY: We only populate these stores in systems that use ResMut on the EsmStore, which manages
// the multithreaded safety for us.
// The pointers are stable because they point into an unordered map, which never moves the memory.
unsafe impl<T: EsmRecord> Send for Store<T> {}
unsafe impl<T: EsmRecord> Sync for Store<T> {}

#[derive(Debug)]
pub struct Store<T: EsmRecord> {
    // Loaded during initial game startup
    pub static_rec: UnorderedMap<RefId, T>,
    pub shared: Vec<*const T>,

    // For save-games/other dynamic loading
    pub dynamic: UnorderedMap<RefId, T>,
    /// How many dynamic records we've created so far
    dynamic_count: u64,
}

impl<T: EsmRecord> Default for Store<T> {
    fn default() -> Self {
        Self {
            static_rec: UnorderedMap::default(),
            shared: Vec::new(),
            dynamic: UnorderedMap::default(),
            dynamic_count: 0,
        }
    }
}

impl<T: EsmRecord> Store<T> {
    pub fn get_raw(&self, id: &RefId) -> Option<*const T> {
        self.dynamic
            .get(id)
            .or_else(|| self.static_rec.get(id))
            .map(|t| t as *const T)
    }
    pub fn get_raw_mut(&mut self, id: &RefId) -> Option<*mut T> {
        self.dynamic
            .get_mut(id)
            .or_else(|| self.static_rec.get_mut(id))
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
    pub fn insert(&mut self, item_ptr: *const T, mut override_only: bool) -> Option<*const T>
    where
        T: Clone,
    {
        let item = unsafe { &*item_ptr };
        let id = item.id();
        let generated = matches!(id, RefId::Generated(_));
        override_only = override_only && !generated;

        // if override_only is true, we can only override existing records,
        // and if it's not found we don't insert anything
        if override_only {
            // If it doesn't exist in static, we ignore the insertion
            if !self.static_rec.contains_key(&id) {
                return None;
            }
        }

        let (is_new, ptr) = self.dynamic.insert_or_assign(id.clone(), item.clone());

        // Update shared if it's a new entry
        if is_new {
            self.shared.push(ptr);
        }

        Some(ptr)
    }
    fn generate_id(&mut self) -> RefId {
        let tmp = self.dynamic_count;
        self.dynamic_count += 1;
        return RefId::Generated(GeneratedRefId::new(tmp));
    }

    /// Adds a record to the static store during loading phases, Should never be used during
    /// active gameplay
    pub fn insert_static(&mut self, id: RefId, record: T) {
        // Because it's a UnorderedMap, this memory slot is permanent.
        // Only insert into shared if this wasn't an update on an existing record
        if self.static_rec.insert(id.clone(), record) == true {
            let ref_to_inserted = self.static_rec.get(&id).unwrap();
            let ptr = ref_to_inserted as *const T;
            self.shared.push(ptr);
        }
    }
    /// Purges a record from the static tree map if a plugin requests a deletion
    pub fn remove_static(&mut self, id: &RefId) {
        self.static_rec.remove(id);
        // TODO:: self.shared.remove(id);
    }

    /// Adds/Overrides a record at runtime (for Save Games or script modifications)
    /// Returns a raw pointer to the dynamic override record.
    pub fn insert_dynamic(&mut self, id: RefId, record: T) -> *const T {
        self.dynamic.insert(id.clone(), record);

        // Get the reference to the thing we just inserted
        let ref_to_inserted = self.dynamic.get(&id).unwrap();

        // Return the stable pointer to this dynamic instance
        ref_to_inserted as *const T
    }
    pub fn remove_dynamic(&mut self, id: &RefId) {
        self.dynamic.remove(id);
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

pub struct SetPlayerRaceInfo {
    race: RefId,
    male: bool,
    head: RefId,
    hair: RefId,
}
impl Store<Npc> {
    /// Returns true if the state actually changed
    pub fn update_player_visuals(
        &mut self,
        ptr: *const Npc,
        new_race_info: SetPlayerRaceInfo,
    ) -> bool {
        let player = unsafe { &mut *(ptr as *mut Npc) };

        // Only update if something changed (prevents unnecessary re-inserts)
        if player.race != new_race_info.race
            || player.head != new_race_info.head
            || player.hair != new_race_info.hair
            || player.is_male() != new_race_info.male
        {
            return false;
        }

        player.race = new_race_info.race;
        player.head = new_race_info.head;
        player.hair = new_race_info.hair;
        player.set_is_male(new_race_info.male);

        return true;
    }
}

impl Store<GameSetting> {
    pub fn find(&self, str: &str) -> &GameSetting {
        self.search(str)
            .expect(&format!("\"{}\" not found in gamesettings", str))
    }
}
