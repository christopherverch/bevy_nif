use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{
        effect_list::EffectList,
        enums::{EnchantType, RecInt},
        flags::{EnchantingFlags, ObjectFlags},
    },
};

use esm_parser_macros::LoadSave;
#[derive(Clone, Debug, Default)]
pub struct Enchantment {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub data: ENDTstruct,
    pub effects: EffectList,
}

impl EsmRecord for Enchantment {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Ench;
}

impl Enchantment {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut enchantment = Enchantment::default();
        enchantment.record_flags = reader.get_record_flags();
        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    enchantment.id = reader.read_refid()?;
                    has_name = true;
                }
                b"ENDT" => {
                    reader.read_sub_header()?;
                    enchantment.data = reader.load()?;
                    has_data = true;
                }
                b"ENAM" => {
                    enchantment.effects.add(reader)?;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                a => {
                    dbg!(a);
                    return Err(reader.fail("Unknown subrecord"));
                }
            }
        }
        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if !has_data && !*is_deleted {
            return Err(reader.fail("Missing ENDT subrecord"));
        }

        Ok(enchantment)
    }
}

#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct ENDTstruct {
    pub m_type: EnchantType,
    pub m_cost: i32,
    pub m_charge: i32,
    pub m_flags: EnchantingFlags,
}
