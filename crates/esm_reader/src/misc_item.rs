// src/misc_item.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct MiscData {
    pub weight: f32,
    pub value: i32,
    pub unknown_or_is_key: i32, // UESP: "Unknown/IsKey?". OpenMW: mIsKey (int, seems to be flags 0x1=key)
} // Total 4+4+4 = 12 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MiscItem {
    pub id: RefId,                 // From NAME subrecord
    pub model: Option<String>,     // From MODL subrecord
    pub full_name: Option<String>, // From FNAM subrecord
    pub data: Option<MiscData>,    // From MCDT subrecord
    pub icon: Option<String>,      // From ITEX subrecord
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
                                   // ENAM (Enchantment) is not standard for MISC but mods might add it.
                                   // We'll omit it for now unless OpenMW specifically loads it for MISC.
                                   // OpenMW's ESM::Miscellaneous does not list mEnchant.
}

impl MiscItem {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut misc = MiscItem::new();
        let mut id_found = false;
        let mut mcdt_found = false; // For MCDT
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    misc.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    misc.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    misc.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"MCDT" => {
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<MiscData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    misc.data = Some(reader.get_t::<MiscData>()?);
                    mcdt_found = true;
                }
                b"ITEX" => {
                    misc.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    misc.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || misc.id.empty() {
            return Err(reader.fail("MISC record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "MISC record '{}' missing FNAM subrecord",
                misc.id.to_debug_string()
            )));
        }
        if !mcdt_found {
            return Err(reader.fail(&format!(
                "MISC record '{}' missing MCDT subrecord",
                misc.id.to_debug_string()
            )));
        }

        Ok(misc)
    }
}
