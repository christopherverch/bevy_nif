use crate::nif::error::{ParseError, Result};

use crate::nif::parser::animation::*;
use crate::nif::parser::base_parsers::parse_ninode_fields;
use crate::nif::parser::effects_properties::*;
use crate::nif::parser::extra_data::{
    parse_nistringextradata_fields, parse_nitextkeyextradata_fields,
};
use crate::nif::parser::helpers::*;
use crate::nif::parser::materials::parse_nimaterialproperty_fields;
use crate::nif::parser::morph::{parse_nigeommorphercontroller_fields, parse_nimorphdata_fields};
use crate::nif::parser::texture::*;
use crate::nif::parser::triangles::{parse_nitrishape_fields, parse_nitrishapedata_fields};
use crate::nif::types::*;
use bevy::log::warn;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};
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
        warn!(
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
        let block_type_len = cursor.read_u32::<LittleEndian>()?;
        let block_type_name = read_nif_string(&mut cursor, block_type_len)?;

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
            "NiSkinData" => parse_niskindata_fields(&mut cursor, i).map(ParsedBlock::SkinData),
            "NiSkinInstance" => {
                parse_niskininstance_fields(&mut cursor, i).map(ParsedBlock::SkinInstance)
            }
            "NiWireframeProperty" => parse_niwireframe_property_fields(&mut cursor, i)
                .map(ParsedBlock::WireframeProperty),
            "NiSequenceStreamHelper" => parse_nisequencestreamhelper_fields(&mut cursor, i)
                .map(ParsedBlock::SequenceStreamHelper),
            "NiStringExtraData" => {
                parse_nistringextradata_fields(&mut cursor, i).map(ParsedBlock::StringExtraData) // Ensure ParsedBlock has a matching variant
            }
            unknown_type => {
                warn!(
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
                warn!("   Failed to parse block {}: {:?}", i, e);
                // Stop parsing on the first error
                return Err(e);
            }
        }
    }

    Ok(ParsedNifData { header, blocks })
}

// Reads exactly 23 bytes per call (1 flag + 4 link + 4 clamp + 4 filter + 4 uv + 2 ps2l + 2 ps2k + 2 unk1 = 23)
fn _read_texture_struct(cursor: &mut Cursor<&[u8]>) -> Result<TextureData> {
    let has_texture_u8 = cursor.read_u8()?;
    let has_texture = has_texture_u8 != 0;

    if has_texture {
        let source_texture = read_link(cursor)?;
        let clamp_raw = cursor.read_u32::<LittleEndian>()?;
        let filter_raw = cursor.read_u32::<LittleEndian>()?;
        let uv_set = cursor.read_u32::<LittleEndian>()?;
        let _ps2_l = cursor.read_i16::<LittleEndian>()?; // Read PS2 L
        let _ps2_k = cursor.read_i16::<LittleEndian>()?; // Read PS2 K
        let _unknown1 = cursor.read_u16::<LittleEndian>()?; // Read Unknown1

        let clamp_mode = ClampMode::from(clamp_raw);
        let filter_mode = FilterMode::from(filter_raw);

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
