use crate::{
    EsmError, EsmReader, HasSpells, RefId,
    common_inventory_types::InventoryList,
    esm_record::EsmRecord,
    name4,
    types::{
        aipackage::{AIData, AIPackageList},
        enums::CreatureType,
        flags::{CreatureFlags, ObjectFlags},
        spell_list::SpellList,
        transport::Transport,
    },
};
use esm_parser_macros::LoadSave;

#[derive(LoadSave, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpdtStruct {
    pub creature_type: CreatureType,
    pub level: i32,
    pub attributes: [i32; 8], // AttributeId::Length is 8
    pub health: i32,
    pub mana: i32,
    pub fatigue: i32,
    pub soul: i32,
    pub combat: i32,
    pub magic: i32,
    pub stealth: i32,
    pub attack: [i32; 6],
    pub gold: i32,
}

#[derive(Debug, Clone)]
pub struct Creature {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub data: NpdtStruct,
    pub blood_type: i32,
    pub flags: CreatureFlags,
    pub scale: f32,
    pub script: RefId,
    pub model: String,
    pub name: String,
    pub original: RefId,

    pub inventory: InventoryList,
    pub spells: SpellList,
    pub ai_data: AIData,
    pub ai_package: AIPackageList,
    pub transport: Transport,
}

impl Default for Creature {
    fn default() -> Self {
        let mut ai_data = AIData::default();
        ai_data.fight = 90;
        ai_data.flee = 20;

        Self {
            record_flags: ObjectFlags::empty(),
            id: RefId::default(),
            data: NpdtStruct::default(),
            blood_type: 0,
            flags: CreatureFlags::empty(),
            scale: 1.0,
            script: RefId::default(),
            model: String::new(),
            name: String::new(),
            original: RefId::default(),
            inventory: InventoryList::default(),
            spells: SpellList::default(),
            ai_data,
            ai_package: AIPackageList::default(),
            transport: Transport::default(),
        }
    }
}

impl EsmRecord for Creature {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: u32 = name4!("CREA").to_u32();
}

impl HasSpells for Creature {
    fn spells(&self) -> &SpellList {
        &self.spells
    }
}
impl Creature {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let record_flags = reader.get_record_flags();

        let mut creature = Creature {
            record_flags,
            ..Default::default()
        };

        let mut has_name = false;
        let mut has_npdt = false;
        let mut has_flags = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    creature.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    creature.model = reader.read_header_decode_string()?;
                }
                b"CNAM" => {
                    creature.original = reader.read_refid()?;
                }
                b"FNAM" => {
                    creature.name = reader.read_header_decode_string()?;
                }
                b"SCRI" => {
                    creature.script = reader.read_refid()?;
                }
                b"NPDT" => {
                    creature.data = reader.read_typed_header_load()?;
                    has_npdt = true;
                }
                b"FLAG" => {
                    let raw_packed_flags: i32 = reader.read_typed_header()?;

                    creature.flags =
                        CreatureFlags::from_bits_retain((raw_packed_flags & 0xFF) as u8);
                    creature.blood_type = ((raw_packed_flags >> 8) & 0xFF) >> 2;
                    has_flags = true;
                }
                b"XSCL" => {
                    // C++: esm.getHT(mScale);
                    creature.scale = reader.read_typed_header()?;
                }
                b"NPCO" => {
                    creature.inventory.add(reader)?;
                }
                b"NPCS" => {
                    creature.spells.add(reader)?;
                }
                b"AIDT" => {
                    creature.ai_data = reader.read_typed_header_load()?;
                }
                b"DODT" | b"DNAM" => {
                    creature.transport.add(reader)?;
                }
                b"AI_W" | b"AI_A" | b"AI_E" | b"AI_F" | b"AI_T" | b"CNDT" => {
                    creature.ai_package.add(reader)?;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                b"INDX" => {
                    // seems to occur only in .ESS files, unsure of purpose
                    let index: i32 = reader.read_typed_header()?;
                    println!("Creature::load: Unhandled INDX {}", index);
                }
                _ => return Err(reader.fail("Unknown subrecord")),
            }
        }

        // Custom validation logic checks
        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if !has_npdt && !*is_deleted {
            return Err(reader.fail("Missing NPDT subrecord"));
        }
        if !has_flags && !*is_deleted {
            return Err(reader.fail("Missing FLAG subrecord"));
        }

        Ok(creature)
    }

    pub fn blank(&mut self) {
        let saved_id = self.id.clone();
        *self = Creature::default();
        self.id = saved_id;
    }
}
