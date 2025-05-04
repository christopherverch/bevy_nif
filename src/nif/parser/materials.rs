use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor; // Added Seek for potential future use

use crate::nif::error::Result;
use crate::nif::parser::base_parsers::parse_niobjectnet_fields;
// Import error types (assuming src/error.rs)
use crate::nif::types::*;
// Import definitions from structs module
pub fn parse_nialphaproperty_fields(
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
