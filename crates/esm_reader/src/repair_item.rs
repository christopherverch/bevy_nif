// src/repair_item.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct RepairData {
    // RIDT subrecord
    pub weight: f32,
    pub value: i32,
    pub uses: i32,    // Number of uses for the repair item
    pub quality: f32, // Quality of the tool (affects repair effectiveness)
} // Total: 4 + 4 + 4 + 4 = 16 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RepairItem {
    pub id: RefId,                 // From NAME subrecord
    pub model: Option<String>,     // From MODL subrecord
    pub full_name: Option<String>, // From FNAM subrecord
    pub data: Option<RepairData>,  // From RIDT subrecord
    pub icon: Option<String>,      // From ITEX subrecord
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
}

impl RepairItem {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut repa = RepairItem::new();
        let mut id_found = false;
        let mut ridt_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    repa.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    repa.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    repa.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"RIDT" => {
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<RepairData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    repa.data = Some(reader.get_t::<RepairData>()?);
                    ridt_found = true;
                }
                b"ITEX" => {
                    repa.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    repa.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || repa.id.empty() {
            return Err(reader.fail("REPA record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "REPA record '{}' missing FNAM subrecord",
                repa.id.to_debug_string()
            )));
        }
        if !ridt_found {
            return Err(reader.fail(&format!(
                "REPA record '{}' missing RIDT subrecord",
                repa.id.to_debug_string()
            )));
        }

        Ok(repa)
    }
}

