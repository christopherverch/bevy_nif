use crate::types::attribute::Attribute;
use crate::types::skill::SkillIndex;
use crate::{
    EsmError, EsmReader, Load, RefId, common_consts::NUM_MAGIC_EFFECTS,
    format_version_defs::MAX_SERIALIZE_EFFECT_REFID_FORMAT_VERSION, name4,
    types::magic_effect::MagicEffect,
};
use esm_parser_macros::LoadSave;

#[derive(Default, Debug)]
pub struct EffectList {
    pub list: Vec<IndexedENAMStruct>,
}
#[derive(Debug)]
pub struct IndexedENAMStruct {
    pub data: ENAMStruct,
    pub index: i32,
}
impl PartialEq for IndexedENAMStruct {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}
impl Load for EffectList {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut effect_list = EffectList::default();
        while reader.is_next_sub(name4!("ENAM"))? {
            effect_list.add(reader)?;
        }
        Ok(effect_list)
    }
}
impl EffectList {
    pub fn add(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        reader.read_sub_header()?;
        let enam_struct =
            if reader.header.format_version <= MAX_SERIALIZE_EFFECT_REFID_FORMAT_VERSION {
                let esm_enam_struct = reader.load()?;
                from_binary(&esm_enam_struct)?
            } else {
                let mut enam_struct = ENAMStruct::default();
                let effect_params = reader.load()?;
                set_effect_params(&effect_params, &mut enam_struct);
                enam_struct.effect_id = reader.read_header_refid_check_name(name4!("ENID"))?;
                enam_struct.skill = reader.read_refid_named_sub_or_default(name4!("ENSK"))?;
                enam_struct.attribute = reader.read_refid_named_sub_or_default(name4!("ENAT"))?;
                enam_struct
            };
        self.list.push(IndexedENAMStruct {
            data: enam_struct,
            index: self.list.len() as i32,
        });
        Ok(())
    }
    pub fn deserialize(src: &EsmENAMStruct) -> ENAMStruct {
        let index = src.effect_id;
        if index < 0 || index > 0 {}
        ENAMStruct::default()
    }
}
fn from_binary(src: &EsmENAMStruct) -> Result<ENAMStruct, EsmError> {
    let index = src.effect_id;
    if index < 0 || index >= NUM_MAGIC_EFFECTS as i16 {
        return Err(EsmError::Custom(format!(
            "Cannot deserialize effect into ENAM with index {}",
            index
        )));
    }
    Ok(ENAMStruct {
        effect_id: MagicEffect::index_to_refid(index.into()),
        skill: SkillIndex::index_to_refid(src.skill.into()),
        attribute: Attribute::index_to_refid(src.attribute.into()),
        range: src.range,
        area: src.area,
        duration: src.duration,
        min_magnitude: src.min_magnitude,
        max_magnitude: src.max_magnitude,
    })
}
/// Defines a spell effect. Shared between SPEL (Spells), ALCH
/// (Potions) and ENCH (Item enchantments) records
#[derive(LoadSave, PartialEq, Default, Debug)]
pub struct ENAMStruct {
    /// Magical effect, serialized to i16
    pub effect_id: RefId,

    /// Which skills/attributes are affected (for restore/drain spells
    /// etc.)
    pub skill: RefId,
    pub attribute: RefId,
    // Other params
    /// 0 - self, 1 - touch, 2 - target (RangeType enum)
    pub range: i32,
    pub area: i32,
    pub duration: i32,
    pub min_magnitude: i32,
    pub max_magnitude: i32,
}
/// The serialized version of the struct, how it appears in morrowind.esm
#[derive(LoadSave, PartialEq, Default)]
pub struct EsmENAMStruct {
    /// Magical effect, serialized to i16
    effect_id: i16,

    /// Which skills/attributes are affected (for restore/drain spells
    /// etc.)
    skill: i8,
    attribute: i8,
    // Other params
    /// 0 - self, 1 - touch, 2 - target (RangeType enum)
    range: i32,
    area: i32,
    duration: i32,
    min_magnitude: i32,
    max_magnitude: i32,
}
#[derive(LoadSave, PartialEq, Default)]
pub struct EffectParams {
    range: i32,
    area: i32,
    duration: i32,
    min_magnitude: i32,
    max_magnitude: i32,
}
fn set_effect_params(src: &EffectParams, dest: &mut ENAMStruct) {
    dest.range = src.range;
    dest.area = src.area;
    dest.duration = src.duration;
    dest.min_magnitude = src.min_magnitude;
    dest.max_magnitude = src.max_magnitude;
}
