// src/skill.rs

 // Corrected to title case as per your preference
use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)] // Ensure C-like packing if reading directly as a struct
pub struct SkillData {
    pub attribute_id: i32,   // ID of the governing attribute
    pub specialization: i32, // 0: Combat, 1: Magic, 2: Stealth
    pub use_value: [f32; 4], // Four values for skill usage
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Skill {
    pub id_index: i32,   // From INDX subrecord
    pub data: SkillData, // From SKDT subrecord
    pub description: String, // From DESC subrecord
                         // The actual name of the skill (e.g., "Block", "Alchemy") is typically
                         // not stored in the SKIL record itself but inferred from id_index by the game.
                         // We can add a field for it if we load skill names from elsewhere later.
}

impl Skill {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a SKIL record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "SKIL" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut skill = Skill::new();
        let mut indx_found = false;
        let mut skdt_found = false;
        let mut desc_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [SKIL DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                // Using title case 'Name' for type alias FixedString<4>
                b"INDX" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    skill.id_index = reader.get_t::<i32>()?;
                    indx_found = true;
                    // println!("    [SKIL DEBUG] ID Index (INDX): {}", skill.id_index);
                }
                b"SKDT" => {
                    reader.get_sub_header()?;
                    let expected_skdt_size = std::mem::size_of::<SkillData>() as u32;
                    if reader.get_sub_size() != expected_skdt_size {
                        return Err(reader
                            .report_sub_size_mismatch(expected_skdt_size, reader.get_sub_size()));
                    }
                    // Read the SkillData struct directly
                    // Ensure SkillData is #[repr(C, packed)] or read fields individually
                    skill.data.attribute_id = reader.get_t::<i32>()?;
                    skill.data.specialization = reader.get_t::<i32>()?;
                    for i in 0..4 {
                        skill.data.use_value[i] = reader.get_t::<f32>()?;
                    }
                    skdt_found = true;
                    // println!("    [SKIL DEBUG] Data (SKDT): {:?}", skill.data);
                }
                b"DESC" => {
                    skill.description = reader.get_h_string()?;
                    desc_found = true;
                    // let desc_preview = skill.description.chars().take(30).collect::<String>();
                    // println!("    [SKIL DEBUG] Description (DESC): '{}...'", desc_preview);
                }
                _ => {
                    // println!("    [SKIL DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !indx_found {
            return Err(reader.fail("SKIL record missing INDX subrecord"));
        }
        if !skdt_found {
            return Err(reader.fail(&format!(
                "SKIL record for index {} missing SKDT subrecord",
                skill.id_index
            )));
        }
        // DESC might be optional for some modded skills, but usually present.
        // For now, we won't error if DESC is missing, it will just be an empty string.
        if !desc_found {
            // Optionally log a warning or handle as needed if DESC is strictly required
            // println!("[SKIL WARNING] Skill with INDX {} is missing DESC subrecord.", skill.id_index);
        }

        Ok(skill)
    }
}
