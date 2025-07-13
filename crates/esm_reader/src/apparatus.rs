// src/apparatus.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct ApparatusData {
    pub apparatus_type: i32, // Enum: 0=Mortar&Pestle, 1=Alembic, 2=Calcinator, 3=Retort
    pub quality: f32,
    pub weight: f32,
    pub value: i32,
} // Total: 4 + 4 + 4 + 4 = 16 bytes. This matches OpenMW's AADT struct.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Apparatus {
    pub id: RefId,                   // From NAME subrecord
    pub model: Option<String>,       // From MODL subrecord
    pub full_name: Option<String>,   // From FNAM subrecord
    pub data: Option<ApparatusData>, // From AADT subrecord
    pub icon: Option<String>,        // From ITEX subrecord
    pub script_id: Option<RefId>,    // From SCRI subrecord (optional)
}

impl Apparatus {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut appa = Apparatus::new();
        let mut id_found = false;
        let mut aadt_found = false;
        let mut fnam_found = false; // FNAM is usually present for usable items

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    appa.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    appa.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    appa.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"AADT" => {
                    // Apparatus Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<ApparatusData>() as u32; // 16 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    appa.data = Some(reader.get_t::<ApparatusData>()?);
                    aadt_found = true;
                }
                b"ITEX" => {
                    appa.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    appa.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || appa.id.empty() {
            return Err(reader.fail("APPA record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "APPA record '{}' missing FNAM subrecord",
                appa.id.to_debug_string()
            )));
        }
        if !aadt_found {
            return Err(reader.fail(&format!(
                "APPA record '{}' missing AADT subrecord",
                appa.id.to_debug_string()
            )));
        }

        Ok(appa)
    }
}
