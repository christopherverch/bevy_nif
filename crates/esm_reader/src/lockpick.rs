// src/lockpick.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct LockpickData {
    // LKDT
    pub weight: f32,
    pub value: i32,
    pub quality: f32,
    pub uses: i32, // Number of uses (OpenMW: int mUses)
} // Total: 4 + 4 + 4 + 4 = 16 bytes. Matches OpenMW's LKDTstruct.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lockpick {
    pub id: RefId,                  // From NAME subrecord
    pub model: Option<String>,      // From MODL subrecord
    pub full_name: Option<String>,  // From FNAM subrecord
    pub data: Option<LockpickData>, // From LKDT subrecord
    pub icon: Option<String>,       // From ITEX subrecord
    pub script_id: Option<RefId>,   // From SCRI subrecord (optional)
}

impl Lockpick {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut lock = Lockpick::new();
        let mut id_found = false;
        let mut lkdt_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    lock.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    lock.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    lock.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"LKDT" => {
                    // Lockpick Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<LockpickData>() as u32; // 16 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    lock.data = Some(reader.get_t::<LockpickData>()?);
                    lkdt_found = true;
                }
                b"ITEX" => {
                    lock.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    lock.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || lock.id.empty() {
            return Err(reader.fail("LOCK record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "LOCK record '{}' missing FNAM subrecord",
                lock.id.to_debug_string()
            )));
        }
        if !lkdt_found {
            return Err(reader.fail(&format!(
                "LOCK record '{}' missing LKDT subrecord",
                lock.id.to_debug_string()
            )));
        }

        Ok(lock)
    }
}
