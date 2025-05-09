// src/nif/parser/texture.rs

use super::helpers::Result;
use crate::{
    NiAlphaProperty,
    nif::{
        parser::{
            base_parsers::parse_niobjectnet_fields,
            helpers::{read_link, read_nif_string},
        },
        types::{
            AlphaFormat, ApplyMode, ClampMode, FilterMode, LightMode, MipMapFormat, NiProperty,
            NiSourceTexture, NiTexturingProperty, NiVertexColorProperty, PixelLayout, TextureData,
            VertexMode,
        },
    },
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub fn parse_nivertexcolorproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiVertexColorProperty> {
    // 1. Parse base NiProperty fields (which parses NiObjectNET)
    let net_part = parse_niobjectnet_fields(cursor)?;
    let property_base = NiProperty { net_base: net_part }; // Construct base

    // 2. Parse NiVertexColorProperty specific fields (v4.0.0.2)
    let flags = cursor.read_u16::<LittleEndian>()?;

    let mut vertex_mode_opt = None;
    // NifXML: Bit 0 - VERTEX_MODE_SRC_AMB_DIF
    let vertex_mode_raw = cursor.read_u32::<LittleEndian>()?;
    if (flags & 0x0001) != 0 {
        let mode = VertexMode::from(vertex_mode_raw);
        vertex_mode_opt = Some(mode);
    } else {
    }

    let mut lighting_mode_opt = None;
    // NifXML: Bit 1 - LIGHTING_MODE_E_A_D
    let lighting_mode_raw = cursor.read_u32::<LittleEndian>()?;
    if (flags & 0x0002) != 0 {
        let mode = LightMode::from(lighting_mode_raw);
        lighting_mode_opt = Some(mode);
    } else {
    }

    Ok(NiVertexColorProperty {
        property_base,
        flags,
        vertex_mode: vertex_mode_opt,
        lighting_mode: lighting_mode_opt,
    })
}

pub fn parse_nisourcetexture_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiSourceTexture> {
    // 1. Parse NiObjectNET base fields
    let net_part = parse_niobjectnet_fields(cursor)?;

    // 2. Parse NiSourceTexture specific fields (v4.0.0.2)

    // Use External flag (read as u8 for v4.0.0.2 based on C++ bool logic)
    let use_external_u8 = cursor.read_u8()?;
    let use_external = use_external_u8 != 0;

    // Has Pixel Data flag (only present if use_external is false)
    let mut has_pixel_data = false; // Default
    if !use_external {
        let has_pixel_data_u8 = cursor.read_u8()?;
        has_pixel_data = has_pixel_data_u8 != 0;
    }

    // File Name (only present if use_external is true)
    let mut file_name = None;
    if use_external {
        // Morrowind often stores filename as length-prefixed string here
        let name_len = cursor.read_u32::<LittleEndian>()?;
        let name_str = read_nif_string(cursor, name_len)?;
        file_name = Some(name_str);
    }

    // Pixel Data Link (only present if NOT use_external AND has_pixel_data)
    let mut pixel_data_link = None;
    if !use_external && has_pixel_data {
        pixel_data_link = read_link(cursor)?;
    }

    // Format Preferences
    let pixel_layout_raw = cursor.read_u32::<LittleEndian>()?;
    let use_mipmaps_raw = cursor.read_u32::<LittleEndian>()?;
    let alpha_format_raw = cursor.read_u32::<LittleEndian>()?;
    let pixel_layout = PixelLayout::from(pixel_layout_raw);
    let use_mipmaps = MipMapFormat::from(use_mipmaps_raw);
    let alpha_format = AlphaFormat::from(alpha_format_raw);

    // Is Static flag (read as u8 for v4.0.0.2 based on C++ bool logic)
    let is_static_u8 = cursor.read_u8()?;
    let is_static = is_static_u8 != 0;

    // Construct struct
    let source_texture = NiSourceTexture {
        net_base: net_part,
        use_external,
        file_name,
        pixel_data_link,
        pixel_layout,
        use_mipmaps,
        alpha_format,
        is_static,
    };

    Ok(source_texture)
}

pub fn parse_nitexturingproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiTexturingProperty> {
    // 1. Parse NiObjectNET base fields shown in NifSkope
    let net_part = parse_niobjectnet_fields(cursor)?; // Reads Name, ExtraLink, ControllerLink (12 bytes total)

    // 2. Parse NiTexturingProperty specific fields shown
    let flags = cursor.read_u16::<LittleEndian>()?; // Reads Flags (2 bytes)
    let apply_mode_raw = cursor.read_u32::<LittleEndian>()?; // Reads Apply Mode (4 bytes)
    let apply_mode = ApplyMode::from(apply_mode_raw);
    let texture_count = cursor.read_u32::<LittleEndian>()?; // Reads Texture Count (4 bytes)
    // Cursor should be at start of Slot 0 data (e.g., 0x1B7)

    // Prepare fields - only base_texture might get full data based on flag
    let mut base_texture_data: Option<TextureData> = None;
    let mut has_dark_texture = false;
    let mut has_detail_texture = false;
    let mut has_gloss_texture = false;
    let mut has_glow_texture = false;
    let mut has_bump_map_texture = false;
    let mut has_decal_0_texture = false;

    // 3. Read Base Texture Slot (Index 0) - Read details shown if enabled
    if texture_count > 0 {
        // Basic check if slot could exist
        let base_has_texture_u32 = cursor.read_u32::<LittleEndian>()?; // Read 1 byte flag
        let base_has_texture = base_has_texture_u32 != 0;

        if base_has_texture {
            // Read all fields listed for Base Texture in NifSkope
            let source_texture = read_link(cursor)?; // 4 bytes
            let clamp_raw = cursor.read_u32::<LittleEndian>()?; // 4 bytes
            let filter_raw = cursor.read_u32::<LittleEndian>()?; // 4 bytes
            let uv_set = cursor.read_u32::<LittleEndian>()?; // 4 bytes
            let _ps2_l = cursor.read_i16::<LittleEndian>()?; // 2 bytes
            let _ps2_k = cursor.read_i16::<LittleEndian>()?; // 2 bytes
            let _unknown1 = cursor.read_u16::<LittleEndian>()?; // 2 bytes
            // Total 22 bytes read after flag

            let clamp_mode = ClampMode::from(clamp_raw);
            let filter_mode = FilterMode::from(filter_raw);

            base_texture_data = Some(TextureData {
                has_texture: true,
                source_texture,
                clamp_mode,
                filter_mode,
                uv_set,
                // ps2_l, ps2_k, unknown1, // Add if needed
            });
        } else {
            // If flag is false, per user instruction, do NOT read/skip the 22 bytes
            base_texture_data = Some(TextureData {
                has_texture: false,
                ..Default::default()
            });
        }
    } else {
    }

    // 4. Read ONLY the boolean flags for subsequent slots (if texture_count implies they exist)
    if texture_count > 1 {
        has_dark_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 1 flag only (1 byte)
    }
    if texture_count > 2 {
        has_detail_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 2 flag only (1 byte)
    }
    if texture_count > 3 {
        has_gloss_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 3 flag only (1 byte)
    }
    if texture_count > 4 {
        has_glow_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 4 flag only (1 byte)
    }
    if texture_count > 5 {
        has_bump_map_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 5 flag only (1 byte)
        // Do NOT read bump map extra data even if true, per user instruction
    }
    if texture_count > 6 {
        has_decal_0_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 6 flag only (1 byte)
    }
    // *** STOP READING HERE - Cursor is now misaligned ***

    // 5. Construct struct with only the information read
    //    Create dummy TextureData just to store the boolean flag for slots > 0
    let create_dummy_tex_data = |has_tex| {
        Some(TextureData {
            has_texture: has_tex,
            ..Default::default()
        })
    };

    let tex_prop = NiTexturingProperty {
        property_base: NiProperty { net_base: net_part },
        flags,
        apply_mode,
        texture_count,                   // Store original count
        base_texture: base_texture_data, // Store base texture info read
        dark_texture: if texture_count > 1 {
            create_dummy_tex_data(has_dark_texture)
        } else {
            None
        },
        detail_texture: if texture_count > 2 {
            create_dummy_tex_data(has_detail_texture)
        } else {
            None
        },
        gloss_texture: if texture_count > 3 {
            create_dummy_tex_data(has_gloss_texture)
        } else {
            None
        },
        glow_texture: if texture_count > 4 {
            create_dummy_tex_data(has_glow_texture)
        } else {
            None
        },
        bump_map_texture: if texture_count > 5 {
            create_dummy_tex_data(has_bump_map_texture)
        } else {
            None
        },
        // NifSkope shows "Decal 0 Texture" which corresponds to slot 6 for v4.0.0.2
        normal_texture: if texture_count > 6 {
            create_dummy_tex_data(has_decal_0_texture)
        } else {
            None
        },
        decal_0_texture: None, // NifSkope didn't show slot 7 separately
        ..Default::default()   // Default bump fields
    };

    Ok(tex_prop)
}
pub fn parse_nialphaproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiAlphaProperty> {
    // 1. Parse NiObjectNET base fields (as NiProperty doesn't add fields itself)
    let net_part = parse_niobjectnet_fields(cursor)?;

    // 2. Parse NiAlphaProperty specific fields (for v4.0.0.2)
    let flags = cursor.read_u16::<LittleEndian>()?;
    let threshold = cursor.read_u8()?;

    // 3. Construct nested struct
    let alpha_prop = NiAlphaProperty {
        property_base: NiProperty {
            // Wrap the parsed NiObjectNET part
            net_base: net_part,
        },
        flags,
        threshold,
    };
    Ok(alpha_prop)
}
