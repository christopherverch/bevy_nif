// src/nif/parser/block_parsers.rs

// --- Imports ---
use super::base_parsers::{parse_niavobject_fields, parse_niobjectnet_fields}; // Import base parsers
use super::helpers::{Result, read_link, read_link_list}; // Import necessary helpers
use crate::nif::types::{NiAlphaProperty, NiNode, NiProperty, NiTriShape}; // Adjust path if needed
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

// --- Specific Block Parsers ---

// Note: Relies on NiNode struct being defined elsewhere. Uses base parsers.
pub fn parse_ninode_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiNode> {
    println!("   Parsing NiNode fields..."); // Keeping your original prints
    let net_base = parse_niobjectnet_fields(cursor)?; // Uses base parser
    let av_base = parse_niavobject_fields(cursor, net_base, block_index)?; // Uses base parser
    // 3. Read NiNode Data (as in original code)
    let children = read_link_list(cursor)?; // Uses helper
    let effects = read_link_list(cursor)?; // Uses helper
    // 4. Construct nested struct (as in original code)
    let node_data = NiNode {
        av_base,
        children,
        effects,
    };
    println!("   -> Successfully parsed NiNode fields.");
    Ok(node_data)
}

// Note: Relies on NiTriShape struct being defined elsewhere. Uses base parsers.
pub fn parse_nitrishape_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiTriShape> {
    println!("   Parsing NiTriShape fields..."); // Keeping your original prints
    let net_base = parse_niobjectnet_fields(cursor)?; // Uses base parser
    let av_base = parse_niavobject_fields(cursor, net_base, block_index)?; // Uses base parser

    // 3. Read NiTriShape specific data (as in original code)
    println!("   Reading NiTriShape specific fields...");
    let data_link = read_link(cursor)?; // Uses helper
    println!("     Data Link: {:?}", data_link);
    let skin_link = read_link(cursor)?; // Uses helper
    println!("     Skin Link: {:?}", skin_link);
    // Shader properties / material properties are usually in the 'properties' list for v4.0.0.2

    // 4. Construct struct (as in original code)
    let trishape_data = NiTriShape {
        av_base,
        data_link,
        skin_link,
    };
    println!("   -> Successfully parsed NiTriShape fields (basic).");
    Ok(trishape_data)
}

// Note: Relies on NiAlphaProperty and NiProperty structs being defined elsewhere. Uses base parser.
pub fn parse_nialphaproperty_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32, // Keep parameter as in original code
) -> Result<NiAlphaProperty> {
    println!("   Parsing NiAlphaProperty fields..."); // Keeping your original prints

    // 1. Parse NiObjectNET base fields (as in original code)
    let net_part = parse_niobjectnet_fields(cursor)?; // Uses base parser

    // 2. Parse NiAlphaProperty specific fields (as in original code)
    let flags = cursor.read_u16::<LittleEndian>()?;
    let threshold = cursor.read_u8()?;
    println!("     -> Flags: {:#06X}, Threshold: {}", flags, threshold);

    // 3. Construct nested struct (as in original code)
    let alpha_prop = NiAlphaProperty {
        property_base: NiProperty {
            // Assumes NiProperty struct exists
            // Wrap the parsed NiObjectNET part
            net_base: net_part,
        },
        flags,
        threshold,
    };
    println!("   -> Successfully parsed NiAlphaProperty fields.");
    Ok(alpha_prop)
}
