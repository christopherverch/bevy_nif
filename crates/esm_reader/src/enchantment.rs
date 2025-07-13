// src/enchantment.rs

use crate::common_types::Name; // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::spell::EffectData; // Re-use EffectData from spell.rs

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct EnchantmentData {
    pub enchant_type: i32, // 0=CastOnce, 1=CastOnStrike, 2=CastWhenUsed, 3=ConstantEffect
    pub cost: i32,         // Enchantment cost
    pub charge: i32,       // Max charge
    pub flags: u32,        // 0x0001=AutoCalc
} // Total: 4 + 4 + 4 + 4 = 16 bytes. Matches OpenMW's ENDT struct.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Enchantment {
    pub id: Name, // From NAME subrecord (Record ID is a 4-char Name, not RefId for ENCH)
    // OpenMW: mId is RefId, but it's constructed from the record's actual ID.
    // For ENCH, the NAME subrecord *is* the ID.
    // If it were a RefId field, it would be read via get_ref_id()
    // But UESP and OpenMW show NAME as a string ID here.
    // Let's stick to Name (FixedString<4>) for the subrecord,
    // but the record itself will have a RefId from the main loop.
    // So, this field might be redundant if the main record ID is already available.
    // OpenMW ESM::Enchantment has no mId from NAME subrecord, it uses the record's ID.
    // It seems ENCH records don't have a NAME subrecord for their ID.
    // The ID is the record name itself.
    // Let's verify OpenMW's loadench.cpp
    // OpenMW: esm.getHNT(mData, "ENDT"); esm.getHNT(mEffects, "ENAM");
    // It does not read a NAME subrecord into mId.
    // So, our Enchantment struct will not have an `id` field read from a NAME subrecord.
    // The ID is known by the caller (EsmReader::get_rec_name()).
    pub data: Option<EnchantmentData>, // From ENDT subrecord
    pub effects: Vec<EffectData>,      // From ENAM subrecords (multiple)
}

impl Enchantment {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads an ENCH record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "ENCH" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut ench = Enchantment::new();
        let mut endt_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"ENDT" => {
                    // Enchantment Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<EnchantmentData>() as u32; // 16 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    ench.data = Some(reader.get_t::<EnchantmentData>()?);
                    endt_found = true;
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
                    let mut effect_data = EffectData::default();
                    effect_data.effect_id = reader.get_t::<i16>()?;
                    effect_data.skill_id = reader.get_t::<i8>()?;
                    effect_data.attribute_id = reader.get_t::<i8>()?;
                    effect_data.range_type = reader.get_t::<i32>()?;
                    effect_data.area = reader.get_t::<i32>()?;
                    effect_data.duration = reader.get_t::<i32>()?;
                    effect_data.mag_min = reader.get_t::<i32>()?;
                    effect_data.mag_max = reader.get_t::<i32>()?;
                    ench.effects.push(effect_data);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !endt_found {
            return Err(reader.fail("ENCH record missing ENDT subrecord"));
        }
        // Enchantments usually have effects, but an empty list might be permissible.
        // if ench.effects.is_empty() {
        //     return Err(reader.fail("ENCH record missing ENAM (effect) subrecords"));
        // }

        Ok(ench)
    }
}
