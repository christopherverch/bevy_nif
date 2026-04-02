use bytemuck::Pod;
use esm_parser_macros::LoadSave;
use std::io;
use std::io::Write;

use crate::{
    EsmError, EsmReader, Load, RefId, Save,
    types::{
        flags::{ObjectFlags, RaceFlags},
        spell_list::SpellList,
    },
};
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Race {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub name: String,
    pub spells: SpellList,
    pub description: String,
    pub data: RaceData,
}
#[repr(C)]
#[derive(Debug, Clone, Default, PartialEq, LoadSave)]
pub struct RaceData {
    pub skill_bonuses: [SkillBonus; 7],
    pub attributes: [i32; 16], // 8 attributes * 2 genders
    pub heights: [f32; 2],
    pub weights: [f32; 2],
    pub flags: RaceFlags,
}
#[repr(C)]
#[derive(Debug, Clone, Default, PartialEq, LoadSave)]
pub struct SkillBonus {
    pub skill: i32,
    pub bonus: i32,
}
impl Race {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Race, EsmError> {
        let mut race = Race::default();
        race.flags = reader.get_record_flags();
        let mut has_name = false;
        let mut has_data = false;
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    race.id = reader.read_ref_id()?;
                    has_name = true;
                }
                b"FNAM" => race.name = reader.read_header_decode_string()?,
                b"DESC" => race.description = reader.read_header_decode_string()?,
                b"RADT" => {
                    reader.read_sub_header()?;
                    // read the 140-byte race data block
                    race.data = reader.load()?;
                    has_data = true;
                }
                b"NPCS" => {
                    // read the spell and add it
                    race.spells.add(reader)?;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => return Err(reader.fail("Unknown subrecord")),
            }
        }
        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if !has_data && !*is_deleted {
            return Err(reader.fail("Missing RADT subrecord"));
        }
        Ok(race)
    }
}
