use crate::error::{ParseError, Result}; // Import error types (assuming src/error.rs)
use crate::structs::*;
use bevy::math::Quat;
// Import definitions from structs module
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read}; // Added Seek for potential future use

// --- Main Parsing Function ---
pub fn parse_nif_start(data: &[u8]) -> Result<ParsedNifData> {
    let mut cursor = Cursor::new(data);

    // 1. Header String Reading (Simplified for brevity)
    let mut header_bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if cursor.read(&mut byte)? == 0 {
            return Err(ParseError::InvalidData("EOF in header".to_string()));
        }
        if byte[0] == b'\n' {
            break;
        }
        header_bytes.push(byte[0]);
        if header_bytes.len() > 100 {
            return Err(ParseError::InvalidData("Header too long".to_string()));
        }
    }
    let version_string = String::from_utf8_lossy(&header_bytes)
        .trim_end_matches('\r')
        .to_string();
    if !version_string.starts_with("NetImmerse File Format") {
        return Err(ParseError::InvalidData("Not a NIF file".to_string()));
    }

    // 2. File Version (u32 LE)
    let file_version = cursor.read_u32::<LittleEndian>()?;
    if file_version != 0x04000002 {
        println!(
            " !! WARNING: Expected version 0x04000002, found 0x{:08X}",
            file_version
        );
        // Consider returning Err(ParseError::UnsupportedVersion(file_version)) if strict
    }

    // 3. Number of Blocks (u32 LE)
    let num_blocks = cursor.read_u32::<LittleEndian>()?;

    let header = NifHeader {
        version_string,
        file_version,
        num_blocks,
    };
    let mut blocks: Vec<ParsedBlock> = Vec::with_capacity(num_blocks as usize);

    // --- Block Reading Loop ---
    for i in 0..num_blocks {
        // Read Block Type Name
        println!("cursor pos: {:x}", cursor.position());
        let block_type_len = cursor.read_u32::<LittleEndian>()?;
        println!("len: {}", block_type_len);
        let block_type_name = read_nif_string(&mut cursor, block_type_len)?;
        println!("-> Processing Block {} (Type: '{}')", i, block_type_name);

        // Dispatch based on Block Type Name
        let parse_result = match block_type_name.as_str() {
            "NiNode" => parse_ninode_fields(&mut cursor, i).map(ParsedBlock::Node),
            "NiTriShape" => parse_nitrishape_fields(&mut cursor, i).map(ParsedBlock::TriShape),
            "NiAlphaProperty" => {
                parse_nialphaproperty_fields(&mut cursor, i).map(ParsedBlock::AlphaProperty)
            }
            "NiTexturingProperty" => {
                parse_nitexturingproperty_fields(&mut cursor, i).map(ParsedBlock::TexturingProperty)
            }
            "NiSourceTexture" => {
                parse_nisourcetexture_fields(&mut cursor, i).map(ParsedBlock::SourceTexture)
            }
            "NiMaterialProperty" => {
                parse_nimaterialproperty_fields(&mut cursor, i).map(ParsedBlock::MaterialProperty)
            }
            "NiTriShapeData" => {
                parse_nitrishapedata_fields(&mut cursor, i).map(ParsedBlock::TriShapeData)
            }
            "NiKeyframeController" => parse_nikeyframecontroller_fields(&mut cursor, i)
                .map(ParsedBlock::KeyframeController),
            "NiKeyframeData" => {
                parse_nikeyframedata_fields(&mut cursor, i).map(ParsedBlock::KeyframeData)
            }
            "NiTextureEffect" => {
                // <-- Add this case
                parse_nitextureeffect_fields(&mut cursor, i).map(ParsedBlock::TextureEffect)
            }
            "NiTextKeyExtraData" => {
                parse_nitextkeyextradata_fields(&mut cursor, i).map(ParsedBlock::TextKeyExtraData)
            }
            "NiVertexColorProperty" => parse_nivertexcolorproperty_fields(&mut cursor, i)
                .map(ParsedBlock::VertexColorProperty),
            "NiGeomMorpherController" => parse_nigeommorphercontroller_fields(&mut cursor, i)
                .map(ParsedBlock::GeomMorpherController),
            "NiMorphData" => parse_nimorphdata_fields(&mut cursor, i).map(ParsedBlock::MorphData),

            unknown_type => {
                println!(
                    "   ERROR: Unsupported block type '{}'. Cannot parse.",
                    unknown_type
                );
                // Skipping requires knowing block size, which is complex for v4.0.0.2.
                // Returning an error is the simplest safe action.
                Err(ParseError::UnsupportedBlockType(unknown_type.to_string()))
            }
        };

        // Handle the result for this block
        match parse_result {
            Ok(parsed_block) => {
                blocks.push(parsed_block);
            }
            Err(e) => {
                eprintln!("   Failed to parse block {}: {:?}", i, e);
                // Stop parsing on the first error
                return Err(e);
            }
        }
    } // End block loop

    println!("\n--- NIF parsing function finished ---");
    Ok(ParsedNifData { header, blocks })
}
fn parse_nimorphdata_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiMorphData> {
    println!("   Parsing NiMorphData fields for block {}...", block_index);

    let num_morph_targets = cursor.read_u32::<LittleEndian>()?;
    let num_vertices = cursor.read_u32::<LittleEndian>()?;
    let relative_targets = cursor.read_u8()? != 0;

    println!(
        "     -> Num Morph Targets: {}, Num Vertices: {}, Relative: {}",
        num_morph_targets, num_vertices, relative_targets
    );

    // Sanity checks
    if num_morph_targets > 1000 {
        return Err(ParseError::InvalidData(
            "Too many morph targets".to_string(),
        ));
    }
    if num_vertices > 100_000 {
        return Err(ParseError::InvalidData(
            "Too many vertices in morph".to_string(),
        ));
    }

    // Read the morph target data
    let mut morph_targets_struct_vec = Vec::with_capacity(num_morph_targets as usize);
    for target_idx in 0..num_morph_targets {
        println!("       Reading target {}...", target_idx);

        // *** Read MorphTarget Key Info FIRST ***
        let num_keys = cursor.read_u32::<LittleEndian>()?;
        let interpolation_raw = cursor.read_u32::<LittleEndian>()?;
        let interpolation = KeyType::from(interpolation_raw);
        println!(
            "         -> Num Keys: {}, Interpolation: {:?}",
            num_keys, interpolation
        );

        let mut keys_vec = Vec::with_capacity(num_keys as usize);
        if num_keys > 10000 {
            return Err(ParseError::InvalidData("Too many morph keys".to_string()));
        }
        for _key_idx in 0..num_keys {
            // Pass interpolation type to handle potential extra data reads/skips
            keys_vec.push(read_key_float(cursor, interpolation)?);
        }
        println!("         -> Read {} keys.", keys_vec.len());

        // *** THEN Read Vertex Vectors for this target ***
        println!(
            "         Reading target {} vertices ({} verts)...",
            target_idx, num_vertices
        );
        let mut vertex_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            vertex_vec.push(read_vector3(cursor)?); // Reads 12 bytes
        }
        println!(
            "         -> Read {} vertices for target {}",
            vertex_vec.len(),
            target_idx
        );

        // Create and push the MorphTarget struct containing keys AND vertices
        morph_targets_struct_vec.push(MorphTarget {
            num_keys,
            interpolation,
            keys: keys_vec,
            vertices: vertex_vec,
        });
    } // End loop over morph targets

    println!(
        "     -> Cursor after NiMorphData fields: {:#X}",
        cursor.position()
    );

    Ok(NiMorphData {
        num_morph_targets,
        num_vertices,
        relative_targets,
        morph_targets: morph_targets_struct_vec, // Assign Vec<MorphTarget>
    })
}
fn parse_nigeommorphercontroller_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiGeomMorpherController> {
    println!(
        "   Parsing NiGeomMorpherController fields for block {}...",
        block_index
    );

    // 1. Read NiTimeController fields
    let next_controller = read_link(cursor)?;
    let flags = cursor.read_u16::<LittleEndian>()?;
    let frequency = cursor.read_f32::<LittleEndian>()?;
    let phase = cursor.read_f32::<LittleEndian>()?;
    let start_time = cursor.read_f32::<LittleEndian>()?;
    let stop_time = cursor.read_f32::<LittleEndian>()?;
    let target = read_link(cursor)?;

    // 2. Read Specific field
    let morph_data = read_link(cursor)?;

    // *** ADDED: Read Always Update flag (byte) based on NifSkope ***
    let always_update_u8 = cursor.read_u8()?; // Reads 1 byte
    let always_update = always_update_u8 != 0; // Convert byte to bool
    // *** END ADDED ***

    println!(
        "     -> Flags: {:#06X}, Freq: {}, Target: {:?}, Data: {:?}, AlwaysUpdate: {}", // Added AlwaysUpdate
        flags, frequency, target, morph_data, always_update
    );

    Ok(NiGeomMorpherController {
        next_controller,
        flags,
        frequency,
        phase,
        start_time,
        stop_time,
        target,
        morph_data,
        always_update, // Store the value read
    })
}

