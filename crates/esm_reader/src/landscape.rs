// src/landscape.rs (Rewritten to align with OpenMW C++ loading strategy)

use crate::common_types::{EsmContext, Name};
use crate::esm_reader::{EsmError, EsmReader}; // Assuming your revised EsmReader

// Constants from OpenMW's LandRecordData and Land
pub const LAND_SIZE: usize = 65;
pub const CELL_VERTEX_SPACING: f32 = 8192.0 / (LAND_SIZE as f32 - 1.0);
pub const LAND_NUM_VERTS: usize = LAND_SIZE * LAND_SIZE;
pub const LAND_TEXTURE_SIZE: usize = 16;
pub const LAND_NUM_TEXTURES: usize = LAND_TEXTURE_SIZE * LAND_TEXTURE_SIZE;
pub const GLOBAL_MAP_LOD_SIZE_SQRT: usize = 9;
pub const GLOBAL_MAP_LOD_SIZE: usize = GLOBAL_MAP_LOD_SIZE_SQRT * GLOBAL_MAP_LOD_SIZE_SQRT;
pub const HEIGHT_SCALE: f32 = 8.0;
pub const DEFAULT_HEIGHT_VALUE: f32 = -2048.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LandDataPresenceFlags(pub u32);
impl LandDataPresenceFlags {
    pub const NONE: u32 = 0;
    pub const VNML: u32 = 1 << 0; // Normals
    pub const VHGT: u32 = 1 << 1; // Heights
    pub const WNAM: u32 = 1 << 2; // World Map Low-LOD Heights
    pub const VCLR: u32 = 1 << 3; // Vertex Colors
    pub const VTEX: u32 = 1 << 4; // Texture Indices
    // pub const ALL_DETAILED: u32 = Self::VNML | Self::VHGT | Self::VCLR | Self::VTEX;
}

// Flags read from the LAND record's "DATA" subrecord
pub mod land_file_content_flags {
    pub const HEIGHTS_AND_NORMALS_PRESENT: u32 = 0x1; // Bit 0
    pub const COLORS_PRESENT: u32 = 0x2; // Bit 1
    pub const TEXTURES_PRESENT: u32 = 0x4; // Bit 2
}

#[derive(Debug, Clone, PartialEq)]
pub struct LandDetailedData {
    pub heights: Option<Box<[f32; LAND_NUM_VERTS]>>,
    pub normals: Option<Box<[[u8; 3]; LAND_NUM_VERTS]>>,
    pub colors: Option<Box<[[u8; 3]; LAND_NUM_VERTS]>>,
    pub textures: Option<Box<[u16; LAND_NUM_TEXTURES]>>,
    pub min_height: f32,
    pub max_height: f32,
    pub data_loaded_mask: LandDataPresenceFlags,
}

