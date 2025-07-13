use crate::animation::{
    BoneData, BoneVertData, NiSequenceStreamHelper, NiSkinData, NiSkinInstance, NifAxisOrder,
};
use crate::nif::error::{ParseError, Result};
use crate::nif::parser::base_parsers::parse_niobjectnet_fields;
use crate::nif::parser::helpers::*;
use crate::nif::types::*;
use bevy::log::warn;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Seek}; // Added Seek for potential future use
pub fn parse_nikeyframecontroller_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiKeyframeController> {
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

    Ok(controller)
}

pub fn parse_nigeometrydata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiGeometryData> {
    let num_vertices = cursor.read_u16::<LittleEndian>()?;

    let has_vertices = cursor.read_u32::<LittleEndian>()? != 0; // u32 bool in v4.0.0.2
    let mut vertices = None;
    if has_vertices {
        let mut verts_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            verts_vec.push(read_vector3(cursor)?);
        }
        vertices = Some(verts_vec);
    }

    let has_normals = cursor.read_u32::<LittleEndian>()? != 0; // u32 bool
    let mut normals = None;
    if has_normals {
        let mut norms_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            norms_vec.push(read_vector3(cursor)?);
        }
        normals = Some(norms_vec);
    }

    let center = read_vector3(cursor)?;
    let radius = cursor.read_f32::<LittleEndian>()?;
    let bounding_sphere = BoundingSphere { center, radius };

    let has_vertex_colors = cursor.read_u32::<LittleEndian>()? != 0; // u32 bool
    let mut vertex_colors = None;
    if has_vertex_colors {
        let mut colors_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            colors_vec.push(read_vector4(cursor)?); // Read RGBA
        }
        vertex_colors = Some(colors_vec);
    }

    let data_flags = cursor.read_u16::<LittleEndian>()?; // Contains num UV sets
    let num_uv_sets = data_flags & 0x3F; // Lower 6 bits often used for count? NifXML implies flags == num for 4.0.0.2
    let has_uv_flag = cursor.read_u32::<LittleEndian>()? != 0; // Read the 'Has UV' boolean flag (as u32)
    let mut uv_sets = Vec::with_capacity(num_uv_sets as usize);
    if num_uv_sets > 4 {
        // Add a sanity check
        warn!(
            "       Unexpectedly high number of UV sets: {} !",
            num_uv_sets
        );
        // Potentially return error or clamp num_uv_sets? Clamping for now.
        // return Err(ParseError::InvalidData(format!("Too many UV sets: {}", num_uv_sets)));
    }
    if has_uv_flag && num_uv_sets > 0 {
        for _set_index in 0..num_uv_sets.min(4) {
            // Read up to 4 sets for safety
            let mut uv_list = Vec::with_capacity(num_vertices as usize);
            for _ in 0..num_vertices {
                uv_list.push(read_vector2(cursor)?);
            }
            uv_sets.push(uv_list);
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

pub fn parse_nikeyframedata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiKeyframeData> {
    // --- Rotation Data ---
    let num_rotation_keys_for_quat = cursor.read_u32::<LittleEndian>()?; // This count is for Quaternion keys if type is not XYZ
    let rotation_type_raw = cursor.read_u32::<LittleEndian>()?;
    let parsed_rotation_type = KeyType::from(rotation_type_raw); //

    let rotation_type_opt: Option<KeyType>;
    let mut quaternion_keys: Vec<KeyQuaternion> = Vec::new();
    let mut x_rotation_interp_opt: Option<KeyType> = None;
    let mut x_rotation_keys_opt: Option<Vec<KeyFloat>> = None;
    let mut y_rotation_interp_opt: Option<KeyType> = None;
    let mut y_rotation_keys_opt: Option<Vec<KeyFloat>> = None;
    let mut z_rotation_interp_opt: Option<KeyType> = None;
    let mut z_rotation_keys_opt: Option<Vec<KeyFloat>> = None;
    let mut axis_order_opt: Option<NifAxisOrder> = None;

    if parsed_rotation_type == KeyType::XyzRotation {
        //
        rotation_type_opt = Some(KeyType::XyzRotation);
        // NIF spec for 4.0.0.2: if Rotation Type is XYZ_ROTATION_KEY, num_rotation_keys (for quats) must be 0.
        if num_rotation_keys_for_quat != 0 {
            warn!(
                "NiKeyframeData: Rotation type is XYZ but Num Rotation Keys (for Quat) is {}. Expected 0.",
                num_rotation_keys_for_quat
            );
        }

        let (x_interp, x_keys) = parse_float_key_list(cursor)?;
        x_rotation_interp_opt = Some(x_interp);
        x_rotation_keys_opt = Some(x_keys);

        let (y_interp, y_keys) = parse_float_key_list(cursor)?;
        y_rotation_interp_opt = Some(y_interp);
        y_rotation_keys_opt = Some(y_keys);

        let (z_interp, z_keys) = parse_float_key_list(cursor)?;
        z_rotation_interp_opt = Some(z_interp);
        z_rotation_keys_opt = Some(z_keys);

        let axis_order_raw = cursor.read_u32::<LittleEndian>()?;
        axis_order_opt = Some(NifAxisOrder::from(axis_order_raw));
    } else if num_rotation_keys_for_quat > 0 {
        rotation_type_opt = Some(parsed_rotation_type);
        quaternion_keys.reserve(num_rotation_keys_for_quat as usize);
        if num_rotation_keys_for_quat > 10000 {
            // Sanity check from your original code
            return Err(ParseError::InvalidData(
                "Too many rotation keys".to_string(),
            ));
        }
        for _ in 0..num_rotation_keys_for_quat {
            quaternion_keys.push(read_key_quat(cursor, parsed_rotation_type)?); //
        }
    } else {
        // No rotation keys (and not XYZ type explicitly)
        rotation_type_opt = Some(parsed_rotation_type); // Still store the type, even if no keys
        // or None if type is only valid with keys
    }

    // --- Translations (Vec3) ---
    let num_translation_keys = cursor.read_u32::<LittleEndian>()?; //
    let mut translation_interp = KeyType::Linear; // Default assumption
    let mut translations = Vec::with_capacity(num_translation_keys as usize);
    if num_translation_keys > 0 {
        let translation_interp_raw = cursor.read_u32::<LittleEndian>()?; //
        translation_interp = KeyType::from(translation_interp_raw); //

        if num_translation_keys > 10000 {
            // Sanity check
            return Err(ParseError::InvalidData(
                "Too many translation keys".to_string(),
            ));
        }
        for _ in 0..num_translation_keys {
            translations.push(read_key_vec3(cursor, translation_interp)?); //
        }
    }

    // --- Scales (Float) ---
    let num_scale_keys = cursor.read_u32::<LittleEndian>()?; //
    let mut scale_interp = KeyType::Linear; // Default assumption
    let mut scales = Vec::with_capacity(num_scale_keys as usize);
    if num_scale_keys > 0 {
        let scale_interp_raw = cursor.read_u32::<LittleEndian>()?; //
        scale_interp = KeyType::from(scale_interp_raw); //

        if num_scale_keys > 10000 {
            // Sanity check
            return Err(ParseError::InvalidData("Too many scale keys".to_string()));
        }
        for _ in 0..num_scale_keys {
            scales.push(read_key_float(cursor, scale_interp)?); //
        }
    }

    let key_data = NiKeyframeData {
        rotation_type: rotation_type_opt,
        quaternion_keys,
        x_rotation_interp: x_rotation_interp_opt,
        x_rotation_keys: x_rotation_keys_opt,
        y_rotation_interp: y_rotation_interp_opt,
        y_rotation_keys: y_rotation_keys_opt,
        z_rotation_interp: z_rotation_interp_opt,
        z_rotation_keys: z_rotation_keys_opt,
        axis_order: axis_order_opt,
        translation_interp,
        translations,
        scale_interp,
        scales,
    };
    Ok(key_data) //
}
// Helper function to parse a list of float keys (for XYZ components)
fn parse_float_key_list(cursor: &mut Cursor<&[u8]>) -> Result<(KeyType, Vec<KeyFloat>)> {
    let num_keys = cursor.read_u32::<LittleEndian>()?;
    let mut interp_type = KeyType::Linear; // Default or read if present
    let mut keys = Vec::with_capacity(num_keys as usize);

    if num_keys > 0 {
        let interp_raw = cursor.read_u32::<LittleEndian>()?;
        interp_type = KeyType::from(interp_raw);

        if num_keys > 10000 {
            // Sanity check from your original code
            return Err(ParseError::InvalidData(format!(
                "Too many float keys: {}",
                num_keys
            )));
        }
        for _ in 0..num_keys {
            keys.push(read_key_float(cursor, interp_type)?); //
        }
    }
    Ok((interp_type, keys))
}
pub fn parse_nitribasedgeomdata_fields(
    cursor: &mut Cursor<&[u8]>,
    geom_base: NiGeometryData,
    _block_index: u32,
) -> Result<NiTriBasedGeomData> {
    let num_triangles = cursor.read_u16::<LittleEndian>()?;
    Ok(NiTriBasedGeomData {
        geom_base,
        num_triangles,
    })
}
pub fn parse_niskininstance_fields(
    cursor: &mut Cursor<&[u8]>,
    block_index: u32,
) -> Result<NiSkinInstance> {
    // NiObject base has no fields to read here

    // Read fields for v4.0.0.2
    let data_link = read_link(cursor)?; // 4 bytes: Link to NiSkinData
    let skeleton_root_link = read_link(cursor)?; // 4 bytes: Link to skeleton root NiNode
    let num_bones = cursor.read_u32::<LittleEndian>()?; // 4 bytes: Count of bones

    // Read the list of bone node links
    let mut bones_vec = Vec::with_capacity(num_bones as usize);
    // Add a sanity check for bone count
    if num_bones > 256 {
        // Max bones influencing a single mesh part is usually low
        return Err(ParseError::InvalidData(format!(
            "Suspiciously high bone count ({}) in NiSkinInstance block {}",
            num_bones, block_index
        )));
    }
    for _ in 0..num_bones {
        bones_vec.push(read_link(cursor)?); // Read i32 link for each bone (N * 4 bytes)
    }

    // NiSkinPartition link is not present in this version

    Ok(NiSkinInstance {
        data: data_link,
        skeleton_root: skeleton_root_link,
        num_bones,
        bones: bones_vec,
    })
}
pub fn parse_niskindata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiSkinData> {
    // Read overall skin transform (as shown in NifSkope)
    let skin_transform = read_nif_transform(cursor)?; // Reads 52 bytes total

    let num_bones = cursor.read_u32::<LittleEndian>()?; // 4 bytes (uint in NifSkope)

    // Skip Skin Partition Link (i32 Ref<NiSkinPartition>) - NifSkope shows None/Invalid(-1)
    // and spec versions it later than 4.0.0.2. Let's explicitly skip 4 bytes.
    // Note: If this causes issues, verify if v4.0.0.2 *always* omits this.
    cursor.seek(std::io::SeekFrom::Current(4))?;

    // Sanity check bone count
    if num_bones > 256 {
        return Err(ParseError::InvalidData(
            "Too many bones in NiSkinData".to_string(),
        ));
    }

    let mut bone_data_list = Vec::with_capacity(num_bones as usize);
    for _bone_idx in 0..num_bones {
        // Read the bone's transform (NiTransform)
        let bone_transform = read_nif_transform(cursor)?; // Reads 52 bytes

        // Read bounding sphere info
        let bs_offset = read_vector3(cursor)?; // 12 bytes
        let bs_radius = cursor.read_f32::<LittleEndian>()?; // 4 bytes

        // *** REMOVED read for 16 unknown bytes ***

        // Read vertex weight info for this bone
        let num_vertices_for_bone = cursor.read_u16::<LittleEndian>()?; // 2 bytes (ushort in NifSkope)

        // Sanity check vertex count
        if num_vertices_for_bone > 65500 {
            return Err(ParseError::InvalidData(
                "Too many verts for bone".to_string(),
            ));
        }

        let mut vert_weights = Vec::with_capacity(num_vertices_for_bone as usize);
        for _ in 0..num_vertices_for_bone {
            let vertex_index = cursor.read_u16::<LittleEndian>()?; // 2 bytes (ushort)
            let vertex_weight = cursor.read_f32::<LittleEndian>()?; // 4 bytes (float)
            vert_weights.push(BoneVertData {
                index: vertex_index,
                weight: vertex_weight,
            });
        }

        bone_data_list.push(BoneData {
            bone_transform, // Use the NiTransform struct
            bounding_sphere_offset: bs_offset,
            bounding_sphere_radius: bs_radius,
            // unknown_16_bytes removed
            num_vertices: num_vertices_for_bone,
            vertex_weights: vert_weights,
        });
    } // End bone loop

    Ok(NiSkinData {
        skin_transform, // Use the NiTransform struct
        num_bones,
        bone_list: bone_data_list,
    })
}
pub fn parse_nisequencestreamhelper_fields(
    cursor: &mut Cursor<&[u8]>, // Takes a mutable reference to Cursor<&[u8]>
    _block_index: u32,          // Takes the block index
) -> Result<NiSequenceStreamHelper> {
    Ok(NiSequenceStreamHelper {
        net_base: parse_niobjectnet_fields(cursor)?,
    })
}
