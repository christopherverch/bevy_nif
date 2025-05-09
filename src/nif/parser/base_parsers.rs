// src/nif/parser/base_parsers.rs

// --- Imports ---
use super::helpers::{
    Result, read_link, read_link_list, read_matrix3x3, read_nif_string, read_vector3,
}; // Import helpers from the same module
use crate::{
    base::{BoundingBox, BoundingVolume},
    nif::{
        error::ParseError,
        types::{BoundingSphere, NiAVObject, NiNode, NiObjectNET, NiTransform},
    },
}; // Adjust path if needed
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

// --- Base Type Parsers ---

pub fn parse_ninode_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiNode> {
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
    Ok(node_data)
}
pub fn parse_niobjectnet_fields(cursor: &mut Cursor<&[u8]>) -> Result<NiObjectNET> {
    let name_len = cursor.read_u32::<LittleEndian>()?;
    let name = read_nif_string(cursor, name_len)?;
    let extra_data_link = read_link(cursor)?; // Single link for v4.0.0.2
    let controller_link = read_link(cursor)?;

    Ok(NiObjectNET {
        // base: RecordBase {}, // Assuming no RecordBase fields needed here
        name,
        extra_data_link,
        controller_link,
    })
}

pub fn parse_niavobject_fields(
    cursor: &mut Cursor<&[u8]>,
    net_base: NiObjectNET, // Assuming NiObjectNET fields were parsed just before this
    _block_index: u32,     // Keep if needed for logging/context
) -> Result<NiAVObject> {
    let flags = cursor.read_u16::<LittleEndian>()?;
    let translation = read_vector3(cursor)?;
    let rotation = read_matrix3x3(cursor)?;
    let scale = cursor.read_f32::<LittleEndian>()?;

    // Velocity might not exist in all NIF versions for NiAVObject
    // Check NIF 4.0.0.2 spec - assuming it does for now based on original code
    let velocity = read_vector3(cursor)?;

    // --- CRITICAL: Field Order ---
    // Verify if 'properties' comes BEFORE bounding volume in NIF 4.0.0.2 NiAVObject
    // If the "Link list count too high" error happened *after* reading bounds,
    // then properties (or children in NiNode) are likely read *later*.
    // Assuming for now based on your original code it's read here:
    let properties = read_link_list(cursor)?; // Read properties link list
    // --- End Critical Section ---

    // Read the bounding volume type indicator (typically u32)
    let has_bounding_volume = cursor.read_u32::<LittleEndian>()? != 0;
    let mut bounding_volume_data = None;
    if has_bounding_volume {
        let bounding_volume_type = cursor.read_u32::<LittleEndian>()?;

        bounding_volume_data = match bounding_volume_type {
            0 => {
                // Sphere: Center (Vector3), Radius (f32)
                let center = read_vector3(cursor)?;
                let radius = cursor.read_f32::<LittleEndian>()?;
                Some(BoundingVolume::Sphere(BoundingSphere { center, radius }))
            }
            1 => {
                // Box: Center (Vector3), Axes (Matrix3x3), Extent (Vector3)
                let center = read_vector3(cursor)?;
                let axes = read_matrix3x3(cursor)?; // Rotation matrix for the box
                let extent = read_vector3(cursor)?; // Half-dimensions along each axis
                Some(BoundingVolume::Box(BoundingBox {
                    center,
                    axes,
                    extent,
                }))
            }
            // Add cases for other known types (Capsule=3, Union=4, HalfSpace=5) if needed
            // 3 => { ... read Capsule ... }
            _ => {
                return Err(ParseError::InvalidData(
                    "bounding box check not implemented!".to_string(),
                ));
                // Option 2: Ignore and continue (set to None), might hide errors
                // None
            }
        };
    }

    // Construct the NiAVObject
    Ok(NiAVObject {
        net_base,
        flags,
        transform: NiTransform {
            rotation,
            translation,
            scale,
        },
        velocity,
        properties,                            // Assign the parsed properties list
        bounding_volume: bounding_volume_data, // Assign the parsed bounding volume data
    })
}