impl Default for LandDetailedData {
    fn default() -> Self {
        Self {
            heights: None,
            normals: None,
            colors: None,
            textures: None,
            min_height: DEFAULT_HEIGHT_VALUE,
            max_height: DEFAULT_HEIGHT_VALUE,
            data_loaded_mask: LandDataPresenceFlags(LandDataPresenceFlags::NONE),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Landscape {
    pub grid_x: i32,
    pub grid_y: i32,
    pub file_data_content_flags: u32, // Flags from "DATA" subrecord
    pub wnam_low_lod_heights: Option<[i8; GLOBAL_MAP_LOD_SIZE]>,
    pub data_chunks_context: Option<EsmContext>, // Points to start of VNML/VHGT etc.
    pub data_chunks_available_in_file: LandDataPresenceFlags, // Based on "DATA" flags and subrecord presence
    pub detailed_data: Option<Box<LandDetailedData>>,
}

fn transpose_texture_data_rust(
    flat_vtex: &[u16; LAND_NUM_TEXTURES],
) -> Box<[u16; LAND_NUM_TEXTURES]> {
    let mut out_vtex = Box::new([0u16; LAND_NUM_TEXTURES]);
    let mut read_pos = 0;
    for y1 in 0..4 {
        for x1 in 0..4 {
            for y2 in 0..4 {
                for x2 in 0..4 {
                    let out_idx = (y1 * 4 + y2) * LAND_TEXTURE_SIZE + (x1 * 4 + x2);
                    if read_pos < LAND_NUM_TEXTURES && out_idx < LAND_NUM_TEXTURES {
                        out_vtex[out_idx] = flat_vtex[read_pos];
                    }
                    read_pos += 1;
                }
            }
        }
    }
    out_vtex
}

impl Landscape {
    pub fn new() -> Self {
        Default::default()
    }

    /// Phase 1: Loads INTV, DATA.
    /// Phase 2: Iterates remaining subrecords (VNML, VHGT, WNAM, VCLR, VTEX).
    ///          Loads WNAM directly. For others, skips content but sets availability
    ///          flags in `data_chunks_available_in_file` based on `file_data_content_flags`.
    ///          Saves context for deferred loading of detailed chunks.
    pub fn load_headers(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut land = Landscape::new();
        let mut intv_found = false;
        let mut data_sub_found = false;

        // --- Phase 1: Load INTV, DATA ---
        'initial_header_loop: loop {
            if !reader.has_more_subs() {
                break 'initial_header_loop;
            }
            // Peek to decide if it's INTV, DATA, or start of data chunks
            let next_sub_name = reader.ret_sub_name(); // Get currently cached name if any
            let is_cached = reader.m_ctx.sub_cached;

            let sub_to_match = if is_cached {
                next_sub_name
            } else if reader.m_ctx.left_rec >= Name::S_CAPACITY as i64 {
                // Peek by reading then caching for this loop's decision making
                let temp_name = reader.get_sub_name()?; // Reads from stream, consumes from left_rec
                reader.cache_sub_name(); // Put it back for actual processing by get_sub_name_is
                temp_name
            } else {
                break 'initial_header_loop; // Not enough data for even a subrecord name
            };

            match &sub_to_match.m_data {
                b"INTV" => {
                    reader.get_sub_name_is(Name { m_data: *b"INTV" })?;
                    reader.get_sub_header()?;
                    if reader.get_sub_data_size() == 8 {
                        land.grid_x = reader.get_t_consuming::<i32>()?;
                        land.grid_y = reader.get_t_consuming::<i32>()?;
                    } else if reader.get_sub_data_size() == 12 {
                        land.grid_x = reader.get_t_consuming::<i32>()?;
                        land.grid_y = reader.get_t_consuming::<i32>()?;
                        let _plugin_idx = reader.get_t_consuming::<i32>()?;
                    } else {
                        return Err(reader.report_sub_size_mismatch(8, reader.get_sub_data_size()));
                    }
                    intv_found = true;
                }
                b"DATA" => {
                    reader.get_sub_name_is(Name { m_data: *b"DATA" })?;
                    reader.get_sub_header()?;
                    if reader.get_sub_data_size() == 4 {
                        land.file_data_content_flags = reader.get_t_consuming::<u32>()?;
                    } else {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_data_size()));
                    }
                    data_sub_found = true;
                }
                b"DELE" => {
                    // Should ideally be handled by the top-level record iteration
                    reader.get_sub_name_is(Name { m_data: *b"DELE" })?;
                    reader.skip_h_sub()?;
                }
                _ => {
                    // Start of data chunks or unknown
                    break 'initial_header_loop;
                }
            }
        }

        if !intv_found {
            return Err(reader.fail("LAND record missing INTV subrecord"));
        }
        if !data_sub_found {
            return Err(reader.fail("LAND record missing DATA (flags) subrecord"));
        }

        // Save context: Reader is now positioned at the start of the first detailed data chunk
        // (or where it was left by the last successful get_sub_name() that was then cached).
        land.data_chunks_context = Some(reader.get_context_for_deferred_load()?);

        // --- Phase 2: Determine available data types and load WNAM ---
        let mut available_mask_val = LandDataPresenceFlags::NONE;
        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?; // Consumes name from stream/cache, updates left_rec
            match &sub_name.m_data {
                b"VNML" => {
                    reader.skip_h_sub()?; // Skips header and payload
                    if (land.file_data_content_flags
                        & land_file_content_flags::HEIGHTS_AND_NORMALS_PRESENT)
                        != 0
                    {
                        available_mask_val |= LandDataPresenceFlags::VNML;
                    }
                }
                b"VHGT" => {
                    reader.skip_h_sub()?;
                    if (land.file_data_content_flags
                        & land_file_content_flags::HEIGHTS_AND_NORMALS_PRESENT)
                        != 0
                    {
                        available_mask_val |= LandDataPresenceFlags::VHGT;
                    }
                }
                b"WNAM" => {
                    reader.get_sub_header()?; // Read WNAM's header
                    let expected_size = GLOBAL_MAP_LOD_SIZE as u32;
                    if reader.get_sub_data_size() == expected_size {
                        let mut wnam_data_arr = [0i8; GLOBAL_MAP_LOD_SIZE];
                        let buffer_view: &mut [u8] = unsafe {
                            std::slice::from_raw_parts_mut(
                                wnam_data_arr.as_mut_ptr() as *mut u8,
                                GLOBAL_MAP_LOD_SIZE,
                            )
                        };
                        reader.read_sub_data_exact(buffer_view)?; // Consumes from left_sub, left_rec
                        land.wnam_low_lod_heights = Some(wnam_data_arr);
                        // OpenMW C++ condition: if (mFlags & Flag_HeightsNormals) mDataTypes |= DATA_WNAM;
                        // Replicating this, though it's a bit odd for WNAM.
                        // If WNAM is present, it's generally considered available.
                        if (land.file_data_content_flags
                            & land_file_content_flags::HEIGHTS_AND_NORMALS_PRESENT)
                            != 0
                        {
                            available_mask_val |= LandDataPresenceFlags::WNAM;
                        } else if land.wnam_low_lod_heights.is_some() {
                            // If we loaded WNAM, consider it available for our mask
                            available_mask_val |= LandDataPresenceFlags::WNAM;
                        }
                    } else {
                        // eprintln!("[LAND WARN] WNAM for ({},{}) has size {}, expected {}. Skipping payload.",
                        //    land.grid_x, land.grid_y, reader.get_sub_data_size(), expected_size);
                        reader.skip_sub_data_payload()?; // Skip incorrect WNAM payload
                    }
                }
                b"VCLR" => {
                    reader.skip_h_sub()?;
                    if (land.file_data_content_flags & land_file_content_flags::COLORS_PRESENT) != 0
                    {
                        available_mask_val |= LandDataPresenceFlags::VCLR;
                    }
                }
                b"VTEX" => {
                    reader.skip_h_sub()?;
                    if (land.file_data_content_flags & land_file_content_flags::TEXTURES_PRESENT)
                        != 0
                    {
                        available_mask_val |= LandDataPresenceFlags::VTEX;
                    }
                }
                b"VMIN" | b"VMAX" => {
                    // These are usually at the end if present
                    reader.skip_h_sub()?;
                }
                _ => {
                    // Log or fail on unknown subrecord within LAND after INTV/DATA
                    // eprintln!("[LAND WARN] Unknown subrecord {} in LAND ({},{}) during header/availability scan. Skipping.",
                    //    sub_name.to_string_lossy(), land.grid_x, land.grid_y);
                    reader.skip_h_sub()?; // Skip it to try and continue
                }
            }
        }
        land.data_chunks_available_in_file = LandDataPresenceFlags(available_mask_val);

