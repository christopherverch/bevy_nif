// src/birthsign.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Birthsign {
    pub id: RefId,                   // From NAME subrecord
    pub full_name: Option<String>,   // From FNAM subrecord
    pub texture: Option<String>,     // From TNAM subrecord
    pub description: Option<String>, // From DESC subrecord
    pub powers: Vec<RefId>,          // From NPCS subrecords
}

impl Birthsign {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a BSGN record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "BSGN" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut sign = Birthsign::new();
        let mut id_found = false;
        // FNAM, TNAM, DESC are typically present for vanilla birthsigns.

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [BSGN DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"NAME" => {
                    // This call relies on EsmReader::get_ref_id() being fixed
                    // to correctly interpret plain string subrecords for older formats.
                    sign.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"FNAM" => {
                    sign.full_name = Some(reader.get_h_string()?);
                }
                b"TNAM" => {
                    sign.texture = Some(reader.get_h_string()?);
                }
                b"DESC" => {
                    sign.description = Some(reader.get_h_string()?);
                }
                b"NPCS" => {
                    // Spell/Ability ID, can occur multiple times
                    // In OpenMW, this is read as esm.getRefId().
                    // This implies the NPCS subrecord data *is* a serialized RefId.
                    // If it's just a string ID, then RefId::string_ref_id(reader.get_h_string()?) would be used.
                    // Given the pattern from REGN's NAME, we'll use get_ref_id here.
                    sign.powers.push(reader.get_ref_id()?);
                }
                _ => {
                    // println!("    [BSGN DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || sign.id.empty() {
            return Err(reader.fail("BSGN record missing mandatory NAME subrecord for its ID"));
        }
        // Unlike some records, OpenMW doesn't strictly fail if FNAM, TNAM, or DESC are missing for BSGN
        // during the load phase itself, as they are std::string members that would just be empty.
        // However, a valid BSGN record should typically have them. We'll make them Option<String>.

        Ok(sign)
    }
}
