// src/activator.rs

// Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Activator {
    pub id: RefId,                 // From NAME subrecord (Record ID)
    pub model: Option<String>,     // From MODL subrecord (optional)
    pub full_name: Option<String>, // From FNAM subrecord (optional, displayable name)
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
}

impl Activator {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads an ACTI record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "ACTI" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut acti = Activator::new();
        let mut id_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    acti.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    acti.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    acti.full_name = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    acti.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    // Activators are simple; other subrecords are unlikely for vanilla.
                    // println!("[ACTI DEBUG] Skipping unknown subrecord: {} for ID: {}",
                    //     sub_name.to_string_lossy(), acti.id.to_debug_string());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || acti.id.empty() {
            return Err(reader.fail("ACTI record missing mandatory NAME subrecord for its ID"));
        }
        // Model, FNAM, and SCRI are all optional for an activator to be valid.
        // An invisible, unnamed activator that only runs a global script via its ID is possible.

        Ok(acti)
    }
}
