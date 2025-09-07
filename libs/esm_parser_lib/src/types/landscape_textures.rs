// src/loadltex.rs

use crate::esm_reader::{ConsumptionType, EsmError, EsmReader};
use crate::refid_types::RefId;
use crate::{Load, load_subrecords};

use super::flags::ObjectFlags;

#[derive(Debug, Default, Clone)]
pub struct LandTexture {
    pub flags: ObjectFlags,
    /// From "DATA" subrecord: The texture file path.
    pub texture_file: String,
    /// From "NAME" subrecord: The editor ID of this landscape texture.
    pub id: RefId,
    /// From "INTV" subrecord: The index/ID number of the texture.
    pub index: u32,
}

impl LandTexture {
    /// Return a string descriptor for this record type.
    pub fn get_record_type() -> &'static str {
        "LandTexture"
    }

    /// Sets the record to a default state.
    pub fn blank(&mut self) {
        self.id = RefId::new(); // Or RefId::default()
        self.texture_file.clear();
        // mIndex is explicitly not cleared in the OpenMW C++ `blank` method.
        // self.index = 0; // If you prefer to reset it.
    }
}
impl Load for LandTexture {
    /// Loads the LandTexture data from the ESM reader.
    /// The ESMReader is expected to be positioned at the beginning of the subrecords
    /// for an LTEX record, after the "LTEX" record header itself has been read.
    /// Returns the loaded LandTexture and a boolean indicating if it's marked as deleted.
    fn load(esm_reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut ltex = Self::default();
        let mut has_name = false;
        let mut has_index = false;

        load_subrecords! {
            esm_reader,
            b"NAME" => {
                // get_ref_id handles its own header
                ltex.id = esm_reader.load()?;
                has_name = true;
            },
            b"INTV" => {
                // Manual header read is required here
                esm_reader.get_sub_header()?;
                let expected_size = std::mem::size_of::<u32>() as u32;
                if esm_reader.get_sub_size() != expected_size {
                    return Err(esm_reader.report_sub_size_mismatch(expected_size, esm_reader.get_sub_size()));
                }
                ltex.index = esm_reader.get_u32_le(ConsumptionType::SubrecordPayload)?;
                has_index = true;
            },
            b"DATA" => {
                // get_h_string handles its own header
                ltex.texture_file = esm_reader.load()?;
            },
            b"DELE" => {
                // skip_sub_data handles its own header
                esm_reader.skip_sub_data()?;
                ltex.flags.insert(ObjectFlags::DELETED);
            },
        }

        if !has_name {
            return Err(esm_reader.fail("Missing NAME subrecord in LTEX"));
        }
        if !has_index {
            return Err(esm_reader.fail("Missing INTV subrecord in LTEX"));
        }

        Ok(ltex)
    }
}
