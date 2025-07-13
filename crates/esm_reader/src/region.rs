// src/region.rs

 // Using title case Name as per your preference
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

// Constants for array sizes based on OpenMW's WEATstruct
const WEATHER_PROBABILITY_COUNT: usize = 10;
const LEGACY_WEATHER_PROBABILITY_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionWeatherData {
    // Clear, Cloudy, Foggy, Overcast, Rain, Thunder, Ash, Blight, Snow, Blizzard
    pub probabilities: [u8; WEATHER_PROBABILITY_COUNT],
}

impl Default for RegionWeatherData {
    fn default() -> Self {
        Self {
            probabilities: [0; WEATHER_PROBABILITY_COUNT],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundRef {
    pub sound_id: RefId,
    pub chance: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Region {
    pub id: RefId,                       // From NAME/SREC_NAME subrecord
    pub full_name: Option<String>,       // From FNAM subrecord
    pub weather_data: RegionWeatherData, // From WEAT subrecord
    pub sleep_list: Option<RefId>,       // From BNAM subrecord (optional)
    pub map_color: Option<u32>,          // From CNAM subrecord (optional, RGBA or BGRA packed)
    pub sound_list: Vec<SoundRef>,       // From SNAM subrecords (optional, multiple)
}

impl Region {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a REGN record's data from the EsmReader.
    /// Assumes the EsmReader is positioned after the "REGN" record name
    /// and its initial record header (size, flags) have been read.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut region = Region::new();
        let mut id_found = false; // For NAME subrecord
        // FNAM, WEAT are mandatory according to OpenMW's non-optional members after load.
        // Other fields like BNAM, CNAM, SNAM are optional.

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [REGN DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                // SREC_NAME in OpenMW is just b"NAME"
                b"NAME" => {
                    region.id = reader.get_ref_id()?; // Reads RefId from subrecord
                    id_found = true;
                    // println!("    [REGN DEBUG] ID (NAME): {:?}", region.id);
                }
                b"FNAM" => {
                    region.full_name = Some(reader.get_h_string()?);
                    // println!("    [REGN DEBUG] Full Name (FNAM): {:?}", region.full_name);
                }
                b"WEAT" => {
                    reader.get_sub_header()?;
                    let sub_size = reader.get_sub_size();
                    if sub_size == WEATHER_PROBABILITY_COUNT as u32 {
                        for i in 0..WEATHER_PROBABILITY_COUNT {
                            region.weather_data.probabilities[i] = reader.get_t::<u8>()?;
                        }
                    } else if sub_size == LEGACY_WEATHER_PROBABILITY_COUNT as u32 {
                        // Read the first 8, last 2 (Snow, Blizzard) will remain 0 from default
                        for i in 0..LEGACY_WEATHER_PROBABILITY_COUNT {
                            region.weather_data.probabilities[i] = reader.get_t::<u8>()?;
                        }
                        // The remaining bytes in the subrecord (if any, though it should be exact) are implicitly skipped
                        // if get_t only reads 8 bytes and sub_size was 8.
                        // If reader.get_sub_size() was strictly checked against individual get_t calls,
                        // a loop would be better:
                        // for i in 0..sub_size as usize {
                        //     if i < WEATHER_PROBABILITY_COUNT {
                        //          region.weather_data.probabilities[i] = reader.get_t::<u8>()?;
                        //     } else { reader.get_t::<u8>()?; /* read and discard if somehow > 10 */ }
                        // }
                        // The OpenMW code `esm.getExact(&mData.mProbabilities, esm.getSubSize());` is simpler.
                        // Let's stick to reading exactly sub_size bytes if it's 8 or 10.
                    } else {
                        return Err(reader.fail(&format!(
                            "REGN record '{}': WEAT subrecord has unexpected size {} (expected {} or {})",
                            region.id.to_debug_string(), sub_size, WEATHER_PROBABILITY_COUNT, LEGACY_WEATHER_PROBABILITY_COUNT
                        )));
                    }
                    // println!("    [REGN DEBUG] Weather (WEAT): {:?}", region.weather_data.probabilities);
                }
                b"BNAM" => {
                    region.sleep_list = Some(reader.get_ref_id()?);
                    // println!("    [REGN DEBUG] Sleep List (BNAM): {:?}", region.sleep_list);
                }
                b"CNAM" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        // Expecting 4 bytes for int32_t mMapColor (RGBA)
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    region.map_color = Some(reader.get_t::<u32>()?); // Read as u32
                    // println!("    [REGN DEBUG] Map Color (CNAM): {:?}", region.map_color);
                }
                b"SNAM" => {
                    reader.get_sub_header()?;
                    // SNAM size = sizeof(RefId, potentially up to 32 for string part) + sizeof(u8 chance)
                    // This is tricky because RefId size can vary.
                    // OpenMW: sr.mSound = esm.getMaybeFixedRefIdSize(32); esm.getT(sr.mChance);
                    // getMaybeFixedRefIdSize needs to be implemented carefully.
                    // For now, let's assume get_ref_id reads a standard RefId structure,
                    // and then we read the chance. This implies the SNAM subrecord contains
                    // a serialized RefId followed immediately by a u8.
                    // The size of the serialized RefId itself is variable.
                    // So, get_sub_header() gives total size of (RefId_bytes + u8_byte).
                    let total_snam_data_size = reader.get_sub_size();
                    if total_snam_data_size < 1 {
                        // Must have at least chance byte
                        return Err(reader.fail("SNAM subrecord too small"));
                    }

                    // This is a simplification. Reading a variable-size RefId and then a u8
                    // from a subrecord whose total size is `total_snam_data_size` is complex.
                    // OpenMW's `getMaybeFixedRefIdSize(32)` probably means it reads a RefId,
                    // and if it's a string type, it's capped at 32 bytes for the string part.
                    // The `ESMReader::get_ref_id_impl(size)` is more appropriate here if we know
                    // the size of the RefId part.
                    // For now, assuming `get_ref_id()` correctly parses a self-contained RefId structure
                    // from the stream, and the next byte is the chance.
                    // This might fail if RefId doesn't consume exactly total_snam_data_size - 1.
                    // A more robust way: read total_snam_data_size bytes into a buffer,
                    // then parse RefId from start, then u8 from end. Or, if RefId is always small and fixed size
                    // when stored like this (e.g. just a FormId).
                    // OpenMW's esm.getMaybeFixedRefIdSize(32) suggests the RefId string itself is 32 bytes max,
                    // not the whole subrecord.
                    // Let's assume the SNAM subrecord contains *only* the RefId string (max 32, null term)
                    // and then a separate DATA u8 subrecord for chance, *OR* that SNAM contains both.
                    // The OpenMW loop for SNAM `esm.startSubRecord("SNAM"); esm.writeMaybeFixedSizeRefId(...); esm.writeT(...); esm.endRecord("SNAM");`
                    // implies a single SNAM subrecord contains both.

                    // Let's assume the RefId is read first, then the chance.
                    // This requires `get_ref_id` to not consume the entire `total_snam_data_size`.
                    // This is problematic. A better approach for SNAM:
                    // The SNAM *subrecord data* is `RefId + u8 chance`.
                    // `get_maybe_fixed_ref_id_size(32)` is a specific string reading variant.
                    // If it's a generic RefId, we'd need to read it.
                    // This part is hard to translate directly without seeing `getMaybeFixedRefIdSize`.
                    // For now, let's try to read a RefId (which reads its own varying length based on its type byte)
                    // and then a u8. This is risky if the RefId read consumes the chance byte.

                    // A simpler assumption, though likely incorrect for OpenMW's SoundRef:
                    // If SNAM was just the RefId string, and a *separate* subrecord (e.g. SCHC for SoundChance) held the byte.
                    // But OpenMW C++ shows SNAM contains both.
                    // This means `esm.getMaybeFixedRefIdSize(32)` reads a string up to 32 bytes
                    // and that becomes the string part of a RefId, then `esm.getT(sr.mChance)` reads the next byte.
                    // The `getSubHeader()` for SNAM would have set `leftSub` to `size_of_string_part + 1`.

                    // Let's attempt to parse as OpenMW does: a string for ID (capped), then a u8.
                    // We don't have `getMaybeFixedRefIdSize` yet.
                    // We can use `get_string_view_internal` if we know the string part size.
                    // size of string = total_snam_data_size - 1 (for u8 chance)
                    if total_snam_data_size < 1 {
                        // Must be at least 1 for the chance
                        return Err(reader.fail(&format!(
                            "SNAM subrecord for {} too small ({})",
                            region.id.to_debug_string(),
                            total_snam_data_size
                        )));
                    }
                    let string_part_size = total_snam_data_size as usize - 1;
                    let sound_id_str = reader.get_string_view_internal(string_part_size)?;
                    let chance = reader.get_t::<u8>()?;
                    region.sound_list.push(SoundRef {
                        sound_id: RefId::string_ref_id(&sound_id_str), // Create RefId from the string
                        chance,
                    });
                }
                _ => {
                    // println!("    [REGN DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found {
            return Err(reader.fail("REGN record missing NAME subrecord"));
        }
        // FNAM and WEAT are considered mandatory by OpenMW struct members, but loading might proceed.
        if region.full_name.is_none() {
            return Err(reader.fail(&format!(
                "REGN record '{}' missing FNAM (full name) subrecord",
                region.id.to_debug_string()
            )));
        }
        // Check if weather data was actually read (not strictly done in OpenMW load, relies on subrecord presence)
        // but our logic for WEAT already handles sizes.

        Ok(region)
    }
}

