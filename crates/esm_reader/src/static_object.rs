// src/static_object.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StaticObject {
    pub id: RefId,             // From NAME subrecord
    pub model: Option<String>, // From MODL subrecord
}

impl StaticObject {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut stat = StaticObject::new();
        let mut id_found = false;
        // MODL is generally expected for a static object to be visible.

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    stat.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    stat.model = Some(reader.get_h_string()?);
                }
                _ => {
                    // STAT records typically only have NAME and MODL.
                    // Other subrecords would be unusual or from mods.
                    // println!("[STAT DEBUG] Skipping unknown subrecord: {} for ID: {}",
                    //     sub_name.to_string_lossy(), stat.id.to_debug_string());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || stat.id.empty() {
            return Err(reader.fail("STAT record missing mandatory NAME subrecord for its ID"));
        }
        // A static object usually needs a model to be meaningful.
        if stat.model.is_none() {
            return Err(reader.fail(&format!(
                "STAT record '{}' missing MODL subrecord",
                stat.id.to_debug_string()
            )));
        }

        Ok(stat)
    }
}