fn parse_nivertexcolorproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiVertexColorProperty> {
    println!(
        "   Parsing NiVertexColorProperty fields for block {}...",
        block_index
    );

    // 1. Parse base NiProperty fields (which parses NiObjectNET)
    let net_part = parse_niobjectnet_fields(cursor)?;
    let property_base = NiProperty { net_base: net_part }; // Construct base

    // 2. Parse NiVertexColorProperty specific fields (v4.0.0.2)
    let flags = cursor.read_u16::<LittleEndian>()?;
    println!("     -> Flags: {:#06X}", flags);

    let mut vertex_mode_opt = None;
    // NifXML: Bit 0 - VERTEX_MODE_SRC_AMB_DIF
    let vertex_mode_raw = cursor.read_u32::<LittleEndian>()?;
    if (flags & 0x0001) != 0 {
        let mode = VertexMode::from(vertex_mode_raw);
        println!(
            "       -> Vertex Mode: {:?} (Raw: {})",
            mode, vertex_mode_raw
        );
        vertex_mode_opt = Some(mode);
    } else {
        println!("       -> Vertex Mode: None (Flag Bit 0 is off)");
    }

    let mut lighting_mode_opt = None;
    // NifXML: Bit 1 - LIGHTING_MODE_E_A_D
    let lighting_mode_raw = cursor.read_u32::<LittleEndian>()?;
    if (flags & 0x0002) != 0 {
        let mode = LightMode::from(lighting_mode_raw);
        println!(
            "       -> Lighting Mode: {:?} (Raw: {})",
            mode, lighting_mode_raw
        );
        lighting_mode_opt = Some(mode);
    } else {
        println!("       -> Lighting Mode: None (Flag Bit 1 is off)");
    }

    println!(
        "     -> Cursor after NiVertexColorProperty fields: {:#X}",
        cursor.position()
    );

    Ok(NiVertexColorProperty {
        property_base,
        flags,
        vertex_mode: vertex_mode_opt,
        lighting_mode: lighting_mode_opt,
    })
}

// Add Matrix4x4 and Plane helpers
fn read_matrix4x4(cursor: &mut Cursor<&[u8]>) -> Result<Matrix4x4> {
    let mut m = [[0.0f32; 4]; 4];
    // NIF matrices are often row-major in docs, confirm storage order if issues arise
    for r in 0..4 {
        for c in 0..4 {
            m[r][c] = cursor.read_f32::<LittleEndian>()?; // Reads 16 floats = 64 bytes
        }
    }
    Ok(Matrix4x4(m))
}
fn read_text_key(cursor: &mut Cursor<&[u8]>) -> Result<TextKey> {
    let time = cursor.read_f32::<LittleEndian>()?; // 4 bytes for time
    let value_len = cursor.read_u32::<LittleEndian>()?; // 4 bytes for string length
    let value = read_nif_string(cursor, value_len)?; // N bytes for string data
    Ok(TextKey { time, value })
}

// --- Optional: Add Base Parser for NiExtraData ---
// Useful if you parse other types inheriting from NiExtraData
fn parse_niextradata_fields(cursor: &mut Cursor<&[u8]>) -> Result<NiExtraData> {
    println!("     Parsing NiExtraData fields...");
    println!("cursor: {:x}", cursor.position());
    let mut name_len = cursor.read_u32::<LittleEndian>()?;
    if name_len == 0xFFFFFFFF {
        name_len = 0;
    }
    let name = read_nif_string(cursor, name_len)?;
    println!("       -> Name: '{}'", name);
    let mut next_extra_data = None;
    if name_len > 0 {
        next_extra_data = read_link(cursor)?;
    }
    println!("       -> Next Extra Data: {:?}", next_extra_data);
    println!("cursor: {:x}", cursor.position());

    let _unknown_int_1 = cursor.read_u32::<LittleEndian>()?;
    println!("cursor: {:x}", cursor.position());
    Ok(NiExtraData {
        name,
        next_extra_data,
    }) // Include new field
}

