// src/land_texture.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // LTEX ID is a RefId

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LandTexture {
    pub id: RefId,                // From NAME subrecord (e.g., "dirt_01")
    pub texture_filename: String, // From DATA subrecord (e.g., "Tx_Dirt_01.tga")
    pub index: Option<i32>,       // From INTV subrecord (Texture index, an integer)
}

impl LandTexture {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut ltex = LandTexture::new();
        let mut id_found = false;
        let mut data_found = false; // For texture filename
        let mut intv_found = false; // For index

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    ltex.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"DATA" => {
                    // Texture Filename
                    ltex.texture_filename = reader.get_h_string()?;
                    data_found = true;
                }
                b"INTV" => {
                    // Texture Index
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    ltex.index = Some(reader.get_t::<i32>()?);
                    intv_found = true;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || ltex.id.empty() {
            return Err(reader.fail("LTEX record missing mandatory NAME subrecord for its ID"));
        }
        if !data_found {
            return Err(reader.fail(&format!(
                "LTEX record '{}' missing DATA (filename) subrecord",
                ltex.id.to_debug_string()
            )));
        }
        if !intv_found {
            return Err(reader.fail(&format!(
                "LTEX record '{}' missing INTV (index) subrecord",
                ltex.id.to_debug_string()
            )));
        }

        Ok(ltex)
    }
}
