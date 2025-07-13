// src/light.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)] // This struct represents the full 24-byte LHDT data
pub struct LightData {
    pub weight: f32, // Present in 24-byte LHDT (carryable lights)
    pub value: i32,  // Present in 24-byte LHDT (carryable lights)
    pub time: i32,   // Duration (-1 or 0 for infinite)
    pub radius: i32,
    pub color: u32, // RGBA packed (OpenMW stores as int32_t, u32 for direct bits)
    pub flags: u32, // Bitflags (e.g., 0x0001=Dynamic, 0x0020=CanCarry, etc.)
} // Full struct is 24 bytes. Partial (dynamic light) is 16 bytes (time, radius, color, flags).

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Light {
    pub id: RefId,                 // From NAME subrecord
    pub model: Option<String>,     // From MODL subrecord (optional)
    pub full_name: Option<String>, // From FNAM subrecord (optional, name if item)
    pub data: Option<LightData>,   // From LHDT subrecord
    pub icon: Option<String>,      // From ITEX subrecord (optional, if item)
    pub sound_id: Option<RefId>,   // From SNAM subrecord (optional)
    pub script_id: Option<RefId>,  // From SCRI subrecord (optional)
}

impl Light {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a LIGH record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "LIGH" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut light = Light::new();
        let mut id_found = false;
        let mut lhdt_found = false; // To ensure LHDT is processed

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    light.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    light.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    light.full_name = Some(reader.get_h_string()?);
                }
                b"LHDT" => {
                    reader.get_sub_header()?; // Reads subrecord size into m_ctx.left_sub
                    let sub_size = reader.get_sub_size();
                    let mut lh_data = LightData::default(); // Initialize with defaults (weight/value = 0)

                    const LHDT_ITEM_SIZE: u32 = 24; // For carryable lights with weight/value
                    const LHDT_DYNAMIC_SIZE: u32 = 16; // For dynamic lights (time, radius, color, flags only)

                    if sub_size == LHDT_ITEM_SIZE {
                        // This is a 24-byte LHDT, typically for a carryable light item
                        lh_data.weight = reader.get_t::<f32>()?;
                        lh_data.value = reader.get_t::<i32>()?;
                        lh_data.time = reader.get_t::<i32>()?;
                        lh_data.radius = reader.get_t::<i32>()?;
                        lh_data.color = reader.get_t::<u32>()?;
                        lh_data.flags = reader.get_t::<u32>()?;
                    } else if sub_size == LHDT_DYNAMIC_SIZE {
                        // This is a 16-byte LHDT, for dynamic lights (or items where weight/value are omitted)
                        // Weight and value remain default (0.0 and 0 from LightData::default())
                        lh_data.time = reader.get_t::<i32>()?;
                        lh_data.radius = reader.get_t::<i32>()?;
                        lh_data.color = reader.get_t::<u32>()?;
                        lh_data.flags = reader.get_t::<u32>()?;
                    } else {
                        // Unexpected LHDT size
                        return Err(reader.fail(&format!(
                            "LIGH record '{}' LHDT has unexpected size {} (expected {} or {})",
                            light.id.to_debug_string(),
                            sub_size,
                            LHDT_ITEM_SIZE,
                            LHDT_DYNAMIC_SIZE
                        )));
                    }
                    light.data = Some(lh_data);
                    lhdt_found = true;
                }
                b"ITEX" => {
                    light.icon = Some(reader.get_h_string()?);
                }
                b"SNAM" => {
                    light.sound_id = Some(reader.get_ref_id()?);
                }
                b"SCRI" => {
                    light.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || light.id.empty() {
            return Err(reader.fail("LIGH record missing mandatory NAME subrecord for its ID"));
        }
        // LHDT is generally expected if it's a functional light.
        // OpenMW checks `if (mModel.empty() && mData.mRadius == 0) esm.fail("LIGH record must have a radius if it has no model");`
        // For simplicity, we'll just ensure LHDT was found.
        if !lhdt_found {
            return Err(reader.fail(&format!(
                "LIGH record '{}' missing LHDT subrecord",
                light.id.to_debug_string()
            )));
        }

        Ok(light)
    }
}
