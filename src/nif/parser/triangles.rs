use crate::nif::error::{ParseError, Result};
use crate::nif::parser::animation::{parse_nigeometrydata_fields, parse_nitribasedgeomdata_fields};
use crate::nif::parser::base_parsers::{parse_niavobject_fields, parse_niobjectnet_fields};
use crate::nif::parser::helpers::*;
use crate::nif::types::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub fn parse_nitrishapedata_fields(
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

pub fn parse_nitrishape_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiTriShape> {
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
