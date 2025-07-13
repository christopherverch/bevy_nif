// src/nif/parser/base_parsers.rs

use super::helpers::{
    Result, read_link, read_link_list, read_matrix3x3, read_nif_string, read_vector3,
};
use crate::{
    Matrix3x3,
    base::{BoundingBox, BoundingCapsule, BoundingVolume, BoundingVolumeType},
    nif::{
        error::ParseError,
        types::{BoundingSphere, NiAVObject, NiNode, NiObjectNET, NiTransform},
    },
};
use bevy::{
    log::{info, warn},
    math::Vec3,
};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

// --- Base Type Parsers ---

pub fn parse_ninode_fields(cursor: &mut Cursor<&[u8]>, block_index: u32) -> Result<NiNode> {
    let net_base = parse_niobjectnet_fields(cursor)?;
    let mut av_base = parse_niavobject_fields(cursor, net_base, block_index)?; // Pass base and index
    if block_index == 0 && !av_base.name.eq_ignore_ascii_case("bip01") {
        // If this is the root node of the file (index 0) and not a skeleton,
        // discard its transform and reset it to the identity.
        av_base.transform = NiTransform {
            rotation: Matrix3x3::default(),
            translation: Vec3::default(),
            scale: 1.0,
        };
    }
    let children = read_link_list(cursor)?;
    let effects = read_link_list(cursor)?;

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
        name,
        extra_data_link,
        controller_link,
    })
}

pub fn parse_niavobject_fields(
    cursor: &mut Cursor<&[u8]>,
    net_base: NiObjectNET, // Assuming NiObjectNET fields were parsed just before this
    _block_index: u32,     // If needed for logging/context
) -> Result<NiAVObject> {
    let flags = cursor.read_u16::<LittleEndian>()?;
    let translation = read_vector3(cursor)?;
    let rotation = read_matrix3x3(cursor)?;
    let scale = cursor.read_f32::<LittleEndian>()?;
    let velocity = read_vector3(cursor)?;
    let properties = read_link_list(cursor)?;

    let has_bounding_volume = cursor.read_u32::<LittleEndian>()? != 0;
    let mut bounding_volume_data: Option<BoundingVolume> = None;

    if has_bounding_volume {
        // Call the recursive helper function.
        match read_bounding_volume(cursor) {
            Ok(bv) => bounding_volume_data = Some(bv),
            Err(e) => warn!("Failed to parse a bounding volume: {:?}", e),
        }
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
        bounding_volume: bounding_volume_data,
    })
}

/// Recursively reads a BoundingVolume from the stream.
/// This is necessary to handle nested volumes like Union.
fn read_bounding_volume(cursor: &mut Cursor<&[u8]>) -> Result<BoundingVolume> {
    let bv_type_raw = cursor.read_u32::<LittleEndian>()?;
    let bounding_volume_type = BoundingVolumeType::try_from(bv_type_raw)?;

    match bounding_volume_type {
        BoundingVolumeType::Sphere => {
            let center = read_vector3(cursor)?;
            let radius = cursor.read_f32::<LittleEndian>()?;
            Ok(BoundingVolume::Sphere(BoundingSphere { center, radius }))
        }
        BoundingVolumeType::Box => {
            let center = read_vector3(cursor)?;
            let axes = read_matrix3x3(cursor)?;
            let extent = read_vector3(cursor)?;
            Ok(BoundingVolume::Box(BoundingBox {
                center,
                axes,
                extent,
            }))
        }
        BoundingVolumeType::Capsule => {
            let center = read_vector3(cursor)?;
            let axis = read_vector3(cursor)?;
            let extent = cursor.read_f32::<LittleEndian>()?;
            let radius = cursor.read_f32::<LittleEndian>()?;
            Ok(BoundingVolume::Capsule(BoundingCapsule {
                center,
                axis,
                extent,
                radius,
            }))
        }
        BoundingVolumeType::Union => {
            let num_children = cursor.read_u32::<LittleEndian>()?;
            let mut children = Vec::with_capacity(num_children as usize);
            for _ in 0..num_children {
                // The recursive call to handle nested volumes.
                children.push(read_bounding_volume(cursor)?);
            }
            Ok(BoundingVolume::Union(children))
        }
        _ => Err(ParseError::InvalidData(format!(
            "Parsing for BoundingVolume type {:?} is not implemented.",
            bounding_volume_type
        ))),
    }
}
