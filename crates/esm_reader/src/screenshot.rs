// src/screenshot.rs

use crate::esm_reader::{EsmError, EsmReader};
// SSCR records don't have a NAME ID subrecord. They are usually one per save file.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Screenshot {
    // The DATA subrecord contains:
    // width (i32), height (i32), then width * height * 3 bytes of RGB data.
    // For simplicity, we'll store width, height, and raw image data.
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub image_data: Option<Vec<u8>>, // RGB raw pixel data
}

impl Screenshot {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut sscr = Screenshot::new();
        // SSCR records don't have a NAME ID. They are identified by their record type.

        if reader.is_next_sub(Name::new("DATA"))? {
            reader.get_sub_header()?;
            let sub_size = reader.get_sub_size();

            if sub_size < 8 {
                // Must have at least width and height (2 * i32)
                return Err(reader.fail(&format!(
                    "SSCR DATA subrecord too small (size: {})",
                    sub_size
                )));
            }

            let width = reader.get_t::<i32>()?;
            let height = reader.get_t::<i32>()?;
            sscr.width = Some(width);
            sscr.height = Some(height);

            let expected_pixel_data_size = (width as u32) * (height as u32) * 3; // RGB
            let actual_pixel_data_size = sub_size.saturating_sub(8);

            if actual_pixel_data_size == expected_pixel_data_size && width > 0 && height > 0 {
                let mut pixels = vec![0u8; expected_pixel_data_size as usize];
                reader.get_exact(&mut pixels)?;
                sscr.image_data = Some(pixels);
            } else if actual_pixel_data_size > 0 {
                // Size mismatch or invalid dimensions, skip remaining data
                // eprintln!("[SSCR WARNING] Pixel data size mismatch or invalid dimensions. Expected: {}, Actual: {}. Skipping.",
                //     expected_pixel_data_size, actual_pixel_data_size);
                reader.skip(actual_pixel_data_size)?;
            }
            // If actual_pixel_data_size is 0 but width/height > 0, image_data remains None.
        } else {
            // This is unusual, SSCR should have DATA.
            // If other subrecords exist, skip them.
            while reader.has_more_subs() {
                let _ = reader.get_sub_name()?;
                reader.skip_h_sub()?;
            }
        }
        // No mandatory ID check as SSCR doesn't have one.
        // The presence of DATA with valid dimensions is key.
        if sscr.width.is_none() || sscr.height.is_none() {
            return Err(reader.fail("SSCR record missing valid DATA subrecord for dimensions."));
        }

        Ok(sscr)
    }
}

