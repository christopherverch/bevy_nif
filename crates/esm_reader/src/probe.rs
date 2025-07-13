// src/probe.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct ProbeData {
    // PBDT subrecord
    pub weight: f32,
    pub value: i32,
    pub quality: f32, // Affects chance of success
    pub uses: i32,    // Number of uses for the probe
} // Total: 4 + 4 + 4 + 4 = 16 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Probe {
    pub id: RefId,                 // From NAME subrecord
    pub model: Option<String>,     // From MODL subrecord
    pub full_name: Option<String>, // From FNAM subrecord
    pub data: Option<ProbeData>,   // From PBDT subrecord
    pub icon: Option<String>,      // From ITEX subrecord
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
}

impl Probe {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut prob = Probe::new();
        let mut id_found = false;
        let mut pbdt_found = false; // For PBDT (Probe Data)
        let mut fnam_found = false; // For FNAM (Full Name)

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    prob.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    prob.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    prob.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"PBDT" => {
                    // Probe Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<ProbeData>() as u32; // Now 16 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    prob.data = Some(reader.get_t::<ProbeData>()?);
                    pbdt_found = true;
                }
                b"ITEX" => {
                    prob.icon = Some(reader.get_h_string()?);
                }
                b"SCRI" => {
                    prob.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || prob.id.empty() {
            return Err(reader.fail("PROB record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            // FNAM is typically mandatory for usable items
            return Err(reader.fail(&format!(
                "PROB record '{}' missing FNAM subrecord",
                prob.id.to_debug_string()
            )));
        }
        if !pbdt_found {
            // PBDT is essential data for a probe
            return Err(reader.fail(&format!(
                "PROB record '{}' missing PBDT subrecord",
                prob.id.to_debug_string()
            )));
        }

        Ok(prob)
    }
}

