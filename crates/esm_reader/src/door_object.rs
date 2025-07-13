// src/door_object.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DoorObject {
    pub id: RefId,                     // From NAME subrecord
    pub model: Option<String>,         // From MODL subrecord
    pub full_name: Option<String>,     // From FNAM subrecord (optional display name)
    pub open_sound_id: Option<RefId>,  // From SNAM subrecord
    pub close_sound_id: Option<RefId>, // From ANAM subrecord
    pub script_id: Option<RefId>,      // From SCRI subrecord (optional)
    pub lock_level: Option<i32>,       // From FLTV subrecord (OpenMW reads float then casts to int)
    pub key_id: Option<RefId>,         // From KNAM subrecord (optional)
    pub trap_id: Option<RefId>,        // From TNAM subrecord (optional)
}

impl DoorObject {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut door = DoorObject::new();
        let mut id_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    door.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    door.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    door.full_name = Some(reader.get_h_string()?);
                }
                b"SNAM" => {
                    // Open sound
                    door.open_sound_id = Some(reader.get_ref_id()?);
                }
                b"ANAM" => {
                    // Close sound
                    door.close_sound_id = Some(reader.get_ref_id()?);
                }
                b"SCRI" => {
                    door.script_id = Some(reader.get_ref_id()?);
                }
                b"FLTV" => {
                    // Lock Level
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    // OpenMW reads this as float then casts to int.
                    let lock_level_float = reader.get_t::<f32>()?;
                    door.lock_level = Some(lock_level_float as i32);
                }
                b"KNAM" => {
                    // Key ID
                    door.key_id = Some(reader.get_ref_id()?);
                }
                b"TNAM" => {
                    // Trap ID
                    door.trap_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || door.id.empty() {
            return Err(reader.fail("DOOR record missing mandatory NAME subrecord for its ID"));
        }
        // Model is usually essential for a door.
        if door.model.is_none() {
            return Err(reader.fail(&format!(
                "DOOR record '{}' missing MODL subrecord",
                door.id.to_debug_string()
            )));
        }

        Ok(door)
    }
}
