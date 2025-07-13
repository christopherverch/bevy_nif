// src/key.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct KeyData {
    // From KNDT or equivalent (usually just standard item data)
    pub weight: f32,
    pub value: i32,
    // Keys don't have specific data beyond standard item data in Morrowind ESM.
    // OpenMW ESM::Key inherits from ESM::Miscellaneous and adds no new data members
    // for subrecords beyond what MISC has (MCDT: weight, value, isKeyFlag).
    // UESP lists no unique subrecord like "KNDT" for KEYM, just standard item ones.
    // Let's assume it uses a structure similar to MISC's MCDT.
    // The "IsKey" aspect is often a flag within this data or implicit by type.
    // For now, let's use weight and value.
} // Size depends on what's standard for keys, often 8 bytes (weight, value)

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Key {
    pub id: RefId,                 // From NAME subrecord
    pub model: Option<String>,     // From MODL subrecord
    pub full_name: Option<String>, // From FNAM subrecord (displayable name)
    pub data: Option<KeyData>,     // From a data subrecord (e.g., "DATA" or "KNDT")
    pub icon: Option<String>,      // From ITEX subrecord
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
}

impl Key {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut key = Key::new();
        let mut id_found = false;
        let mut fnam_found = false;
        let mut data_sub_found = false; // For the data block (e.g. DATA, MCDT, or KNDT)

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    key.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    key.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    // Displayable name for the key
                    key.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                // Keys use a "DATA" subrecord in Morrowind.esm for weight & value, like MISC's MCDT
                // There isn't a distinct "KNDT".
                // Let's assume a 12-byte structure similar to MISC's MCDT for keys,
                // where the third int might be flags or unused.
                // OpenMW ESM::Key inherits from Miscellaneous. Its mData is ESM::Miscellaneous::MCData.
                // MCData has float mWeight, int mValue, int mIsKey;
                b"DATA" => {
                    // This is the common data subrecord for keys
                    reader.get_sub_header()?;
                    let expected_size = 12; // float weight, int value, int flags/isKey
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    let mut k_data = KeyData::default();
                    k_data.weight = reader.get_t::<f32>()?;
                    k_data.value = reader.get_t::<i32>()?;
                    let _flags_or_iskey = reader.get_t::<i32>()?; // Read the "isKey" or flags field
                    key.data = Some(k_data);
                    data_sub_found = true;
                }
                b"ITEX" => {
                    key.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    key.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || key.id.empty() {
            return Err(reader.fail("KEYM record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            // Displayable name is important
            return Err(reader.fail(&format!(
                "KEYM record '{}' missing FNAM subrecord",
                key.id.to_debug_string()
            )));
        }
        if !data_sub_found {
            // Data (weight/value) is important
            return Err(reader.fail(&format!(
                "KEYM record '{}' missing DATA/MCDT subrecord",
                key.id.to_debug_string()
            )));
        }

        Ok(key)
    }
}
