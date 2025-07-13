// src/class.rs

 // Using title case as requested
use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)] // Ensure C-like packing for direct read attempt, or read fields individually
pub struct ClassData {
    pub attribute_id1: i32,
    pub attribute_id2: i32,
    pub specialization: i32, // 0: Combat, 1: Magic, 2: Stealth
    pub minor_skill_ids: [i32; 5],
    pub major_skill_ids: [i32; 5],
    pub flags: u32,           // e.g., 0x1 = Playable
    pub auto_calc_flags: u32, // e.g., 0x1 = Attributes, 0x2 = Skills
}

// For reference:
// Attribute IDs: 0=Str, 1=Int, 2=Wil, 3=Agi, 4=Spd, 5=End, 6=Per, 7=Luc
// Skill IDs: 0-26

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Class {
    // The record ID (e.g., "ACROBAT") is read by EsmReader::get_rec_name()
    // before this load function is called. We'll store the display name.
    pub full_name: String,   // From FNAM subrecord
    pub data: ClassData,     // From CLDT subrecord
    pub description: String, // From DESC subrecord
}

impl Class {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a CLAS record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "CLAS" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut class_info = Class::new();
        let mut fnam_found = false;
        let mut cldt_found = false;
        // DESC is often present but might be optional in some contexts or mods.

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [CLAS DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"FNAM" => {
                    class_info.full_name = reader.get_h_string()?;
                    fnam_found = true;
                    // println!("    [CLAS DEBUG] Full Name (FNAM): {}", class_info.full_name);
                }
                b"CLDT" => {
                    reader.get_sub_header()?;
                    let expected_cldt_size = std::mem::size_of::<ClassData>() as u32;
                    if reader.get_sub_size() != expected_cldt_size {
                        return Err(reader
                            .report_sub_size_mismatch(expected_cldt_size, reader.get_sub_size()));
                    }

                    // Read ClassData fields individually to be explicit about order and types
                    class_info.data.attribute_id1 = reader.get_t::<i32>()?;
                    class_info.data.attribute_id2 = reader.get_t::<i32>()?;
                    class_info.data.specialization = reader.get_t::<i32>()?;
                    for i in 0..5 {
                        class_info.data.minor_skill_ids[i] = reader.get_t::<i32>()?;
                    }
                    for i in 0..5 {
                        class_info.data.major_skill_ids[i] = reader.get_t::<i32>()?;
                    }
                    class_info.data.flags = reader.get_t::<u32>()?;
                    class_info.data.auto_calc_flags = reader.get_t::<u32>()?;

                    cldt_found = true;
                    // println!("    [CLAS DEBUG] Data (CLDT): {:?}", class_info.data);
                }
                b"DESC" => {
                    class_info.description = reader.get_h_string()?;
                    // let desc_preview = class_info.description.chars().take(30).collect::<String>();
                    // println!("    [CLAS DEBUG] Description (DESC): '{}...'", desc_preview);
                }
                _ => {
                    // println!("    [CLAS DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !fnam_found {
            return Err(reader.fail("CLAS record missing FNAM (full name) subrecord"));
        }
        if !cldt_found {
            return Err(reader.fail(&format!(
                "CLAS record for '{}' missing CLDT (data) subrecord",
                class_info.full_name
            )));
        }

        Ok(class_info)
    }
}
