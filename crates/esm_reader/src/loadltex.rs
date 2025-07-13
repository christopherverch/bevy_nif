// src/loadltex.rs

use crate::common_types::Name;
use crate::esm_reader::{ConsumptionType, EsmError, EsmReader};
use crate::refid_types::RefId;
// If ESMWriter is to be implemented, its definition would be needed here or imported.
// For now, we'll stub it for the save method.
// use crate::esm_writer::EsmWriter;

// These would typically come from a defs.rs or similar, analogous to ESM::RecNameInts
// For now, we'll define them locally for clarity, or use Name::new("...")
const SREC_NAME_STR: &str = "NAME";
const SREC_INTV_STR: &str = "INTV";
const SREC_DATA_STR: &str = "DATA";
const SREC_DELE_STR: &str = "DELE";

#[derive(Debug, Default, Clone)]
pub struct LandTexture {
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

    /// Loads the LandTexture data from the ESM reader.
    /// The ESMReader is expected to be positioned at the beginning of the subrecords
    /// for an LTEX record, after the "LTEX" record header itself has been read.
    /// Returns the loaded LandTexture and a boolean indicating if it's marked as deleted.
    pub fn load(esm_reader: &mut EsmReader) -> Result<(Self, bool), EsmError> {
        let mut ltex = LandTexture::default();
        let mut is_deleted = false;
        let mut has_name = false;
        let mut has_index = false;

        while esm_reader.has_more_subs() {
            let sub_name = esm_reader.get_sub_name()?; // Consumes subrecord name

            // Compare by creating Name structs or by their u32 representation if available
            if sub_name == Name::new(SREC_NAME_STR) {
                // get_ref_id should handle its own subrecord header reading (NAME + size)
                // and then read its specific payload format.
                ltex.id = esm_reader.get_ref_id()?;
                has_name = true;
            } else if sub_name == Name::new(SREC_INTV_STR) {
                esm_reader.get_sub_header()?; // Read "INTV" subrecord size
                if esm_reader.get_sub_size() != std::mem::size_of::<u32>() as u32 {
                    return Err(esm_reader.report_sub_size_mismatch(
                        std::mem::size_of::<u32>() as u32,
                        esm_reader.get_sub_size(),
                    ));
                }
                ltex.index = esm_reader.get_u32_le(ConsumptionType::SubrecordPayload)?;
                has_index = true;
            } else if sub_name == Name::new(SREC_DATA_STR) {
                // get_h_string should handle its own subrecord header reading (DATA + size)
                ltex.texture_file = esm_reader.get_h_string()?;
            } else if sub_name == Name::new(SREC_DELE_STR) {
                // skip_sub_data should handle its own subrecord header reading (DELE + size)
                // and skip its payload.
                esm_reader.skip_sub_data()?;
                is_deleted = true;
            } else {
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

        if !has_name {
            return Err(esm_reader.fail("Missing NAME subrecord in LTEX"));
        }
        if !has_index {
            return Err(esm_reader.fail("Missing INTV subrecord in LTEX"));
        }

        Ok((ltex, is_deleted))
    }

    /// Sets the record to a default state.
    pub fn blank(&mut self) {
        self.id = RefId::new(); // Or RefId::default()
        self.texture_file.clear();
        // mIndex is explicitly not cleared in the OpenMW C++ `blank` method.
        // self.index = 0; // If you prefer to reset it.
    }

    // Placeholder for save method, as EsmWriter is not fully defined here.
    // To implement this, you would need an EsmWriter similar to your EsmReader.
    /*
    pub fn save(&self, esm_writer: &mut EsmWriter, is_deleted: bool) -> Result<(), EsmError> {
        esm_writer.write_hn_cref_id(Name::new(SREC_NAME_STR), &self.id)?;
        esm_writer.write_hnt_u32(Name::new(SREC_INTV_STR), self.index)?;
        esm_writer.write_hnc_string(Name::new(SREC_DATA_STR), &self.texture_file)?;

        if is_deleted {
            esm_writer.write_hn_string(Name::new(SREC_DELE_STR), "", 3)?; // Assuming size 3 is fixed or handled by writer
        }
        Ok(())
    }
    */
}