        // Debug logging for cell (0,0) as per your request
        if land.grid_x == 0 && land.grid_y == 0 {
            info!(
                "LAND (0,0) [load_headers] - file_data_content_flags: 0x{:X}",
                land.file_data_content_flags
            );
            info!(
                "LAND (0,0) [load_headers] - data_chunks_available_in_file (final): 0x{:X}",
                land.data_chunks_available_in_file.0
            );
            info!(
                "LAND (0,0) [load_headers] - VHGT bit available: {}",
                (land.data_chunks_available_in_file.0 & LandDataPresenceFlags::VHGT) != 0
            );
            info!(
                "LAND (0,0) [load_headers] - WNAM bit available: {}",
                (land.data_chunks_available_in_file.0 & LandDataPresenceFlags::WNAM) != 0
            );
            info!(
                "LAND (0,0) [load_headers] - WNAM data loaded: {}",
                land.wnam_low_lod_heights.is_some()
            );
        }

        Ok(land)
    }

    /// Phase 2: Loads detailed data chunks on demand using the saved context.
    pub fn load_detailed_data(
        &mut self,
        reader: &mut EsmReader,
        requested_data_mask: LandDataPresenceFlags,
    ) -> Result<(), EsmError> {
        let context_to_restore = match self.data_chunks_context.clone() {
            // Clone context for restoration
            Some(ctx) => ctx,
            None => {
                // If no context, means no detailed data chunks were expected to follow headers.
                // Only proceed if there's something available and requested that wasn't loaded.
                if (requested_data_mask.0 & self.data_chunks_available_in_file.0) != 0 {
                    // This case might indicate an issue if WNAM was the only thing available and already loaded
                }
                return Ok(());
            }
        };

        // Initialize detailed_data if it's None
        if self.detailed_data.is_none() {
            self.detailed_data = Some(Box::new(LandDetailedData::default()));
        }
        // Get a mutable reference to the detailed_data, which is now guaranteed to be Some.
        let detail_data_mut = self.detailed_data.as_mut().unwrap();

        // Determine which specific chunks need loading in this call
        let mut load_this_pass_mask = LandDataPresenceFlags::NONE;
        if (requested_data_mask.0 & LandDataPresenceFlags::VNML != 0)
            && (self.data_chunks_available_in_file.0 & LandDataPresenceFlags::VNML != 0)
            && detail_data_mut.normals.is_none()
        {
            // Check if not already loaded in detail_data_mut
            load_this_pass_mask |= LandDataPresenceFlags::VNML;
        }
        if (requested_data_mask.0 & LandDataPresenceFlags::VHGT != 0)
            && (self.data_chunks_available_in_file.0 & LandDataPresenceFlags::VHGT != 0)
            && detail_data_mut.heights.is_none()
        {
            load_this_pass_mask |= LandDataPresenceFlags::VHGT;
        }
        if (requested_data_mask.0 & LandDataPresenceFlags::VCLR != 0)
            && (self.data_chunks_available_in_file.0 & LandDataPresenceFlags::VCLR != 0)
            && detail_data_mut.colors.is_none()
        {
            load_this_pass_mask |= LandDataPresenceFlags::VCLR;
        }
        if (requested_data_mask.0 & LandDataPresenceFlags::VTEX != 0)
            && (self.data_chunks_available_in_file.0 & LandDataPresenceFlags::VTEX != 0)
            && detail_data_mut.textures.is_none()
        {
            load_this_pass_mask |= LandDataPresenceFlags::VTEX;
        }
        // WNAM is not part of detailed_data, already on self.

        if load_this_pass_mask == LandDataPresenceFlags::NONE {
            return Ok(()); // Nothing to load in this call
        }

        reader.restore_context(&context_to_restore)?; // Restore reader to start of detailed chunks

        // Loop to load the necessary data chunks
        while reader.has_more_subs() {
            // Optimization: if all bits in load_this_pass_mask have been fulfilled, we can stop.
            if (detail_data_mut.data_loaded_mask.0 & load_this_pass_mask) == load_this_pass_mask {
                break;
            }

            let sub_name = reader.get_sub_name()?; // Consumes name, updates left_rec
            match &sub_name.m_data {
                b"VNML"
                    if (load_this_pass_mask & LandDataPresenceFlags::VNML != 0)
                        && detail_data_mut.normals.is_none() =>
                {
                    reader.get_sub_header()?;
                    let expected_size = (LAND_NUM_VERTS * 3) as u32;
                    if reader.get_sub_data_size() == expected_size {
                        let mut normals_flat = vec![0u8; expected_size as usize];
                        reader.read_sub_data_exact(&mut normals_flat)?;
                        let mut normals_structured = Box::new([[0u8; 3]; LAND_NUM_VERTS]);
                        for i in 0..LAND_NUM_VERTS {
                            normals_structured[i][0] = normals_flat[i * 3];
                            normals_structured[i][1] = normals_flat[i * 3 + 1];
                            normals_structured[i][2] = normals_flat[i * 3 + 2];
                        }
                        detail_data_mut.normals = Some(normals_structured);
                        detail_data_mut.data_loaded_mask.0 |= LandDataPresenceFlags::VNML;
                    } else {
                        reader.skip_sub_data_payload()?;
                    }
                }
                b"VHGT"
                    if (load_this_pass_mask & LandDataPresenceFlags::VHGT != 0)
                        && detail_data_mut.heights.is_none() =>
                {
                    reader.get_sub_header()?;
                    let expected_size = 4 + LAND_NUM_VERTS as u32; // float offset + i8 deltas
                    if reader.get_sub_data_size() == expected_size {
                        let vhgt_offset = reader.get_t_consuming::<f32>()?; // Consumes 4 bytes
                        let mut vhgt_deltas_raw_i8 = [0i8; LAND_NUM_VERTS];
                        let buffer_view: &mut [u8] = unsafe {
                            std::slice::from_raw_parts_mut(
                                vhgt_deltas_raw_i8.as_mut_ptr() as *mut u8,
                                LAND_NUM_VERTS,
                            )
                        };
                        reader.read_sub_data_exact(buffer_view)?; // Consumes LAND_NUM_VERTS bytes

                        let mut heights_calculated = Box::new([0.0f32; LAND_NUM_VERTS]);
                        let mut current_row_offset_accum = vhgt_offset;
                        let mut min_h = f32::MAX;
                        let mut max_h = f32::MIN;

                        for y_idx in 0..LAND_SIZE {
                            current_row_offset_accum +=
                                vhgt_deltas_raw_i8[y_idx * LAND_SIZE] as f32;
                            let val = current_row_offset_accum * HEIGHT_SCALE;
                            heights_calculated[y_idx * LAND_SIZE] = val;
                            min_h = min_h.min(val);
                            max_h = max_h.max(val);

                            let mut current_col_offset_accum = current_row_offset_accum;
                            for x_idx in 1..LAND_SIZE {
                                let flat_idx = y_idx * LAND_SIZE + x_idx;
                                current_col_offset_accum += vhgt_deltas_raw_i8[flat_idx] as f32;
                                let val_col = current_col_offset_accum * HEIGHT_SCALE;
                                heights_calculated[flat_idx] = val_col;
                                min_h = min_h.min(val_col);
                                max_h = max_h.max(val_col);
                            }
                        }
                        detail_data_mut.heights = Some(heights_calculated);
                        detail_data_mut.min_height = min_h;
                        detail_data_mut.max_height = max_h;
                        detail_data_mut.data_loaded_mask.0 |= LandDataPresenceFlags::VHGT;
                        if self.grid_x == 0 && self.grid_y == 0 {
                            info!(
                                "LAND (0,0) [load_detailed_data] - VHGT block processed. Heights.is_some(): {}",
                                detail_data_mut.heights.is_some()
                            );
                        }
                    } else {
                        reader.skip_sub_data_payload()?;
                    }
                }
                b"VCLR"
                    if (load_this_pass_mask & LandDataPresenceFlags::VCLR != 0)
                        && detail_data_mut.colors.is_none() =>
                {
                    reader.get_sub_header()?;
                    let expected_size = (LAND_NUM_VERTS * 3) as u32;
                    if reader.get_sub_data_size() == expected_size {
                        let mut colors_flat = vec![0u8; expected_size as usize];
                        reader.read_sub_data_exact(&mut colors_flat)?;
                        let mut colors_structured = Box::new([[0u8; 3]; LAND_NUM_VERTS]);
                        for i in 0..LAND_NUM_VERTS {
                            colors_structured[i][0] = colors_flat[i * 3];
                            colors_structured[i][1] = colors_flat[i * 3 + 1];
                            colors_structured[i][2] = colors_flat[i * 3 + 2];
                        }
                        detail_data_mut.colors = Some(colors_structured);
                        detail_data_mut.data_loaded_mask.0 |= LandDataPresenceFlags::VCLR;
                    } else {
                        reader.skip_sub_data_payload()?;
                    }
                }
                b"VTEX"
                    if (load_this_pass_mask & LandDataPresenceFlags::VTEX != 0)
                        && detail_data_mut.textures.is_none() =>
                {
                    reader.get_sub_header()?;
                    let expected_size = (LAND_NUM_TEXTURES * 2) as u32; // u16 per texture
                    if reader.get_sub_data_size() == expected_size {
                        let mut vtex_flat_raw = [0u16; LAND_NUM_TEXTURES];
                        // Read as a block of u16s
                        for i in 0..LAND_NUM_TEXTURES {
                            vtex_flat_raw[i] = reader.get_t_consuming::<u16>()?;
                        }
                        detail_data_mut.textures =
                            Some(transpose_texture_data_rust(&vtex_flat_raw));
                        detail_data_mut.data_loaded_mask.0 |= LandDataPresenceFlags::VTEX;
                    } else {
                        reader.skip_sub_data_payload()?;
                    }
                }
                b"WNAM" | b"VMIN" | b"VMAX" => {
                    // WNAM is already loaded in `self`. VMIN/VMAX could be parsed here if needed.
                    // For now, just ensure they are skipped if encountered again.
                    reader.skip_h_sub()?;
                }
                _ => {
                    // Skip any other unexpected or already processed subrecords
                    reader.skip_h_sub()?;
                }
            }
        }
        // Ensure the rest of the record's data block is consumed if we broke early
        // or if there were trailing unknown subrecords.
        while reader.has_more_subs() {
            reader.skip_h_sub()?;
        }
        Ok(())
    }
}

