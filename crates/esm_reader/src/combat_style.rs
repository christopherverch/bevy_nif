// src/combat_style.rs

use crate::common_types::Name; // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // ID is a RefId

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct CombatStyleData {
    // CSDA subrecord
    pub rush_distance_multiplier: f32, // How close to get before rushing (multiple of weapon range)
    pub retreat_distance_multiplier: f32, // How close to get before retreating
    pub attack_distance_multiplier: f32, // Optimal attack distance
    pub ranged_attack_multiplier: f32, // Multiplier for ranged attack preference
    pub melee_attack_multiplier: f32,  // Multiplier for melee attack preference
    pub flags: u32,                    // Bitflags (e.g., Dodge, Attack HTH if no weapon)
} // Total: 6 * 4 = 24 bytes. Matches OpenMW's CombatStyleData.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CombatStyle {
    pub id: RefId,                     // From NAME subrecord
    pub data: Option<CombatStyleData>, // From CSDA subrecord
}

impl CombatStyle {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut csty = CombatStyle::new();
        let mut id_found = false;
        let mut csda_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    csty.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"CSDA" => {
                    // Combat Style Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<CombatStyleData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    csty.data = Some(reader.get_t::<CombatStyleData>()?);
                    csda_found = true;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || csty.id.empty() {
            return Err(reader.fail("CSTY record missing mandatory NAME subrecord for its ID"));
        }
        if !csda_found {
            return Err(reader.fail(&format!(
                "CSTY record '{}' missing CSDA subrecord",
                csty.id.to_debug_string()
            )));
        }

        Ok(csty)
    }
}

