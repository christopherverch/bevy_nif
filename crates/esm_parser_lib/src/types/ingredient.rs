use esm_parser_macros::LoadSave;

use crate::{
    EmptyRefId, EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{
        attribute::Attribute, enums::RecInt, flags::ObjectFlags, magic_effect::MagicEffect,
        skill::SkillIndex,
    },
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ingredient {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub icon: String,
    pub name: String,
    pub data: IngredientData,
    pub script: RefId,
}

impl EsmRecord for Ingredient {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Ingr;
}

impl Ingredient {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut ingredient = Ingredient::default();
        ingredient.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    ingredient.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    ingredient.model = normalized;
                }
                b"FNAM" => {
                    ingredient.name = reader.read_header_decode_string()?;
                }
                b"IRDT" => {
                    let bin: EsmIRDTstruct = reader.read_typed_header_load()?;
                    ingredient.data = IngredientData {
                        m_weight: bin.m_weight,
                        m_value: bin.m_value,
                        m_effect_id: std::array::from_fn(|i| {
                            MagicEffect::index_to_refid(bin.m_effect_id[i])
                        }),
                        m_skills: std::array::from_fn(|i| {
                            SkillIndex::index_to_refid(bin.m_skills[i] as i8)
                        }),
                        m_attributes: std::array::from_fn(|i| {
                            Attribute::attr_index_to_refid(bin.m_attributes[i])
                        }),
                    };
                    has_data = true;
                }
                b"SCRI" => {
                    ingredient.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    ingredient.icon = reader.read_header_decode_string()?;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
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
        if !has_data && !*is_deleted {
            return Err(reader.fail("Missing IRDT subrecord"));
        }

        // horrible hack to fix broken data in records
        for i in 0..4 {
            if ingredient.data.m_effect_id[i] != MagicEffect::ABSORBATTRIBUTE
                && ingredient.data.m_effect_id[i] != MagicEffect::DAMAGEATTRIBUTE
                && ingredient.data.m_effect_id[i] != MagicEffect::DRAINATTRIBUTE
                && ingredient.data.m_effect_id[i] != MagicEffect::FORTIFYATTRIBUTE
                && ingredient.data.m_effect_id[i] != MagicEffect::RESTOREATTRIBUTE
            {
                ingredient.data.m_attributes[i] = RefId::Empty(EmptyRefId);
            }
            if ingredient.data.m_effect_id[i] != MagicEffect::ABSORBSKILL
                && ingredient.data.m_effect_id[i] != MagicEffect::DAMAGESKILL
                && ingredient.data.m_effect_id[i] != MagicEffect::DRAINSKILL
                && ingredient.data.m_effect_id[i] != MagicEffect::FORTIFYSKILL
                && ingredient.data.m_effect_id[i] != MagicEffect::RESTORESKILL
            {
                ingredient.data.m_skills[i] = RefId::Empty(EmptyRefId);
            }
        }

        Ok(ingredient)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IngredientData {
    pub m_weight: f32,
    pub m_value: i32,
    pub m_effect_id: [RefId; 4],
    pub m_skills: [RefId; 4],
    pub m_attributes: [RefId; 4],
}
// Temporary binary layout matching EsmIRDTstruct on disk
#[derive(Clone, Copy, LoadSave)]
#[repr(C)]
struct EsmIRDTstruct {
    m_weight: f32,
    m_value: i32,
    m_effect_id: [i32; 4],
    m_skills: [i32; 4],
    m_attributes: [i32; 4],
}
