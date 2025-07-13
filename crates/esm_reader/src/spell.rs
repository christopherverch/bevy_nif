// src/spell.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct SpellData {
    pub spell_type: i32, // 0=Spell, 1=Ability, 2=Blight, 3=Disease, 4=Curse, 5=Power
    pub cost: i32,       // Magicka cost for spells, 0 for others
    pub flags: u32,      // 0x1:AutoCalc, 0x2:PC Start, 0x4:AlwaysSucceeds
} // Total 12 bytes

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct EffectData {
    pub effect_id: i16,   // Magic Effect ID (from MGEF INDX)
    pub skill_id: i8,     // Skill affected (-1 if none, or attribute ID if attr_id is -1)
    pub attribute_id: i8, // Attribute affected (-1 if none)
    pub range_type: i32,  // 0=Self, 1=Touch, 2=Target
    pub area: i32,
    pub duration: i32,
    pub mag_min: i32,
    pub mag_max: i32,
} // Total 24 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spell {
    pub id: String,               // Record ID, supplied by caller
    pub full_name: String,        // From FNAM subrecord
    pub data: SpellData,          // From SPDT subrecord
    pub effects: Vec<EffectData>, // From ENAM subrecords (multiple)
}

impl Spell {
    pub fn new(id: String) -> Self {
        Spell {
            id,
            ..Default::default()
        }
    }

    pub fn load(reader: &mut EsmReader, record_id: String) -> Result<Self, EsmError> {
        let mut spell = Spell::new(record_id);
        let mut fnam_found = false;
        let mut spdt_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [SPEL DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"FNAM" => {
                    spell.full_name = reader.get_h_string()?;
                    fnam_found = true;
                }
                b"SPDT" => {
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<SpellData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    spell.data.spell_type = reader.get_t::<i32>()?;
                    spell.data.cost = reader.get_t::<i32>()?;
                    spell.data.flags = reader.get_t::<u32>()?;
                    spdt_found = true;
                }
                b"ENAM" => {
                    // Effect Data, can occur multiple times
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<EffectData>() as u32;
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
                    spell.effects.push(effect_data);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !fnam_found {
            return Err(reader.fail(&format!(
                "SPEL record '{}' missing FNAM subrecord",
                spell.id
            )));
        }
        if !spdt_found {
            return Err(reader.fail(&format!(
                "SPEL record '{}' missing SPDT subrecord",
                spell.id
            )));
        }
        // A spell must have at least one effect, usually.
        if spell.effects.is_empty() {
            // This might be a warning rather than an error for some data files.
            // println!("[SPEL WARNING] Spell '{}' has no effects (ENAM subrecords).", spell.id);
        }

        Ok(spell)
    }
}