// Specific parser for NiTextKeyExtraData (NO CHANGES NEEDED HERE)
// It correctly calls the modified parse_niextradata_fields for its base.
fn parse_nitextkeyextradata_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiTextKeyExtraData> {
    println!(
        "   Parsing NiTextKeyExtraData fields for block {}...",
        block_index
    );
    println!("cursor: {:x}", cursor.position());
    let base_data = parse_niextradata_fields(cursor)?;
    println!("cursor: {:x}", cursor.position());
    let num_keys = cursor.read_u32::<LittleEndian>()?;
    println!("     -> Num Text Keys: {}", num_keys);
    let mut keys_vec = Vec::with_capacity(num_keys as usize);
    if num_keys > 1000 {
        return Err(ParseError::InvalidData("Too many text keys".to_string()));
    }
    for i in 0..num_keys {
        keys_vec.push(read_text_key(cursor)?);
    }
    println!("       -> Read {} text keys.", keys_vec.len());
    Ok(NiTextKeyExtraData {
        base: base_data,
        num_keys,
        keys: keys_vec,
    })
}
fn parse_nidynamiceffect_fields(
    cursor: &mut Cursor<&[u8]>,
    av_base: NiAVObject, // Takes the parsed AVObject base
    block_index: u32,
) -> Result<NiDynamicEffect> {
    println!(
        "     Parsing NiDynamicEffect fields (Block {})...",
        block_index
    );

    let num_affected_nodes = cursor.read_u32::<LittleEndian>()?; // Read u32 count
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

fn parse_nitextureeffect_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiTextureEffect> {
    println!(
        "   Parsing NiTextureEffect fields for block {}...",
        block_index
    );
    let initial_pos = cursor.position();

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
    println!("       -> Cursor after matrix: {:#X}", cursor.position());
    let model_projection_translation = read_vector3(cursor)?;
    println!(
        "       -> Cursor after translation: {:#X}",
        cursor.position()
    );
    let texture_filtering_raw = cursor.read_u32::<LittleEndian>()?;
    println!(
        "       -> Cursor after filter raw: {:#X}",
        cursor.position()
    );
    let texture_clamping_raw = cursor.read_u32::<LittleEndian>()?;
    println!("       -> Cursor after clamp raw: {:#X}", cursor.position());
    let texture_type_raw = cursor.read_u32::<LittleEndian>()?;
    println!("       -> Cursor after type raw: {:#X}", cursor.position());
    let coordinate_generation_type_raw = cursor.read_u32::<LittleEndian>()?;
    println!(
        "       -> Cursor after coordgen raw: {:#X}",
        cursor.position()
    );
    let source_texture = read_link(cursor)?;
    println!(
        "       -> Cursor after source texture link: {:#X}",
        cursor.position()
    );
    let clipping_plane_raw = cursor.read_u8()?;
    println!(
        "       -> Cursor after clipping plane raw: {:#X}",
        cursor.position()
    );
    let enable_plane = clipping_plane_raw != 0;

    println!("       -> Reading Clipping Plane data...");
    let plane_data = Some(read_plane(cursor)?);
    println!(
        "       -> Cursor after reading plane data: {:#X}",
        cursor.position()
    );

    let ps2_l = cursor.read_i16::<LittleEndian>()?;
    println!("       -> Cursor after ps2_l: {:#X}", cursor.position());
    let ps2_k = cursor.read_i16::<LittleEndian>()?;
    println!("       -> Cursor after ps2_k: {:#X}", cursor.position());
    let unknown_short = cursor.read_u16::<LittleEndian>()?;
    println!(
        "       -> Cursor after unknown_short: {:#X}",
        cursor.position()
    );

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
fn read_plane(cursor: &mut Cursor<&[u8]>) -> Result<Plane> {
    let normal = read_vector3(cursor)?; // Reads 3 floats = 12 bytes
    let constant = cursor.read_f32::<LittleEndian>()?; // Reads 1 float = 4 bytes
    Ok(Plane { normal, constant }) // Total 16 bytes
}
fn read_quat_wxyz(cursor: &mut Cursor<&[u8]>) -> Result<Quaternion> {
    let w = cursor.read_f32::<LittleEndian>()?; // Read W component
    let x = cursor.read_f32::<LittleEndian>()?; // Read X component
    let y = cursor.read_f32::<LittleEndian>()?; // Read Y component
    let z = cursor.read_f32::<LittleEndian>()?; // Read Z component
    // Construct Bevy Quat using XYZW order
    Ok(Quat::from_xyzw(x, y, z, w))
}

fn read_key_quat(cursor: &mut Cursor<&[u8]>, key_type: KeyType) -> Result<KeyQuaternion> {
    let time = cursor.read_f32::<LittleEndian>()?;
    let value = read_quat_wxyz(cursor)?;

    let mut fwd = None;
    let mut bwd = None;
    let mut ten = None;
    let mut bia = None;
    let mut con = None;

    match key_type {
        KeyType::Linear | KeyType::Const => {}
        KeyType::Quadratic => {
            // Read Bezier tangents (2 * Quat = 32 bytes)
            println!("       -> Reading Quadratic tangents for Quat key");
            fwd = Some(read_quat_wxyz(cursor)?); // Read forward tangent quat
            bwd = Some(read_quat_wxyz(cursor)?); // Read backward tangent quat
        }
        KeyType::TBC => {
            // Read TBC parameters (3 * f32 = 12 bytes)
            println!("       -> Reading TBC parameters for Quat key");
            ten = Some(cursor.read_f32::<LittleEndian>()?); // Read tension
            bia = Some(cursor.read_f32::<LittleEndian>()?); // Read bias
            con = Some(cursor.read_f32::<LittleEndian>()?); // Read continuity
        }
        _ => {
            return Err(ParseError::InvalidData(format!(
                "Unsupported KeyType {:?} for Quat key",
                key_type
            )));
        }
    }
    Ok(KeyQuaternion {
        time,
        value,
        forward_tangent: fwd,
        backward_tangent: bwd,
        tension: ten,
        bias: bia,
        continuity: con,
    })
}

fn read_key_vec3(cursor: &mut Cursor<&[u8]>, key_type: KeyType) -> Result<KeyVec3> {
    let time = cursor.read_f32::<LittleEndian>()?;
    let value = read_vector3(cursor)?;
    let mut fwd = None;
    let mut bwd = None;
    let mut ten = None;
    let mut bia = None;
    let mut con = None;

    match key_type {
        KeyType::Linear | KeyType::Const => {}
        KeyType::Quadratic => {
            // Read Bezier tangents (2 * Vec3f = 24 bytes)
            println!("       -> Reading Quadratic tangents for Vec3 key");
            fwd = Some(read_vector3(cursor)?); // Read forward tangent vec3
            bwd = Some(read_vector3(cursor)?); // Read backward tangent vec3
        }
        KeyType::TBC => {
            // Read TBC parameters (3 * f32 = 12 bytes)
            println!("       -> Reading TBC parameters for Vec3 key");
            ten = Some(cursor.read_f32::<LittleEndian>()?);
            bia = Some(cursor.read_f32::<LittleEndian>()?);
            con = Some(cursor.read_f32::<LittleEndian>()?);
        }
        _ => {
            return Err(ParseError::InvalidData(format!(
                "Unsupported KeyType {:?} for Vec3 key",
                key_type
            )));
        }
    }
    Ok(KeyVec3 {
        time,
        value,
        forward_tangent: fwd,
        backward_tangent: bwd,
        tension: ten,
        bias: bia,
        continuity: con,
    })
}

fn read_key_float(cursor: &mut Cursor<&[u8]>, key_type: KeyType) -> Result<KeyFloat> {
    let time = cursor.read_f32::<LittleEndian>()?;
    let value = cursor.read_f32::<LittleEndian>()?;
    let mut fwd = None;
    let mut bwd = None;
    let mut ten = None;
    let mut bia = None;
    let mut con = None;

    match key_type {
        KeyType::Linear | KeyType::Const => {}
        KeyType::Quadratic => {
            // Read Bezier tangents for float (2 * f32 = 8 bytes)
            println!("       -> Reading Quadratic tangents for Float key");
            fwd = Some(cursor.read_f32::<LittleEndian>()?); // Read forward tangent float
            bwd = Some(cursor.read_f32::<LittleEndian>()?); // Read backward tangent float
        }
        KeyType::TBC => {
            // Read TBC parameters (3 * f32 = 12 bytes)
            println!("       -> Reading TBC parameters for Float key");
            ten = Some(cursor.read_f32::<LittleEndian>()?);
            bia = Some(cursor.read_f32::<LittleEndian>()?);
            con = Some(cursor.read_f32::<LittleEndian>()?);
        }
        _ => {
            return Err(ParseError::InvalidData(format!(
                "Unsupported KeyType {:?} for float key",
                key_type
            )));
        }
    }
    Ok(KeyFloat {
        time,
        value,
        forward_tangent: fwd,
        backward_tangent: bwd,
        tension: ten,
        bias: bia,
        continuity: con,
    })
}
fn parse_nikeyframedata_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiKeyframeData> {
    println!(
        "   Parsing NiKeyframeData fields for block {}...",
        block_index
    );

    // Rotations (Quaternions)
    let num_rotation_keys = cursor.read_u32::<LittleEndian>()?;
    let mut rotation_type_opt = None; // Renamed to Option
    let mut quaternion_keys = Vec::with_capacity(num_rotation_keys as usize);
    if num_rotation_keys > 0 {
        let rotation_type_raw = cursor.read_u32::<LittleEndian>()?;
        let parsed_rotation_type = KeyType::from(rotation_type_raw);
        rotation_type_opt = Some(parsed_rotation_type); // Store the type
        println!(
            "     -> Rotation Keys: {}, Type: {:?}",
            num_rotation_keys, parsed_rotation_type
        );
        if num_rotation_keys > 10000 {
            return Err(ParseError::InvalidData(
                "Too many rotation keys".to_string(),
            ));
        } // Sanity check
        for _ in 0..num_rotation_keys {
            quaternion_keys.push(read_key_quat(cursor, parsed_rotation_type)?); // Pass type
        }
    } else {
        println!("     -> Rotation Keys: 0");
    }

    // Translations (Vec3)
    let num_translation_keys = cursor.read_u32::<LittleEndian>()?;
    let mut translation_interp = KeyType::Linear; // Default assumption
    let mut translations = Vec::with_capacity(num_translation_keys as usize);
    println!("     -> Translation Keys: {}", num_translation_keys);
    if num_translation_keys > 0 {
        // *** Read the interpolation type for translations ***
        let translation_interp_raw = cursor.read_u32::<LittleEndian>()?;
        translation_interp = KeyType::from(translation_interp_raw);
        println!(
            "       -> Translation Interpolation: {:?}",
            translation_interp
        );

        if num_translation_keys > 10000 {
            return Err(ParseError::InvalidData(
                "Too many translation keys".to_string(),
            ));
        } // Sanity check
        for _ in 0..num_translation_keys {
            // *** Pass the interpolation type to the key reader ***
            translations.push(read_key_vec3(cursor, translation_interp)?);
        }
    }

    // Scales (Float)
    let num_scale_keys = cursor.read_u32::<LittleEndian>()?;
    let mut scale_interp = KeyType::Linear; // Default assumption
    let mut scales = Vec::with_capacity(num_scale_keys as usize);
    println!("     -> Scale Keys: {}", num_scale_keys);
    if num_scale_keys > 0 {
        // *** Read the interpolation type for scales ***
        let scale_interp_raw = cursor.read_u32::<LittleEndian>()?;
        scale_interp = KeyType::from(scale_interp_raw);
        println!("       -> Scale Interpolation: {:?}", scale_interp);

        if num_scale_keys > 10000 {
            return Err(ParseError::InvalidData("Too many scale keys".to_string()));
        } // Sanity check
        for _ in 0..num_scale_keys {
            // *** Pass the interpolation type to the key reader ***
            scales.push(read_key_float(cursor, scale_interp)?);
        }
    }

    let key_data = NiKeyframeData {
        rotation_type: rotation_type_opt,
        quaternion_keys,
        translation_interp,
        translations,
        scale_interp,
        scales,
    };

    println!("   -> Successfully parsed NiKeyframeData fields.");
    Ok(key_data)
}

fn parse_nikeyframecontroller_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiKeyframeController> {
    println!(
        "   Parsing NiKeyframeController fields for block {}...",
        block_index
    );

    // Read NiTimeController base fields (v4.0.0.2 structure)
    let next_controller = read_link(cursor)?;
    let flags = cursor.read_u16::<LittleEndian>()?;
    let frequency = cursor.read_f32::<LittleEndian>()?;
    let phase = cursor.read_f32::<LittleEndian>()?;
    let start_time = cursor.read_f32::<LittleEndian>()?;
    let stop_time = cursor.read_f32::<LittleEndian>()?;
    let target = read_link(cursor)?;

    // Read NiKeyframeController specific field for v4.0.0.2
    let keyframe_data = read_link(cursor)?;

    println!(
        "     -> Flags: {:#06X}, Freq: {}, Start: {}, Stop: {}, Target: {:?}, Data: {:?}",
        flags, frequency, start_time, stop_time, target, keyframe_data
    ); // Added more fields to print

    // Construct struct
    let controller = NiKeyframeController {
        next_controller,
        flags,
        frequency,
        phase,
        start_time,
        stop_time,
        target,
        keyframe_data,
    };

    println!("   -> Successfully parsed NiKeyframeController fields.");
    Ok(controller)
}

fn parse_nigeometrydata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiGeometryData> {
    println!("     Parsing NiGeometryData fields...");
    let num_vertices = cursor.read_u16::<LittleEndian>()?;
    println!("       -> Num Vertices: {}", num_vertices);

    let has_vertices = cursor.read_u32::<LittleEndian>()? != 0; // u32 bool in v4.0.0.2
    println!("       -> Has Vertices: {}", has_vertices);
    let mut vertices = None;
    if has_vertices {
        let mut verts_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            verts_vec.push(read_vector3(cursor)?);
        }
        vertices = Some(verts_vec);
        println!("         (Read {} vertices)", num_vertices);
    }

    let has_normals = cursor.read_u32::<LittleEndian>()? != 0; // u32 bool
    println!("       -> Has Normals: {}", has_normals);
    let mut normals = None;
    if has_normals {
        let mut norms_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            norms_vec.push(read_vector3(cursor)?);
        }
        normals = Some(norms_vec);
        println!("         (Read {} normals)", num_vertices);
    }

    let center = read_vector3(cursor)?;
    let radius = cursor.read_f32::<LittleEndian>()?;
    let bounding_sphere = BoundingSphere { center, radius };
    println!(
        "       -> Bounding Sphere: Center={:?}, Radius={}",
        center.0, radius
    );

    let has_vertex_colors = cursor.read_u32::<LittleEndian>()? != 0; // u32 bool
    println!("       -> Has Vertex Colors: {}", has_vertex_colors);
    let mut vertex_colors = None;
    if has_vertex_colors {
        let mut colors_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            colors_vec.push(read_vector4(cursor)?); // Read RGBA
        }
        vertex_colors = Some(colors_vec);
        println!("         (Read {} vertex colors)", num_vertices);
    }

    let data_flags = cursor.read_u16::<LittleEndian>()?; // Contains num UV sets
    let num_uv_sets = data_flags & 0x3F; // Lower 6 bits often used for count? NifXML implies flags == num for 4.0.0.2
    println!(
        "       -> Data Flags: {:#06X} (Num UV Sets: {})",
        data_flags, num_uv_sets
    );
    let has_uv_flag = cursor.read_u32::<LittleEndian>()? != 0; // Read the 'Has UV' boolean flag (as u32)
    println!("       -> Has UV flag: {}", has_uv_flag);
    let mut uv_sets = Vec::with_capacity(num_uv_sets as usize);
    if num_uv_sets > 4 {
        // Add a sanity check
        println!(
            "       !!!! WARN: Unexpectedly high number of UV sets: {} !!!!",
            num_uv_sets
        );
        // Potentially return error or clamp num_uv_sets? Clamping for now.
        // return Err(ParseError::InvalidData(format!("Too many UV sets: {}", num_uv_sets)));
    }
    if has_uv_flag && num_uv_sets > 0 {
        for set_index in 0..num_uv_sets.min(4) {
            // Read up to 4 sets for safety
            let mut uv_list = Vec::with_capacity(num_vertices as usize);
            for _ in 0..num_vertices {
                uv_list.push(read_vector2(cursor)?);
            }
            uv_sets.push(uv_list);
            println!("         (Read {} UVs for Set {})", num_vertices, set_index);
        }
    }

    Ok(NiGeometryData {
        num_vertices,
        has_vertices,
        vertices,
        has_normals,
        normals,
        bounding_sphere,
        has_vertex_colors,
        vertex_colors,
        num_uv_sets,
        uv_sets,
    })
}