// Add to your EsmReader a way to peek next sub name's bytes without full context save/restore,
// if the current `peek_next_sub` isn't ideal for the `load_headers` initial loop.
// For now, assuming `get_sub_name` + `cache_sub_name` is the way `peek_next_sub_name_bytes` is simulated.
// It's better if EsmReader::peek_next_sub itself is robust.

// A helper in EsmReader might be:
// fn peek_actual_next_sub_name(&mut self) -> Result<Option<Name>, EsmError> {
// if !self.has_more_subs() { return Ok(None); }
// if self.m_ctx.sub_cached { return Ok(Some(self.m_ctx.sub_name)); }
//
// let original_pos = self.m_ctx.file_pos;
// let original_left_rec = self.m_ctx.left_rec;
// let name = self.get_sub_name_raw_no_cache_clear()?; // Hypothetical: reads name, updates pos, left_rec
//
// // Restore:
// self.m_ctx.file_pos = original_pos;
// self.m_ctx.left_rec = original_left_rec;
// if let Some(ref mut r) = self.m_reader { r.seek(SeekFrom::Start(original_pos))?; }
//
// Ok(Some(name))
// }
// This is still tricky. The current `peek_next_sub` should be made robust for this.
// The version of `is_next_sub` and `peek_next_sub` in the `esm_reader.rs` I provided
// previously should correctly handle caching and allow the `Landscape::load_headers`
// initial loop to work.

