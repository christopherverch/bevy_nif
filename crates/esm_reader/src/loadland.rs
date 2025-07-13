//! Module for loading and representing LAND (landscape) records from an ESM file.
//! This version implements a two-stage loading process similar to OpenMW:
//! 1. Initial read: Parses essential data (grid coords, flags) and identifies
//!    the presence and declared sizes of large data subrecords (VNML, VHGT, etc.),
//!    but defers loading their actual content.
//! 2. Data loading: A separate method to load the actual large data arrays when needed.

use crate::common_types::EsmContext;
use crate::esm_reader::{ConsumptionType, EsmError, EsmReader}; // Assuming EsmContext is your context struct

// --- Constants for LAND record structure ---
pub const LAND_SIDE_VERTICES: usize = 65;
pub const LAND_NUM_VERTS: usize = LAND_SIDE_VERTICES * LAND_SIDE_VERTICES;
pub const LAND_TEXTURE_SQUARES_PER_SIDE: usize = 16;
pub const LAND_NUM_TEXTURES: usize = LAND_TEXTURE_SQUARES_PER_SIDE * LAND_TEXTURE_SQUARES_PER_SIDE;
pub const LAND_VTEX_TEXTURE_PALETTE_SIZE: usize = LAND_NUM_TEXTURES * 2;
pub const VNML_PAYLOAD_SIZE: usize = LAND_NUM_VERTS * 3;
pub const VHGT_PAYLOAD_SIZE: usize = std::mem::size_of::<f32>() + LAND_NUM_VERTS + 3;
pub const WNAM_PAYLOAD_SIZE: usize = 81;
pub const MORROWIND_HEIGHT_SCALE: f32 = 0.1;
pub const VCLR_PAYLOAD_SIZE: usize = LAND_NUM_VERTS * 3;
pub const LAND_INTV_PAYLOAD_SIZE: usize = std::mem::size_of::<i32>() * 2;
pub const LAND_DATA_PAYLOAD_SIZE: usize = std::mem::size_of::<u32>();
const VTEX_GRID_SIDE: usize = 16; // The final grid is 16x16
const VTEX_CHUNK_SIDE: usize = 4; // Data is in 4x4 chunks

// --- Data Structures for LAND record ---

#[derive(Debug, Clone, Copy, Default)]
pub struct LandVertexNormal {
    pub x: i8,
    pub y: i8,
    pub z: i8,
}

#[derive(Debug, Clone)]
pub struct LandVertexHeightData {
    pub heights: Vec<f32>,
}

