// src/loadltex.rs

use crate::Load;
use crate::esm_reader::{ConsumptionType, EsmError, EsmReader};
use crate::refid_types::RefId;

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
        self.id = RefId::default(); // Or RefId::default()
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

        while esm_reader.has_more_subs() {
            let sub_name = esm_reader.get_or_read_sub_name()?; // Consumes subrecord name

            // Compare by creating Name4 structs or by their u32 representation if available
            match sub_name.as_bytes() {
                b"NAME" => {
                    // read_refid should handle its own subrecord header reading (NAME + size)
                    // and then read its specific payload format.
                    ltex.id = esm_reader.read_refid()?;
                    has_name = true;
                }
                b"INTV" => {
                    esm_reader.read_sub_header()?; // Read "INTV" subrecord size
                    if esm_reader.get_sub_size() != std::mem::size_of::<u32>() as u32 {
                        return Err(EsmReader::report_sub_size_mismatch(
                            std::mem::size_of::<u32>() as u32,
                            esm_reader.get_sub_size(),
                            &esm_reader.context,
                            esm_reader.file_pos,
                        ));
                    }
                    ltex.index = esm_reader.read_typed(ConsumptionType::SubrecordPayload)?;
                    has_index = true;
                }
                b"DATA" => {
                    // get_header_string should handle its own subrecord header reading (DATA + size)
                    ltex.texture_file = esm_reader.read_header_decode_str()?.to_string();
                }
                b"DELE" => {
                    // skip_sub_data should handle its own subrecord header reading (DELE + size)
                    // and skip its payload.
                    esm_reader.skip_sub_data()?;
                }
                _ => {
                    // Unknown subrecord, fail or skip
                    let msg = format!(
                        "Unknown LTEX subrecord: {} in record {}",
                        sub_name.to_string_lossy(),
                        esm_reader.context.rec_name.to_string_lossy()
                    );
                    // Option 1: Fail
                    return Err(esm_reader.fail(&msg));
                    // Option 2: Skip and log (if you want to be more lenient)
                    // eprintln!("{}", msg);
                    // esm_reader.skip_sub_data()?;
                }
            }
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
