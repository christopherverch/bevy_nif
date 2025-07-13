// src/magic_effect.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
// use crate::refid_types::RefId; // For later if sound/VFX IDs become RefId

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)] // Ensure C-like packing if reading directly as a struct
pub struct MagicEffectData {
    pub school_id_raw: i32, // Raw skill/school index from file
    pub base_cost: f32,
    pub flags: u32,
    pub color_red: i32,   // Changed from u8 to i32
    pub color_green: i32, // Changed from u8 to i32
    pub color_blue: i32,  // Changed from u8 to i32
    // Removed _padding: u8
    pub size_x: f32,   // Corresponds to mUnknown1 / "Size X"
    pub speed: f32,    // Corresponds to mSpeed
    pub size_cap: f32, // Corresponds to mUnknown2 / "Size Cap"
} // Total: 4+4+4+4+4+4+4+4+4 = 36 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MagicEffect {
    pub id: String,                       // Record ID, supplied by caller
    pub effect_id_idx: i32,               // From INDX subrecord (the numerical MGEF ID)
    pub data: MagicEffectData,            // From MEDT subrecord
    pub icon_texture: Option<String>,     // From ITEX
    pub particle_texture: Option<String>, // From PTEX
    pub bolt_sound_id: Option<String>,    // From BSND (OpenMW uses RefId, we use String for now)
    pub cast_sound_id: Option<String>,    // From CSND
    pub hit_sound_id: Option<String>,     // From HSND
    pub area_sound_id: Option<String>,    // From ASND
    pub description: Option<String>,      // From DESC
                                          // OpenMW also has mCasting, mHit, mArea, mBolt (VFX as RefId) - these would be CVFX, HVFX, AVFX, BVFX
}

impl MagicEffect {
    pub fn new(id: String) -> Self {
        MagicEffect {
            id,
            ..Default::default()
        }
    }

    pub fn load(reader: &mut EsmReader, record_id: String) -> Result<Self, EsmError> {
        let mut mgef = MagicEffect::new(record_id);
        let mut indx_found = false;
        let mut medt_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"INDX" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    mgef.effect_id_idx = reader.get_t::<i32>()?;
                    indx_found = true;
                }
                b"MEDT" => {
                    reader.get_sub_header()?;
                    let medt_data_size_from_file = reader.get_sub_size();
                    let expected_medt_struct_size = std::mem::size_of::<MagicEffectData>() as u32;

                    if medt_data_size_from_file != expected_medt_struct_size {
                        // This error means the MEDT subrecord in the file does not match our 36-byte expectation
                        // based on OpenMW's MEDTstruct. This would be unusual if Morrowind.esm is standard.
                        eprintln!(
                            "[MGEF WARNING] MEDT for '{}' (idx {}) has size {}, expected {}. Attempting to read known fields, but data might be corrupt or format unexpected.",
                            mgef.id,
                            mgef.effect_id_idx,
                            medt_data_size_from_file,
                            expected_medt_struct_size
                        );
                        // If we still want to try and parse a portion or if OpenMW has more complex logic
                        // for variable MEDT (which it doesn't seem to for this core block), it would go here.
                        // For now, strict matching to the 36-byte struct.
                        return Err(reader.report_sub_size_mismatch(
                            expected_medt_struct_size,
                            medt_data_size_from_file,
                        ));
                    }

                    // Read the 36 bytes into MagicEffectData
                    mgef.data.school_id_raw = reader.get_t::<i32>()?;
                    mgef.data.base_cost = reader.get_t::<f32>()?;
                    mgef.data.flags = reader.get_t::<u32>()?;
                    mgef.data.color_red = reader.get_t::<i32>()?; // Read as i32
                    mgef.data.color_green = reader.get_t::<i32>()?; // Read as i32
                    mgef.data.color_blue = reader.get_t::<i32>()?; // Read as i32
                    mgef.data.size_x = reader.get_t::<f32>()?; // Was mUnknown1
                    mgef.data.speed = reader.get_t::<f32>()?;
                    mgef.data.size_cap = reader.get_t::<f32>()?; // Was mUnknown2

                    medt_found = true;
                }
                b"ITEX" => mgef.icon_texture = Some(reader.get_h_string()?),
                b"PTEX" => mgef.particle_texture = Some(reader.get_h_string()?),
                b"BSND" => mgef.bolt_sound_id = Some(reader.get_h_string()?), // OpenMW uses RefId
                b"CSND" => mgef.cast_sound_id = Some(reader.get_h_string()?), // OpenMW uses RefId
                b"HSND" => mgef.hit_sound_id = Some(reader.get_h_string()?),  // OpenMW uses RefId
                b"ASND" => mgef.area_sound_id = Some(reader.get_h_string()?), // OpenMW uses RefId
                // CVFX, BVFX, HVFX, AVFX for visual effects (RefId in OpenMW) could be added here
                b"DESC" => mgef.description = Some(reader.get_h_string()?),
                _ => {
                    // println!("[MGEF DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !indx_found {
            return Err(reader.fail(&format!("MGEF record '{}' missing INDX subrecord", mgef.id)));
        }
        if !medt_found {
            return Err(reader.fail(&format!("MGEF record '{}' missing MEDT subrecord", mgef.id)));
        }

        Ok(mgef)
    }
}

