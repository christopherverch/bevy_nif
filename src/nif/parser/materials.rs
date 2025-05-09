use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor; // Added Seek for potential future use

use crate::nif::error::Result;
use crate::nif::parser::base_parsers::parse_niobjectnet_fields;
use crate::nif::parser::helpers::read_vector3;
// Import error types (assuming src/error.rs)
use crate::nif::types::*;
// Import definitions from structs module
pub fn parse_nimaterialproperty_fields(
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
