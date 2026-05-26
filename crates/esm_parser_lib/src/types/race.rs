use esm_parser_macros::LoadSave;

use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{
        flags::{ObjectFlags, RaceFlags},
        skill::SkillIndex,
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
impl EsmRecord for Race {
    fn id(&self) -> &RefId {
        &self.id
    }
}
#[repr(C)]
#[derive(Debug, Clone, Default, PartialEq, LoadSave)]
pub struct RaceData {
    pub skill_bonuses: [SkillBonus; 7],
    pub attributes: [i32; 16], // 8 attributes * 2 genders
    pub male_height: f32,
    pub female_height: f32,
    pub male_weight: f32,
    pub female_weight: f32,
    pub flags: RaceFlags,
}
#[repr(C)]
#[derive(Debug, Clone, Default, PartialEq, LoadSave)]
pub struct SkillBonus {
    pub skill: i32,
    pub bonus: i32,
}
impl SkillBonus {
    pub fn get_skill_id(&self) -> RefId {
        let skill_index: SkillIndex = self.skill.try_into().unwrap();
        skill_index.index_to_refid()
    }
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
                    race.id = reader.read_refid()?;
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
