use crate::geometry::NiWireframeProperty;
use crate::nif::error::{ParseError, Result};
use crate::nif::parser::base_parsers::{parse_niavobject_fields, parse_niobjectnet_fields};
use crate::nif::parser::helpers::*;
use crate::nif::types::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub fn parse_nidynamiceffect_fields(
    cursor: &mut Cursor<&[u8]>,
    av_base: NiAVObject,
    block_index: u32,
) -> Result<NiDynamicEffect> {
    println!(
        "     Parsing NiDynamicEffect fields (Block {})...",
        block_index
    );

    let num_affected_nodes = cursor.read_u32::<LittleEndian>()?;
    println!(
        "       -> Num Affected Nodes: {} @ cursor {:#X}",
        num_affected_nodes,
        cursor.position()
    );

    let mut affected_nodes = Vec::with_capacity(num_affected_nodes as usize);
    if num_affected_nodes > 1000 {
        return Err(ParseError::InvalidData(
            "Too many affected nodes".to_string(),
        ));
    } // Sanity check
    for _ in 0..num_affected_nodes {
        affected_nodes.push(read_link(cursor)?); // Read N * i32 links
    }
    println!(
        "       -> Read {} affected node links. Cursor {:#X}",
        affected_nodes.len(),
        cursor.position()
    );
    // Cursor is now positioned correctly after affected nodes list

    Ok(NiDynamicEffect {
        av_base,
        num_affected_nodes,
        affected_nodes,
    })
}
pub fn parse_niproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    // block_index: u32, // Optional context
    // nif_version: u32, // Optional context
) -> Result<NiProperty> {
    let property_start_pos = cursor.position();
    println!(
        "    Parsing NiProperty fields starting at 0x{:X}...",
        property_start_pos
    );
    // Assumes NiObjectNET was parsed *just before* calling this
    // Alternatively, parse it here:
    let net_base = parse_niobjectnet_fields(cursor)?;

    // Read fields specific to NiProperty for NIF v4.0.0.2

    let property_end_pos = cursor.position();
    println!(
        "    Finished NiProperty fields at 0x{:X} (Size: {} bytes)",
        property_end_pos,
        property_end_pos - property_start_pos
    );

    Ok(NiProperty { net_base })
}
pub fn parse_niwireframe_property_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
    // block_index: u32, // Optional context
    // nif_version: u32, // Optional context
) -> Result<NiWireframeProperty> {
    let wireframe_start_pos = cursor.position();
    println!(
        "  Parsing NiWireframeProperty fields starting at 0x{:X}...",
        wireframe_start_pos
    );

    // Parse the base NiProperty fields first
    let base_property = parse_niproperty_fields(cursor)?;

    // Now read the fields specific to NiWireframeProperty itself
    let wire_flags_pos = cursor.position();
    let wire_flags = cursor.read_u16::<LittleEndian>()?;
    println!(
        "    -> NiWireframeProperty Flags: 0x{:04X} (Enabled: {}) at 0x{:X}",
        wire_flags,
        (wire_flags & 1) != 0, // Check bit 0 for enabled status
        wire_flags_pos
    );

    let wireframe_end_pos = cursor.position();
    println!(
        "  Finished NiWireframeProperty fields at 0x{:X} (Size: {} bytes)",
        wireframe_end_pos,
        wireframe_end_pos - wireframe_start_pos
    );

    Ok(NiWireframeProperty {
        base_property,
        wire_flags,
    })
}

pub fn parse_nitextureeffect_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiTextureEffect> {
    println!(
        "   Parsing NiTextureEffect fields for block {}...",
        block_index
    );

    // 1. Parse base classes sequentially
    println!(
        "     Calling parse_niobjectnet_fields starting @ {:#X}",
        cursor.position()
    );
    let net_part = parse_niobjectnet_fields(cursor)?;
    println!(
        "     -> After parse_niobjectnet_fields: {:#X}",
        cursor.position()
    );

    println!(
        "     Calling parse_niavobject_fields starting @ {:#X}",
        cursor.position()
    );
    let av_part = parse_niavobject_fields(cursor, net_part, block_index)?;
    println!(
        "     -> After parse_niavobject_fields: {:#X}",
        cursor.position()
    ); // IMPORTANT CHECK

    println!(
        "     Calling parse_nidynamiceffect_fields starting @ {:#X}",
        cursor.position()
    );
    let dynamic_effect_part = parse_nidynamiceffect_fields(cursor, av_part, block_index)?;
    println!(
        "     -> After parse_nidynamiceffect_fields: {:#X}",
        cursor.position()
    ); // IMPORTANT CHECK

    // 2. Parse NiTextureEffect specific fields
    println!(
        "     Reading NiTextureEffect specific fields starting @ {:#X}...",
        cursor.position()
    );

    let model_projection_matrix = read_matrix3x3(cursor)?;
    let model_projection_translation = read_vector3(cursor)?;
    let texture_filtering_raw = cursor.read_u32::<LittleEndian>()?;
    let texture_clamping_raw = cursor.read_u32::<LittleEndian>()?;
    let texture_type_raw = cursor.read_u32::<LittleEndian>()?;
    let coordinate_generation_type_raw = cursor.read_u32::<LittleEndian>()?;
    let source_texture = read_link(cursor)?;
    let clipping_plane_raw = cursor.read_u8()?;
    let enable_plane = clipping_plane_raw != 0;

    let plane_data = Some(read_plane(cursor)?);

    let ps2_l = cursor.read_i16::<LittleEndian>()?;
    let ps2_k = cursor.read_i16::<LittleEndian>()?;
    let unknown_short = cursor.read_u16::<LittleEndian>()?;

    // Convert enums
    let texture_filtering = FilterMode::from(texture_filtering_raw);
    let texture_clamping = ClampMode::from(texture_clamping_raw);
    let texture_type = EffectType::from(texture_type_raw);
    let coordinate_generation_type = CoordGenType::from(coordinate_generation_type_raw);

    println!(
        "     -> Final Values: Filter:{:?}, Clamp:{:?}, TexType:{:?}, CoordGen:{:?}, SourceTex:{:?}, UsePlane:{}, PS2L:{}, PS2K:{}, UnkShort:{}",
        texture_filtering,
        texture_clamping,
        texture_type,
        coordinate_generation_type,
        source_texture,
        enable_plane,
        ps2_l,
        ps2_k,
        unknown_short
    );

    // 3. Construct struct
    let tex_effect = NiTextureEffect {
        dynamic_effect_base: dynamic_effect_part,
        model_projection_matrix,
        model_projection_translation,
        texture_filtering,
        texture_clamping,
        texture_type,
        coordinate_generation_type,
        source_texture,
        enable_plane,
        plane: plane_data,
        ps2_l,
        ps2_k,
        unknown_short,
    };
    println!(
        "   -> Successfully parsed NiTextureEffect fields. Cursor ending at {:#X}",
        cursor.position()
    );
    Ok(tex_effect)
}
