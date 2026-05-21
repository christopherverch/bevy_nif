use crate::{
    EsmError, EsmReader, RefId,
    common_inventory_types::InventoryList,
    esm_record::EsmRecord,
    types::{
        aipackage::{AIData, AIPackageList},
        flags::{NpcFlags, ObjectFlags},
        spell_list::SpellList,
        transport::Transport,
    },
};
use esm_parser_macros::LoadSave;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Npc {
    pub id: RefId,
    pub record_flags: ObjectFlags,
    pub name: String,
    pub model: String,

    pub race: RefId,
    pub class: RefId,
    pub faction: RefId,
    pub script: RefId,
    pub head: RefId,
    pub hair: RefId,

    pub npc_flags: NpcFlags,
    pub blood_type: i32,

    pub data: Npdt52,
    pub ai_data: AIData,
    pub ai_packages: AIPackageList,
    pub inventory: InventoryList,
    pub spells: SpellList,
    pub transport: Transport,
}
impl EsmRecord for Npc {
    fn id(&self) -> &RefId {
        &self.id
    }
}
impl Npc {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        let mut npc = Npc::default();
        npc.record_flags = reader.get_record_flags();
        npc.ai_data.greeting_distance = 30;
        npc.ai_data.fight = 30;
        npc.ai_data.flee = 30;
        let mut has_name = false;
        let mut has_npdt = false;
        let mut has_flags = false;
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    npc.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => npc.model = reader.read_header_decode_string()?,
                b"FNAM" => npc.name = reader.read_header_decode_string()?,
                b"RNAM" => npc.race = reader.read_refid()?,
                b"CNAM" => npc.class = reader.read_refid()?,
                b"ANAM" => npc.faction = reader.read_refid()?,
                b"BNAM" => npc.head = reader.read_refid()?,
                b"KNAM" => npc.hair = reader.read_refid()?,
                b"SCRI" => npc.script = reader.read_refid()?,
                b"NPDT" => {
                    has_npdt = true;
                    reader.read_sub_header()?;
                    let size = reader.context.remaining_subrec_data;
                    // Size of npc.data, NPDT52
                    if size == 52 {
                        npc.data = reader.load()?;
                    } else if size == 12 {
                        let data12: Npdt12 = reader.load()?;
                        // Convert short stats to internal 52-byte struct
                        npc.data.level = data12.level;
                        npc.data.disposition = data12.disposition;
                        npc.data.reputation = data12.reputation;
                        npc.data.rank = data12.rank;
                        npc.data.gold = data12.gold;
                    } else {
                        return Err(reader.fail("NPC_NPDT must be 12 or 52 bytes long"));
                    }
                }
                b"FLAG" => {
                    has_flags = true;
                    reader.read_sub_header()?;
                    let raw_flags: i32 = reader.load()?;
                    npc.npc_flags = NpcFlags::from_bits_truncate((raw_flags & 0xFF) as u8);
                    npc.blood_type = ((raw_flags >> 8) & 0xFF) >> 2;
                }
                b"NPCS" => {
                    npc.spells.add(reader)?;
                }
                b"NPCO" => {
                    npc.inventory.add(reader)?;
                }
                b"AIDT" => {
                    reader.read_sub_header()?;
                    npc.ai_data = reader.load()?;
                }
                b"DODT" | b"DNAM" => {
                    npc.transport.add(reader)?;
                }

                // Delegate AI packages to our specialized list handler
                b"AI_W" | b"AI_T" | b"AI_F" | b"AI_E" | b"AI_A" | b"CNDT" => {
                    npc.ai_packages.add(reader)?;
                }
                b"DELE" => {
                    reader.skip_sub_data()?;
                    *is_deleted = true;
                }
                _ => {
                    return Err(reader.fail("Unknown subrecord"));
                }
            }
        }

        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        // Note: isDeleted check usually happens at the record level,
        // but mirroring the C++ safety check here:
        if !npc.record_flags.contains(ObjectFlags::DELETED) {
            if !has_npdt {
                return Err(reader.fail("Missing NPDT subrecord"));
            }
            if !has_flags {
                return Err(reader.fail("Missing FLAG subrecord"));
            }
        }

        Ok(npc)
    }
    pub fn is_male(&self) -> bool {
        self.npc_flags.contains(NpcFlags::FEMALE)
    }
}
#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct Npdt52 {
    pub level: i16,
    pub attributes: [u8; 8], // Strength, Intelligence, etc.
    pub skills: [u8; 27],    // Block, Armorer, etc.
    pub _padding: u8,        // Align health to 2-byte boundary
    pub health: u16,
    pub mana: u16,
    pub fatigue: u16,
    pub disposition: u8,
    pub reputation: u8,
    pub rank: u8,
    pub _padding2: u8, // Align gold to 4-byte boundary
    pub gold: i32,
}
// Converted to npdt52 internally
#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct Npdt12 {
    pub level: i16,
    pub disposition: u8,
    pub reputation: u8,
    pub rank: u8,
    pub _padding: [u8; 3],
    pub gold: i32,
}