impl Default for LandVertexHeightData {
    fn default() -> Self {
        Self {
            heights: vec![0.0; LAND_NUM_VERTS],
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LandVertexColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Flags indicating which optional data types are present in this LAND record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LandDataTypes(pub u32);

impl LandDataTypes {
    pub const NONE: Self = Self(0);
    pub const VNML: Self = Self(1 << 0); // Vertex Normals
    pub const VHGT: Self = Self(1 << 1); // Vertex Heights
    pub const WNAM: Self = Self(1 << 2); // World Map / Blending Data
    pub const VCLR: Self = Self(1 << 3); // Vertex Colors
    pub const VTEX: Self = Self(1 << 4); // Texture Palette Indices

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl Default for LandDataTypes {
    fn default() -> Self {
        Self::NONE
    }
}

impl std::ops::BitOrAssign for LandDataTypes {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Holds the actual large data arrays for a LAND record, loaded on demand.
#[derive(Debug, Clone, Default)]
pub struct LandPayloadData {
    pub vertex_normals: Option<Box<[LandVertexNormal; LAND_NUM_VERTS]>>,
    pub vertex_heights: Option<LandVertexHeightData>,
    pub wnam_global_map_data: Option<[i8; 81]>,
    pub vertex_colors: Option<Box<[LandVertexColor; LAND_NUM_VERTS]>>,
}

#[derive(Debug, Clone, Default)]
pub struct Landscape {
    pub grid_x: i32,
    pub grid_y: i32,
    pub data_flags_from_esm: Option<u32>, // Flags read from the DATA subrecord

    /// Bitflags indicating which large data subrecords (VNML, VHGT, etc.) are present
    /// according to the LAND.DATA flags and were found during initial scan.
    pub present_data_types: LandDataTypes,

    /// Stores the context (file position) of the LAND record's subrecords.
    /// This is used to re-read and load detailed data later.
    pub record_context: Option<EsmContext>, // Store the reader's context

    /// Holds the actual large data payloads, loaded on demand via `load_detailed_data`.
    /// Kept in an Option to allow for lazy loading.
    pub detailed_data: Option<Box<LandPayloadData>>,

    // WNAM is small enough that OpenMW loads it directly in the first pass.
    // We can choose to do the same or make it part of LandPayloadData.
    // For consistency with OpenMW's Land::mWnam, let's keep it separate for now.
    pub wnam_data_direct: Option<Box<[i8; WNAM_PAYLOAD_SIZE]>>,
    pub texture_indices: Option<Box<[u16; LAND_NUM_TEXTURES]>>,
}

impl Landscape {
    pub fn new() -> Self {
        Default::default()
    }

    /// Performs the initial read of the LAND record.
    /// Parses INTV and DATA subrecords directly.
    /// For large data subrecords (VNML, VHGT, VCLR, VTEX), it records their presence
    /// (based on DATA flags) and skips their content.
    /// WNAM is read directly if present and indicated by DATA flags.
    /// Stores the reader context to allow for later loading of detailed data.
    pub fn load(reader: &mut EsmReader) -> Result<(Self, bool), EsmError> {
        let mut land = Landscape::new();
        let mut is_deleted = false;
        let mut found_intv = false;

        // Store the context at the beginning of this LAND record's subrecords
        // This allows us to return later to load detailed data.
        // get_context() should capture the current file_pos.
        land.record_context = Some(reader.get_context_for_deferred_load()?);

        // --- Pass 1: Read INTV and DATA, identify presence of other subrecords ---
        // This loop mimics OpenMW's first pass in Land::load
        while reader.has_more_subs() {
            // Get the name of the next subrecord. This reads 4 bytes (name) and updates
            // file_pos, rem_file_data, rem_rec_data by 4.
            // context.sub_name is set. context.sub_cached is now false.
            let current_sub_name = reader.get_sub_name()?;

            // Now get the size for this subrecord. This reads 4 bytes (size) and updates
            // file_pos, rem_file_data, rem_rec_data by 4.
            // context.remaining_subrec_data is set to the payload size.
            // This specific get_sub_header does not use/set sub_cached.
            reader.get_sub_header()?; // Your version from prompt 017

            match current_sub_name.as_bytes() {
                b"INTV" => {
                    if reader.get_sub_size() as usize != LAND_INTV_PAYLOAD_SIZE { /* error */ }
                    land.grid_x = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
                    land.grid_y = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
                    found_intv = true;
                    // All payload consumed, remaining_subrec_data should be 0
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                b"DATA" => {
                    if reader.get_sub_size() as usize != LAND_DATA_PAYLOAD_SIZE { /* error */ }
                    let flags = reader.get_u32_le(ConsumptionType::SubrecordPayload)?;
                    land.data_flags_from_esm = Some(flags);
                    if (flags & 0x1) != 0 {
                        land.present_data_types |= LandDataTypes::VNML;
                        land.present_data_types |= LandDataTypes::VHGT;
                        land.present_data_types |= LandDataTypes::WNAM;
                    }
                    if (flags & 0x2) != 0 {
                        land.present_data_types |= LandDataTypes::VCLR;
                    }
                    if (flags & 0x4) != 0 {
                        land.present_data_types |= LandDataTypes::VTEX;
                    }
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                b"DELE" => {
                    // The DELE subrecord means this LAND record should be ignored.
                    is_deleted = true;
                    // We still need to skip its payload to advance the reader correctly.
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                }
                b"VNML" if land.present_data_types.contains(LandDataTypes::VNML) => {
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    // For initial load, we skip based on *declared size*.
                    // The error "Expected 12675, got 0" happens if the file *declares* 0.
                    // So, if it declares 0, we skip 0.
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                b"VHGT" if land.present_data_types.contains(LandDataTypes::VHGT) => {
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                b"WNAM" if land.present_data_types.contains(LandDataTypes::WNAM) => {
                    // OpenMW reads WNAM directly in Land::load.
                    if reader.get_sub_size() as usize != WNAM_PAYLOAD_SIZE { /* error */ }
                    let mut wnam_array = Box::new([0i8; WNAM_PAYLOAD_SIZE]);
                    let u8_slice_for_read: &mut [u8] = unsafe {
                        // Get a mutable slice to the data within the Box: &mut [i8; WNAM_PAYLOAD_SIZE]
                        let i8_array_slice_mut: &mut [i8; WNAM_PAYLOAD_SIZE] = &mut *wnam_array;
                        // Get the raw mutable pointer to the start of the i8 slice.
                        let ptr_i8_mut = i8_array_slice_mut.as_mut_ptr();
                        // Cast this pointer to a *mut u8.
                        let ptr_u8_mut = ptr_i8_mut as *mut u8;
                        // Create a new mutable slice of u8 from the raw pointer and length.
                        // The length is WNAM_PAYLOAD_SIZE because each i8 is 1 byte.
                        std::slice::from_raw_parts_mut(ptr_u8_mut, WNAM_PAYLOAD_SIZE)
                    };
                    reader.get_exact(
                        &mut u8_slice_for_read[..],
                        ConsumptionType::SubrecordPayload,
                    )?;
                    land.wnam_data_direct = Some(wnam_array);
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                b"VCLR" if land.present_data_types.contains(LandDataTypes::VCLR) => {
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                b"VTEX" if land.present_data_types.contains(LandDataTypes::VTEX) => {
                    if land.present_data_types.contains(LandDataTypes::VTEX) {
                        const VTEX_ACTUAL_PAYLOAD_SIZE: usize =
                            LAND_NUM_TEXTURES * std::mem::size_of::<u16>();
                        if reader.get_sub_size() as usize == VTEX_ACTUAL_PAYLOAD_SIZE {
                            let mut chunked_textures_array = [0u16; LAND_NUM_TEXTURES];
                            for i in 0..LAND_NUM_TEXTURES {
                                chunked_textures_array[i] =
                                    reader.get_u16_le(ConsumptionType::SubrecordPayload)?;
                            }
                            //currently the data is stored in weird 4x4 chunks, we need to un-chunk
                            //it
                            let transposed_textures_array =
                                transpose_vtex_indices(&chunked_textures_array);
                            land.texture_indices = Some(Box::new(transposed_textures_array));
                        } else if reader.get_sub_size() > 0 {
                            eprintln!(
                                "WARN: VTEX size mismatch for LAND ({},{}). Expected {}, got {}. Skipping.",
                                land.grid_x,
                                land.grid_y,
                                VTEX_ACTUAL_PAYLOAD_SIZE,
                                reader.get_sub_size()
                            );
                            let payload_to_skip = reader.context.remaining_subrec_data;
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    } else {
                        let payload_to_skip = reader.context.remaining_subrec_data;
                        if payload_to_skip > 0 {
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    }
                }
                _ => {
                    // Unknown or unexpected subrecord
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
            }
            // After processing any subrecord, `sub_cached` is false (from `get_sub_name`)
            // and `remaining_subrec_data` is 0 (because payload was read/skipped
            // and `skip_raw_bytes` or `get_exact` with `SubrecordPayload` zeroed it).
        }

        if !found_intv { /* error */ }
        Ok((land, is_deleted))
    }
    pub fn load_detailed_data(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        let context_to_restore = self.record_context.as_ref().cloned().ok_or_else(|| {
            EsmError::Custom(
                "Cannot load detailed data without a stored record context.".to_string(),
            )
        })?;

        if self.detailed_data.is_none() {
            self.detailed_data = Some(Box::new(LandPayloadData::default()));
        }
        // This unwrap is safe because we just ensured it's Some.
        let payload_data = self.detailed_data.as_mut().unwrap();

        // Restore the reader's state to where the subrecords for this LAND begin.
        reader.restore_context(&context_to_restore)?;

        // Iterate through the subrecords again, this time loading the data.
        while reader.has_more_subs() {
            let current_sub_name = reader.get_sub_name()?;
            reader.get_sub_header()?; // Reads size, sets reader.context.remaining_subrec_data

            match current_sub_name.as_bytes() {
                b"INTV" | b"DATA" | b"DELE" => {
                    // These were handled in the initial load or are not part of the detailed_data payload.
                    // Their payload must be skipped to correctly process subsequent subrecords.
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                }
                b"VNML" => {
                    if self.present_data_types.contains(LandDataTypes::VNML) {
                        if reader.get_sub_size() as usize == VNML_PAYLOAD_SIZE {
                            let mut normals_array =
                                Box::new([LandVertexNormal::default(); LAND_NUM_VERTS]);
                            // Unsafe cast to read into the array of structs as a byte slice.
                            // This is generally safe if LandVertexNormal is #[repr(C)] and has no padding issues.
                            let buffer: &mut [u8] = unsafe {
                                std::slice::from_raw_parts_mut(
                                    normals_array.as_mut_ptr() as *mut u8,
                                    VNML_PAYLOAD_SIZE,
                                )
                            };
                            reader.get_exact(buffer, ConsumptionType::SubrecordPayload)?;
                            payload_data.vertex_normals = Some(normals_array);
                        } else if reader.get_sub_size() > 0 {
                            // Size mismatch, but data exists. Skip to avoid errors.
                            eprintln!(
                                "WARN: VNML size mismatch for LAND ({},{}). Expected {}, got {}. Skipping.",
                                self.grid_x,
                                self.grid_y,
                                VNML_PAYLOAD_SIZE,
                                reader.get_sub_size()
                            );
                            let payload_to_skip = reader.context.remaining_subrec_data;
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                        // If get_sub_size() is 0, remaining_subrec_data is already 0.
                    } else {
                        // VNML subrecord present, but not indicated by DATA flags. Skip its payload.
                        let payload_to_skip = reader.context.remaining_subrec_data;
                        if payload_to_skip > 0 {
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    }
                }
                b"VHGT" => {
                    if self.present_data_types.contains(LandDataTypes::VHGT) {
                        if reader.get_sub_size() as usize == VHGT_PAYLOAD_SIZE {
                            // --- This is the new decoding logic, moved from your Bevy app ---
                            let height_offset =
                                reader.get_f32_le(ConsumptionType::SubrecordPayload)?;

                            // Read the raw i8 data into a temporary buffer
                            let mut raw_height_data = [0i8; LAND_NUM_VERTS];
                            let buffer: &mut [u8] = unsafe {
                                std::slice::from_raw_parts_mut(
                                    raw_height_data.as_mut_ptr() as *mut u8,
                                    LAND_NUM_VERTS,
                                )
                            };
                            reader.get_exact(buffer, ConsumptionType::SubrecordPayload)?;
                            reader.skip_raw_bytes(3, ConsumptionType::SubrecordPayload)?; // Skip padding

                            // Now, perform the decoding immediately
                            let mut decoded_heights = vec![0.0f32; LAND_NUM_VERTS];
                            let mut current_z_at_first_col_unscaled = height_offset;

                            for y_idx in 0..LAND_SIDE_VERTICES {
                                current_z_at_first_col_unscaled +=
                                    raw_height_data[y_idx * LAND_SIDE_VERTICES] as f32;
                                decoded_heights[y_idx * LAND_SIDE_VERTICES] =
                                    current_z_at_first_col_unscaled * MORROWIND_HEIGHT_SCALE;

                                let mut current_z_in_row_unscaled = current_z_at_first_col_unscaled;
                                for x_idx in 1..LAND_SIDE_VERTICES {
                                    current_z_in_row_unscaled +=
                                        raw_height_data[y_idx * LAND_SIDE_VERTICES + x_idx] as f32;
                                    decoded_heights[y_idx * LAND_SIDE_VERTICES + x_idx] =
                                        current_z_in_row_unscaled * MORROWIND_HEIGHT_SCALE;
                                }
                            }

                            // Store the final, decoded data
                            payload_data.vertex_heights = Some(LandVertexHeightData {
                                heights: decoded_heights,
                            });
                            // --- End of new decoding logic ---
                        } else if reader.get_sub_size() > 0 {
                            eprintln!(
                                "WARN: VHGT size mismatch for LAND ({},{}). Expected {}, got {}. Skipping.",
                                self.grid_x,
                                self.grid_y,
                                VHGT_PAYLOAD_SIZE,
                                reader.get_sub_size()
                            );
                            let payload_to_skip = reader.context.remaining_subrec_data;
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    } else {
                        let payload_to_skip = reader.context.remaining_subrec_data;
                        if payload_to_skip > 0 {
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    }
                }
                b"WNAM" => {
                    // WNAM is special: Landscape::load already attempts to load it into self.wnam_data_direct.
                    // Here, we can choose to populate payload_data.wnam_global_map_data as well.
                    if self.present_data_types.contains(LandDataTypes::WNAM) {
                        if reader.get_sub_size() as usize == WNAM_PAYLOAD_SIZE {
                            // If wnam_data_direct was successfully loaded, we could copy from there.
                            // However, to keep LandPayloadData self-contained from the file read pass:
                            let mut wnam_array = [0i8; WNAM_PAYLOAD_SIZE];
                            let u8_slice_for_read: &mut [u8] = unsafe {
                                std::slice::from_raw_parts_mut(
                                    wnam_array.as_mut_ptr() as *mut u8,
                                    WNAM_PAYLOAD_SIZE,
                                )
                            };
                            reader
                                .get_exact(u8_slice_for_read, ConsumptionType::SubrecordPayload)?;
                            payload_data.wnam_global_map_data = Some(wnam_array);

                            // Verify against already loaded wnam_data_direct if desired (optional debug check)
                            // if let Some(existing_wnam_box) = &self.wnam_data_direct {
                            //     if &wnam_array[..] != &existing_wnam_box[..] {
                            //         eprintln!("WARN: WNAM data re-read differs from initially loaded for LAND ({},{})", self.grid_x, self.grid_y);
                            //     }
                            // }
                        } else if reader.get_sub_size() > 0 {
                            eprintln!(
                                "WARN: WNAM size mismatch for LAND ({},{}). Expected {}, got {}. Skipping.",
                                self.grid_x,
                                self.grid_y,
                                WNAM_PAYLOAD_SIZE,
                                reader.get_sub_size()
                            );
                            let payload_to_skip = reader.context.remaining_subrec_data;
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    } else {
                        let payload_to_skip = reader.context.remaining_subrec_data;
                        if payload_to_skip > 0 {
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    }
                }
                b"VCLR" => {
                    if self.present_data_types.contains(LandDataTypes::VCLR) {
                        if reader.get_sub_size() as usize == VCLR_PAYLOAD_SIZE {
                            let mut colors_array =
                                Box::new([LandVertexColor::default(); LAND_NUM_VERTS]);
                            let buffer: &mut [u8] = unsafe {
                                std::slice::from_raw_parts_mut(
                                    colors_array.as_mut_ptr() as *mut u8,
                                    VCLR_PAYLOAD_SIZE,
                                )
                            };
                            reader.get_exact(buffer, ConsumptionType::SubrecordPayload)?;
                            payload_data.vertex_colors = Some(colors_array);
                        } else if reader.get_sub_size() > 0 {
                            eprintln!(
                                "WARN: VCLR size mismatch for LAND ({},{}). Expected {}, got {}. Skipping.",
                                self.grid_x,
                                self.grid_y,
                                VCLR_PAYLOAD_SIZE,
                                reader.get_sub_size()
                            );
                            let payload_to_skip = reader.context.remaining_subrec_data;
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    } else {
                        let payload_to_skip = reader.context.remaining_subrec_data;
                        if payload_to_skip > 0 {
                            reader.skip_raw_bytes(
                                payload_to_skip as i64,
                                ConsumptionType::SubrecordPayload,
                            )?;
                        }
                    }
                }
                b"VTEX" if self.present_data_types.contains(LandDataTypes::VTEX) => {
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                    debug_assert!(reader.context.remaining_subrec_data == 0);
                }
                _ => {
                    // Unknown subrecord or one not handled for detailed loading.
                    // Its payload must be skipped to advance the reader correctly.
                    let payload_to_skip = reader.context.remaining_subrec_data;
                    if payload_to_skip > 0 {
                        eprintln!(
                            "INFO: Skipping unexpected/unhandled subrecord '{}' of size {} in LAND ({},{}) detailed load.",
                            String::from_utf8_lossy(current_sub_name.as_bytes()),
                            payload_to_skip,
                            self.grid_x,
                            self.grid_y
                        );
                        reader.skip_raw_bytes(
                            payload_to_skip as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                }
            }
            // Ensure the subrecord payload was fully consumed or skipped
            debug_assert!(reader.context.remaining_subrec_data == 0);
        }
        Ok(())
    }
}
fn transpose_vtex_indices(input_chunked: &[u16; LAND_NUM_TEXTURES]) -> [u16; LAND_NUM_TEXTURES] {
    let mut output_linear = [0u16; LAND_NUM_TEXTURES];
    let mut read_pos = 0;

    for y1 in 0..VTEX_CHUNK_SIDE {
        // Major block row
        for x1 in 0..VTEX_CHUNK_SIDE {
            // Major block column
            for y2 in 0..VTEX_CHUNK_SIDE {
                // Minor row within block
                for x2 in 0..VTEX_CHUNK_SIDE {
                    // Minor column within block
                    let global_y = y1 * VTEX_CHUNK_SIDE + y2;
                    let global_x = x1 * VTEX_CHUNK_SIDE + x2;

                    // Ensure we don't write out of bounds if logic were ever faulty,
                    // though with correct constants, this should always be true.
                    if global_y < VTEX_GRID_SIDE
                        && global_x < VTEX_GRID_SIDE
                        && read_pos < LAND_NUM_TEXTURES
                    {
                        output_linear[global_y * VTEX_GRID_SIDE + global_x] =
                            input_chunked[read_pos];
                    }
                    read_pos += 1;
                }
            }
        }
    }
    // A check to ensure the logic processed all expected elements.
    // If this assertion fails, there's an issue with the loop bounds or constants.
    assert_eq!(
        read_pos, LAND_NUM_TEXTURES,
        "Transpose logic did not process all texture indices"
    );
    output_linear
}
