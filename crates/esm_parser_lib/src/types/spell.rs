use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{
        effect_list::EffectList,
        flags::{ObjectFlags, SpellFlags},
    },
};
use esm_parser_macros::LoadSave;

#[derive(Default, Debug)]
pub struct Spell {
    pub record_flags: ObjectFlags,
    pub data: SpellData,
    pub name: String,
    pub id: RefId,
    pub effect_list: EffectList,
}
impl Spell {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Spell, EsmError> {
        *is_deleted = false;
        let record_flags = reader.get_record_flags();
        let mut has_name = false;
        let mut has_data = false;
        let mut spell = Spell {
            record_flags,
            ..Default::default()
        };
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    spell.id = reader.read_refid()?;
                    has_name = true;
                }
                b"FNAM" => {
                    spell.name = reader.read_header_decode_string()?;
                }
                b"SPDT" => {
                    spell.data = reader.read_typed_header_load()?;
                    has_data = true;
                }
                b"ENAM" => {
                    spell.effect_list.add(reader)?;
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
            return Err(reader.fail("Missing SPDT subrecord"));
        }
        Ok(spell)
    }
}
impl EsmRecord for Spell {
    fn id(&self) -> &RefId {
        &self.id
    }
}
pub enum SpellType {
    Spell = 0,   // Normal spell, must be cast and costs mana
    Ability = 1, // ability, always in effect
    Blight = 2,  // Blight disease
    Disease = 3, // Common disease
    Curse = 4,   // Curse (?)
    Power = 5,   // Power, can use once a day
}

#[derive(LoadSave, Default, Debug)]
pub struct SpellData {
    pub spell_type: i32, // SpellType
    pub cost: i32,       // Mana cost
    pub flags: SpellFlags,
}