fn parse_nitribasedgeomdata_fields(
    cursor: &mut Cursor<&[u8]>,
    geom_base: NiGeometryData,
    _block_index: u32,
) -> Result<NiTriBasedGeomData> {
    println!("     Parsing NiTriBasedGeomData fields...");
    let num_triangles = cursor.read_u16::<LittleEndian>()?;
    println!("       -> Num Triangles: {}", num_triangles);
    Ok(NiTriBasedGeomData {
        geom_base,
        num_triangles,
    })
}

fn parse_nitrishapedata_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiTriShapeData> {
    println!(
        "   Parsing NiTriShapeData fields for block {}...",
        block_index
    );

    // 1. Parse base geometry fields first
    let geom_part = parse_nigeometrydata_fields(cursor, block_index)?;
    let tri_base_part = parse_nitribasedgeomdata_fields(cursor, geom_part, block_index)?;

    // 2. Parse NiTriShapeData specific fields
    let num_triangle_points = cursor.read_u32::<LittleEndian>()?; // u32 for index count
    println!("     Read Num Triangle Points: {}", num_triangle_points);
    if num_triangle_points as u64 != (tri_base_part.num_triangles as u64 * 3) {
        println!(
            "     !!!! WARN: num_triangle_points ({}) != num_triangles ({}) * 3 !!!!",
            num_triangle_points, tri_base_part.num_triangles
        );
        // Allow parsing to continue but this indicates an issue
    }

    let mut triangles = Vec::with_capacity(num_triangle_points as usize);
    for _ in 0..num_triangle_points {
        triangles.push(cursor.read_u16::<LittleEndian>()?); // Read vertex indices
    }
    println!("       -> Read {} triangle indices.", triangles.len());

    let num_match_groups = cursor.read_u16::<LittleEndian>()?;
    println!("     Read Num Match Groups: {}", num_match_groups);

    let mut match_groups_data = Vec::with_capacity(num_match_groups as usize);
    if num_match_groups > 0 {
        println!("     Reading Match Groups data...");
        if num_match_groups > 1000 {
            // Example limit
            return Err(ParseError::InvalidData(format!(
                "num_match_groups {} too large",
                num_match_groups
            )));
        }
        for group_index in 0..num_match_groups {
            let num_vertices_in_group = cursor.read_u16::<LittleEndian>()?;
            if num_vertices_in_group > 10000 {
                // Example limit
                return Err(ParseError::InvalidData(format!(
                    "num_vertices_in_group {} too large",
                    num_vertices_in_group
                )));
            }
            println!(
                "       Group {}: Num Vertices = {}",
                group_index, num_vertices_in_group
            );

            let mut group_indices = Vec::with_capacity(num_vertices_in_group as usize);
            for _ in 0..num_vertices_in_group {
                group_indices.push(cursor.read_u16::<LittleEndian>()?);
            }
            match_groups_data.push(group_indices);
        }
    } else {
        println!("     No Match Groups data to read.");
    }

    // 4. Construct struct
    let tri_data = NiTriShapeData {
        tri_base: tri_base_part,
        num_triangle_points,
        triangles,
        num_match_groups,
        match_groups: match_groups_data, // Assign the (potentially empty) Vec
    };

    println!("   -> Successfully parsed NiTriShapeData fields.");
    Ok(tri_data)
}
fn parse_nimaterialproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiMaterialProperty> {
    println!(
        "   Parsing NiMaterialProperty fields for block {}...",
        block_index
    );

    // 1. Parse NiObjectNET base fields (as NiProperty adds no fields itself)
    let net_part = parse_niobjectnet_fields(cursor)?;

    // 2. Parse NiMaterialProperty specific fields (v4.0.0.2)
    let flags = cursor.read_u16::<LittleEndian>()?;
    let ambient_color = read_vector3(cursor)?;
    let diffuse_color = read_vector3(cursor)?;
    let specular_color = read_vector3(cursor)?;
    let emissive_color = read_vector3(cursor)?;
    let glossiness = cursor.read_f32::<LittleEndian>()?;
    let alpha = cursor.read_f32::<LittleEndian>()?;
    // No emissive_mult in v4.0.0.2

    println!(
        "     -> Flags: {:#06X}, Gloss: {}, Alpha: {}",
        flags, glossiness, alpha
    );
    // Add printing for colors if needed, but can be verbose

    // 3. Construct struct
    let mat_prop = NiMaterialProperty {
        property_base: NiProperty { net_base: net_part },
        flags,
        ambient_color,
        diffuse_color,
        specular_color,
        emissive_color,
        glossiness,
        alpha,
    };

    println!("   -> Successfully parsed NiMaterialProperty fields.");
    Ok(mat_prop)
}

