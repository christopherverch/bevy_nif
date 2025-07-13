// src/sound.rs

 // Using title case as requested
use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)] // For fixed size, though we'll read fields individually
pub struct SoundData {
    pub volume: u8, // 0-255
    pub min_range: u8,
    pub max_range: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sound {
    pub id: String,       // The Record ID (e.g., "Arrow Hit"), set by the caller
    pub filename: String, // From FNAM subrecord
    pub data: SoundData,  // From DATA subrecord
}

impl Sound {
    pub fn new(id: String) -> Self {
        Sound {
            id,
            ..Default::default()
        }
    }

    /// Loads a SOUN record's data from the EsmReader.
    /// The `id` (record ID) should be provided by the caller (e.g., from EsmReader::get_rec_name()).
    /// Assumes the EsmReader is positioned after the "SOUN" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader, record_id: String) -> Result<Self, EsmError> {
        let mut sound_info = Sound::new(record_id);
        let mut fnam_found = false;
        let mut data_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [SOUN DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"FNAM" => {
                    sound_info.filename = reader.get_h_string()?;
                    fnam_found = true;
                    // println!("    [SOUN DEBUG] Filename (FNAM): {}", sound_info.filename);
                }
                b"DATA" => {
                    reader.get_sub_header()?;
                    let expected_data_size = std::mem::size_of::<SoundData>() as u32;
                    if reader.get_sub_size() != expected_data_size {
                        return Err(reader
                            .report_sub_size_mismatch(expected_data_size, reader.get_sub_size()));
                    }

                    sound_info.data.volume = reader.get_t::<u8>()?;
                    sound_info.data.min_range = reader.get_t::<u8>()?;
                    sound_info.data.max_range = reader.get_t::<u8>()?;

                    data_found = true;
                    // println!("    [SOUN DEBUG] Data (DATA): {:?}", sound_info.data);
                }
                _ => {
                    // println!("    [SOUN DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !fnam_found {
            return Err(reader.fail(&format!(
                "SOUN record '{}' missing FNAM (filename) subrecord",
                sound_info.id
            )));
        }
        if !data_found {
            return Err(reader.fail(&format!(
                "SOUN record '{}' missing DATA subrecord",
                sound_info.id
            )));
        }

        Ok(sound_info)
    }
}
