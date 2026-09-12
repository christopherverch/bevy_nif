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
        self.id = RefId::default();
        self.texture_file.clear();
    }
}
/// Loads the LandTexture data from the ESM reader.
/// The ESMReader is expected to be positioned at the beginning of the subrecords
/// for an LTEX record, after the "LTEX" record header itself has been read.
/// Returns the loaded LandTexture and a boolean indicating if it's marked as deleted.
impl LandTexture {
    pub fn load(esm_reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        let mut ltex = Self::default();
        *is_deleted = false;
        let mut has_name = false;
        let mut has_index = false;

        while esm_reader.has_more_subs() {
            let sub_name = esm_reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    // read_refid handles its own subrecord header reading (NAME + size)
                    // and then read its specific payload format.
                    ltex.id = esm_reader.read_refid()?;
                    has_name = true;
                }
                b"INTV" => {
                    esm_reader.read_sub_header()?;
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
                    ltex.texture_file = esm_reader.read_header_decode_str()?.to_string();
                }
                b"DELE" => {
                    esm_reader.skip_sub_data()?;
                }
                _ => {
                    // Unknown subrecord, fail
                    let msg = format!(
                        "Unknown LTEX subrecord: {} in record {}",
                        sub_name.to_string_lossy(),
                        esm_reader.context.rec_name.to_string_lossy()
                    );
                    return Err(esm_reader.fail(&msg));
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