fn parse_nisourcetexture_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiSourceTexture> {
    println!(
        "   Parsing NiSourceTexture fields for block {}...",
        block_index
    );

    // 1. Parse NiObjectNET base fields
    let net_part = parse_niobjectnet_fields(cursor)?;

    // 2. Parse NiSourceTexture specific fields (v4.0.0.2)

    // Use External flag (read as u8 for v4.0.0.2 based on C++ bool logic)
    let use_external_u8 = cursor.read_u8()?;
    let use_external = use_external_u8 != 0;
    println!(
        "     -> Use External: {} (Byte: {:#04X})",
        use_external, use_external_u8
    );

    // Has Pixel Data flag (only present if use_external is false)
    let mut has_pixel_data = false; // Default
    if !use_external {
        let has_pixel_data_u8 = cursor.read_u8()?;
        has_pixel_data = has_pixel_data_u8 != 0;
        println!(
            "     -> Has Internal Pixel Data: {} (Byte: {:#04X})",
            has_pixel_data, has_pixel_data_u8
        );
    }

    // File Name (only present if use_external is true)
    let mut file_name = None;
    if use_external {
        // Morrowind often stores filename as length-prefixed string here
        let name_len = cursor.read_u32::<LittleEndian>()?;
        let name_str = read_nif_string(cursor, name_len)?;
        println!("     -> External File Name: '{}'", name_str);
        file_name = Some(name_str);
    }

    // Pixel Data Link (only present if NOT use_external AND has_pixel_data)
    let mut pixel_data_link = None;
    if !use_external && has_pixel_data {
        pixel_data_link = read_link(cursor)?;
        println!("     -> Internal Pixel Data Link: {:?}", pixel_data_link);
    }

    // Format Preferences
    let pixel_layout_raw = cursor.read_u32::<LittleEndian>()?;
    let use_mipmaps_raw = cursor.read_u32::<LittleEndian>()?;
    let alpha_format_raw = cursor.read_u32::<LittleEndian>()?;
    let pixel_layout = PixelLayout::from(pixel_layout_raw);
    let use_mipmaps = MipMapFormat::from(use_mipmaps_raw);
    let alpha_format = AlphaFormat::from(alpha_format_raw);
    println!(
        "     -> PixelLayout: {:?}, UseMipmaps: {:?}, AlphaFormat: {:?}",
        pixel_layout, use_mipmaps, alpha_format
    );

    // Is Static flag (read as u8 for v4.0.0.2 based on C++ bool logic)
    let is_static_u8 = cursor.read_u8()?;
    let is_static = is_static_u8 != 0;
    println!(
        "     -> Is Static: {} (Byte: {:#04X})",
        is_static, is_static_u8
    );

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

    println!("   -> Successfully parsed NiSourceTexture fields.");
    Ok(source_texture)
}

