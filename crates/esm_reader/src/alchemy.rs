// src/alchemy.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;
use crate::spell::EffectData; // Re-use EffectData from spell.rs

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct AlchemyData {
    pub weight: f32,
    pub value: i32,
    pub flags: u32, // 0x0001 = Auto-Calculate, 0x0002 = PC Start Item (not used for ALCH)
} // Total: 4 + 4 + 4 = 12 bytes. Matches OpenMW's ALDT struct.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlchemyItem {
    pub id: RefId,                 // From NAME subrecord
    pub model: Option<String>,     // From MODL subrecord
    pub full_name: Option<String>, // From FNAM subrecord
    pub data: Option<AlchemyData>, // From ALDT subrecord
    pub icon: Option<String>,      // From ITEX subrecord
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
    pub effects: Vec<EffectData>,  // From ENAM subrecords (up to 4 for potions)
}

impl AlchemyItem {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut alch = AlchemyItem::new();
        let mut id_found = false;
        let mut aldt_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    alch.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    alch.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    alch.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"ALDT" => {
                    // Alchemy Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<AlchemyData>() as u32; // 12 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    alch.data = Some(reader.get_t::<AlchemyData>()?);
                    aldt_found = true;
                }
                b"ENAM" => {
                    // Effect Data, can occur multiple times
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<EffectData>() as u32; // 24 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    // Read EffectData fields individually, as it's #[repr(C, packed)]
                    let mut effect_data = EffectData::default();
                    effect_data.effect_id = reader.get_t::<i16>()?;
                    effect_data.skill_id = reader.get_t::<i8>()?;
                    effect_data.attribute_id = reader.get_t::<i8>()?;
                    effect_data.range_type = reader.get_t::<i32>()?;
                    effect_data.area = reader.get_t::<i32>()?;
                    effect_data.duration = reader.get_t::<i32>()?;
                    effect_data.mag_min = reader.get_t::<i32>()?;
                    effect_data.mag_max = reader.get_t::<i32>()?;
                    alch.effects.push(effect_data);
                }
                b"ITEX" => {
                    alch.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    alch.script_id = Some(reader.get_ref_id()?);
                }
                // TEXT subrecord for description is not typically on ALCH, but some mods might add it.
                // OpenMW ESM::Potion doesn't explicitly load TEXT.
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || alch.id.empty() {
            return Err(reader.fail("ALCH record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "ALCH record '{}' missing FNAM subrecord",
                alch.id.to_debug_string()
            )));
        }
        if !aldt_found {
            return Err(reader.fail(&format!(
                "ALCH record '{}' missing ALDT subrecord",
                alch.id.to_debug_string()
            )));
        }
        if alch.effects.is_empty() {
            // Potions should generally have effects.
            // Could be a warning or an error depending on strictness.
            // OpenMW allows potions with no effects.
        }

        Ok(alch)
    }
}