fn parse_nitexturingproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiTexturingProperty> {
    println!("   Parsing NiTexturingProperty fields [NifSkope View Logic]...");

    // 1. Parse NiObjectNET base fields shown in NifSkope
    let net_part = parse_niobjectnet_fields(cursor)?; // Reads Name, ExtraLink, ControllerLink (12 bytes total)

    // 2. Parse NiTexturingProperty specific fields shown
    let flags = cursor.read_u16::<LittleEndian>()?; // Reads Flags (2 bytes)
    let apply_mode_raw = cursor.read_u32::<LittleEndian>()?; // Reads Apply Mode (4 bytes)
    let apply_mode = ApplyMode::from(apply_mode_raw);
    let texture_count = cursor.read_u32::<LittleEndian>()?; // Reads Texture Count (4 bytes)
    // Cursor should be at start of Slot 0 data (e.g., 0x1B7)

    println!(
        "     -> Read Header Fields. Flags: {:#06X}, ApplyMode: {:?}, TextureCount: {}",
        flags, apply_mode, texture_count
    );

    // Prepare fields - only base_texture might get full data based on flag
    let mut base_texture_data: Option<TextureData> = None;
    let mut has_dark_texture = false;
    let mut has_detail_texture = false;
    let mut has_gloss_texture = false;
    let mut has_glow_texture = false;
    let mut has_bump_map_texture = false;
    let mut has_decal_0_texture = false;

    // 3. Read Base Texture Slot (Index 0) - Read details shown if enabled
    println!("     Reading Base Texture Slot (Index 0)...");
    if texture_count > 0 {
        // Basic check if slot could exist
        let base_has_texture_u32 = cursor.read_u32::<LittleEndian>()?; // Read 1 byte flag
        let base_has_texture = base_has_texture_u32 != 0;
        println!(
            "       -> Has Base Texture: {} (Byte: {:#04X})",
            base_has_texture, base_has_texture_u32
        );

        if base_has_texture {
            // Read all fields listed for Base Texture in NifSkope
            let source_texture = read_link(cursor)?; // 4 bytes
            let clamp_raw = cursor.read_u32::<LittleEndian>()?; // 4 bytes
            let filter_raw = cursor.read_u32::<LittleEndian>()?; // 4 bytes
            let uv_set = cursor.read_u32::<LittleEndian>()?; // 4 bytes
            let ps2_l = cursor.read_i16::<LittleEndian>()?; // 2 bytes
            let ps2_k = cursor.read_i16::<LittleEndian>()?; // 2 bytes
            let unknown1 = cursor.read_u16::<LittleEndian>()?; // 2 bytes
            // Total 22 bytes read after flag

            let clamp_mode = ClampMode::from(clamp_raw);
            let filter_mode = FilterMode::from(filter_raw);
            println!(
                "         -> Read SourceLink:{:?}, Clamp:{:?}, Filter:{:?}, UVSet:{}, PS2L:{}, PS2K:{}, Unk1:{}",
                source_texture, clamp_mode, filter_mode, uv_set, ps2_l, ps2_k, unknown1
            );

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
            println!("       -> Base Texture Slot Disabled. Reading nothing further for Slot 0.");
            base_texture_data = Some(TextureData {
                has_texture: false,
                ..Default::default()
            });
        }
    } else {
        println!("     Texture Count is 0, skipping Base Texture read.");
    }

    // 4. Read ONLY the boolean flags for subsequent slots (if texture_count implies they exist)
    println!("     Reading only 'Has Texture' flags for subsequent slots...");
    if texture_count > 1 {
        has_dark_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 1 flag only (1 byte)
        println!("       -> Has Dark Texture: {}", has_dark_texture);
    }
    if texture_count > 2 {
        has_detail_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 2 flag only (1 byte)
        println!("       -> Has Detail Texture: {}", has_detail_texture);
    }
    if texture_count > 3 {
        has_gloss_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 3 flag only (1 byte)
        println!("       -> Has Gloss Texture: {}", has_gloss_texture);
    }
    if texture_count > 4 {
        has_glow_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 4 flag only (1 byte)
        println!("       -> Has Glow Texture: {}", has_glow_texture);
    }
    if texture_count > 5 {
        has_bump_map_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 5 flag only (1 byte)
        println!("       -> Has Bump Map Texture: {}", has_bump_map_texture);
        // Do NOT read bump map extra data even if true, per user instruction
    }
    if texture_count > 6 {
        has_decal_0_texture = cursor.read_u32::<LittleEndian>()? != 0; // Read Slot 6 flag only (1 byte)
        println!("       -> Has Decal 0 Texture: {}", has_decal_0_texture);
    }
    // *** STOP READING HERE - Cursor is now misaligned ***

    let final_cursor_pos = cursor.position();
    println!(
        "   -> Parsed NiTexturingProperty fields [NifSkope View Logic]. Cursor ending MISALIGNED at: {:#X}",
        final_cursor_pos
    );

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

// --- Corrected Helper Function for nested TextureData struct ---
// Reads exactly 23 bytes per call (1 flag + 4 link + 4 clamp + 4 filter + 4 uv + 2 ps2l + 2 ps2k + 2 unk1 = 23)
fn read_texture_struct(cursor: &mut Cursor<&[u8]>) -> Result<TextureData> {
    let has_texture_u8 = cursor.read_u8()?;
    let has_texture = has_texture_u8 != 0;
    // println!("       Has Texture: {}", has_texture); // Keep this for debugging if needed

    if has_texture {
        let source_texture = read_link(cursor)?;
        let clamp_raw = cursor.read_u32::<LittleEndian>()?;
        let filter_raw = cursor.read_u32::<LittleEndian>()?;
        let uv_set = cursor.read_u32::<LittleEndian>()?;
        let ps2_l = cursor.read_i16::<LittleEndian>()?; // Read PS2 L
        let ps2_k = cursor.read_i16::<LittleEndian>()?; // Read PS2 K
        let unknown1 = cursor.read_u16::<LittleEndian>()?; // Read Unknown1

        let clamp_mode = ClampMode::from(clamp_raw);
        let filter_mode = FilterMode::from(filter_raw);
        println!(
            "         -> SourceLink: {:?}, Clamp: {:?}, Filter: {:?}, UVSet: {}, PS2L: {}, PS2K: {}, Unk1: {}",
            source_texture, clamp_mode, filter_mode, uv_set, ps2_l, ps2_k, unknown1
        );

        Ok(TextureData {
            has_texture: true,
            source_texture,
            clamp_mode,
            filter_mode,
            uv_set,
            // Store ps2/unk fields if added to struct definition
        })
    } else {
        // Consume exactly 22 bytes to match the 'true' branch size after the flag
        let _ = read_link(cursor)?; // 4 bytes
        let _ = cursor.read_u32::<LittleEndian>()?; // 4 bytes (clamp)
        let _ = cursor.read_u32::<LittleEndian>()?; // 4 bytes (filter)
        let _ = cursor.read_u32::<LittleEndian>()?; // 4 bytes (uvset)
        // Instead of seek, read the values to ensure correct byte count
        let _ = cursor.read_i16::<LittleEndian>()?; // 2 bytes (ps2l)
        let _ = cursor.read_i16::<LittleEndian>()?; // 2 bytes (ps2k)
        let _ = cursor.read_u16::<LittleEndian>()?; // 2 bytes (unk1)
        // Total = 4+4+4+4 + 2+2+2 = 22 bytes consumed

        Ok(TextureData {
            has_texture: false,
            ..Default::default()
        })
    }
} // Total function consumes 1 + 22 = 23 bytes

fn parse_nialphaproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiAlphaProperty> {
    println!("   Parsing NiAlphaProperty fields...");

    // 1. Parse NiObjectNET base fields (as NiProperty doesn't add fields itself)
    let net_part = parse_niobjectnet_fields(cursor)?;

    // 2. Parse NiAlphaProperty specific fields (for v4.0.0.2)
    let flags = cursor.read_u16::<LittleEndian>()?;
    let threshold = cursor.read_u8()?;
    println!("     -> Flags: {:#06X}, Threshold: {}", flags, threshold);

    // 3. Construct nested struct
    let alpha_prop = NiAlphaProperty {
        property_base: NiProperty {
            // Wrap the parsed NiObjectNET part
            net_base: net_part,
        },
        flags,
        threshold,
    };
    println!("   -> Successfully parsed NiAlphaProperty fields.");
    Ok(alpha_prop)
}

fn parse_niobjectnet_fields(cursor: &mut Cursor<&[u8]>) -> Result<NiObjectNET> {
    println!("    Parsing NiObjectNET fields...");
    let name_len = cursor.read_u32::<LittleEndian>()?;
    let name = read_nif_string(cursor, name_len)?;
    let extra_data_link = read_link(cursor)?; // Single link for v4.0.0.2
    let controller_link = read_link(cursor)?;
    println!(
        "      -> Name: '{}', ExtraLink: {:?}, ControllerLink: {:?}",
        name, extra_data_link, controller_link
    );

    Ok(NiObjectNET {
        // base: RecordBase {}, // Assuming no RecordBase fields needed here
        name,
        extra_data_link,
        controller_link,
    })
}
fn parse_niavobject_fields(
    cursor: &mut Cursor<&[u8]>,
    net_base: NiObjectNET,
    _block_index: u32,
) -> Result<NiAVObject> {
    println!("     Parsing NiAVObject fields...");
    let flags = cursor.read_u16::<LittleEndian>()?;
    let translation = read_vector3(cursor)?;
    let rotation = read_matrix3x3(cursor)?;
    let scale = cursor.read_f32::<LittleEndian>()?;
    let velocity = read_vector3(cursor)?;
    let properties = read_link_list(cursor)?;
    let has_bounds_u32 = cursor.read_u32::<LittleEndian>()?;
    let has_bounding_volume = has_bounds_u32 != 0;

    println!( /* ... */ );

    let mut sphere_data: Option<BoundingSphere> = None; // Initialize as None
    if has_bounding_volume {
        println!("       -> Bounding Volume present, reading data...");
        let center = read_vector3(cursor)?; // Read center
        let radius = cursor.read_f32::<LittleEndian>()?; // Read radius
        sphere_data = Some(BoundingSphere { center, radius }); // Store it
        println!("         -> Center:{:?}, Radius:{}", center.0, radius);
    } else {
        println!("       -> No Bounding Volume data present.");
    }

    Ok(NiAVObject {
        net_base,
        flags,
        transform: NiTransform {
            rotation,
            translation,
            scale,
        },
        velocity,
        properties,
        bounding_sphere: sphere_data, // Assign the Option<BoundingSphere>
    })
}
// --- Specific Field Parsing Functions ---

fn parse_ninode_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiNode> {
    println!("   Parsing NiNode fields...");
    let net_base = parse_niobjectnet_fields(cursor)?;
    let av_base = parse_niavobject_fields(cursor, net_base, block_index)?; // Pass base and index

    // 3. Read NiNode Data
    let children = read_link_list(cursor)?;
    let effects = read_link_list(cursor)?;

    // 4. Construct nested struct
    let node_data = NiNode {
        av_base,
        children,
        effects,
    };
    println!("   -> Successfully parsed NiNode fields.");
    Ok(node_data)
}

fn parse_nitrishape_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiTriShape> {
    println!("   Parsing NiTriShape fields...");
    let net_base = parse_niobjectnet_fields(cursor)?;
    let av_base = parse_niavobject_fields(cursor, net_base, block_index)?; // Pass base and index
    // 3. Read NiTriShape specific data
    println!("   Reading NiTriShape specific fields...");
    let data_link = read_link(cursor)?; // Link to NiTriShapeData/NiTriStripsData
    println!("     Data Link: {:?}", data_link);
    let skin_link = read_link(cursor)?; // Link to NiSkinInstance
    println!("     Skin Link: {:?}", skin_link);
    // Shader properties / material properties are usually in the 'properties' list for v4.0.0.2

    // 4. Construct struct
    let trishape_data = NiTriShape {
        av_base,
        data_link,
        skin_link,
    };
    println!("   -> Successfully parsed NiTriShape fields (basic).");
    Ok(trishape_data)
}

// *** ADD OTHER parse_..._fields functions here ***

// --- Helper Functions ---

fn read_nif_string(cursor: &mut Cursor<&[u8]>, len: u32) -> Result<String> {
    if len > 819200 {
        return Err(ParseError::InvalidData(format!(
            "String length too long: {}",
            len
        )));
    }
    if len == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; len as usize];
    cursor.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn read_link(cursor: &mut Cursor<&[u8]>) -> Result<RecordLink> {
    let index = cursor.read_i32::<LittleEndian>()?;
    if index < -1 {
        Err(ParseError::InvalidData(format!(
            "Invalid link index: {}",
            index
        )))
    } else if index == -1 {
        Ok(None)
    } else {
        Ok(Some(index as usize))
    }
}

fn read_vector3(cursor: &mut Cursor<&[u8]>) -> Result<Vector3> {
    Ok(Vector3([
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
    ]))
}

fn read_matrix3x3(cursor: &mut Cursor<&[u8]>) -> Result<Matrix3x3> {
    let mut m = [[0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = cursor.read_f32::<LittleEndian>()?;
        }
    }
    Ok(Matrix3x3(m))
}

fn read_link_list(cursor: &mut Cursor<&[u8]>) -> Result<Vec<RecordLink>> {
    let count = cursor.read_u32::<LittleEndian>()?;
    if count > 50000 {
        return Err(ParseError::InvalidData(format!(
            "Link list count too high: {}",
            count
        )));
    }
    let mut links = Vec::with_capacity(count as usize);
    for _ in 0..count {
        links.push(read_link(cursor)?);
    }
    Ok(links)
}
fn read_vector2(cursor: &mut Cursor<&[u8]>) -> Result<Vector2> {
    Ok(Vector2([
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
    ]))
}
fn read_vector4(cursor: &mut Cursor<&[u8]>) -> Result<Vector4> {
    Ok(Vector4([
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
    ]))
}
