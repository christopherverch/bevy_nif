// src/nif/parser/helpers.rs

// --- Imports ---
// Assuming ParseError and structs like Vector3, Matrix3x3, RecordLink, Vector2, Vector4 are defined
// in the parent `nif` module (e.g., src/nif/structs.rs) or crate root (e.g., src/error.rs)
// Adjust these paths as needed based on your project structure.
use crate::NiTransform; // Adjust path if needed
use crate::nif::error::ParseError; // Adjust path if needed
use crate::nif::types::{
    KeyFloat, KeyQuaternion, KeyType, KeyVec3, Matrix3x3, Plane, Quaternion, RecordLink, Vector2,
    Vector3, Vector4,
};
use bevy::math::Quat;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

// --- Result Type ---
// Assuming ParseError is defined elsewhere (e.g., crate::error::ParseError)
pub type Result<T> = std::result::Result<T, ParseError>;

// --- Helper Functions ---

pub fn read_nif_string(cursor: &mut Cursor<&[u8]>, len: u32) -> Result<String> {
    if len > 819200 {
        // Using the arbitrary limit from your original code
        return Err(ParseError::InvalidData(format!(
            "String length too long: {}",
            len
        )));
    }
    if len == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; len as usize];
    cursor.read_exact(&mut buf)?; // Relies on From<std::io::Error> for ParseError or map_err
    Ok(String::from_utf8_lossy(&buf).to_string())
}

pub fn read_link(cursor: &mut Cursor<&[u8]>) -> Result<RecordLink> {
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

pub fn read_vector3(cursor: &mut Cursor<&[u8]>) -> Result<Vector3> {
    Ok(Vector3([
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
    ]))
}

pub fn read_matrix3x3(cursor: &mut Cursor<&[u8]>) -> Result<Matrix3x3> {
    let mut m = [[0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            m[i][j] = cursor.read_f32::<LittleEndian>()?;
        }
    }
    Ok(Matrix3x3(m))
}

pub fn read_link_list(cursor: &mut Cursor<&[u8]>) -> Result<Vec<RecordLink>> {
    let count = cursor.read_u32::<LittleEndian>()?;
    if count > 50000 {
        // Using the arbitrary limit from your original code
        return Err(ParseError::InvalidData(format!(
            "Link list count too high: {}",
            count
        )));
    }
    let mut links = Vec::with_capacity(count as usize);
    for _ in 0..count {
        links.push(read_link(cursor)?); // Uses read_link from this module
    }
    Ok(links)
}

pub fn read_vector2(cursor: &mut Cursor<&[u8]>) -> Result<Vector2> {
    Ok(Vector2([
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
    ]))
}

pub fn read_vector4(cursor: &mut Cursor<&[u8]>) -> Result<Vector4> {
    Ok(Vector4([
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
        cursor.read_f32::<LittleEndian>()?,
    ]))
}
pub fn read_key_float(cursor: &mut Cursor<&[u8]>, key_type: KeyType) -> Result<KeyFloat> {
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
            fwd = Some(cursor.read_f32::<LittleEndian>()?); // Read forward tangent float
            bwd = Some(cursor.read_f32::<LittleEndian>()?); // Read backward tangent float
        }
        KeyType::TBC => {
            // Read TBC parameters (3 * f32 = 12 bytes)
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

pub fn read_plane(cursor: &mut Cursor<&[u8]>) -> Result<Plane> {
    let normal = read_vector3(cursor)?; // Reads 3 floats = 12 bytes
    let constant = cursor.read_f32::<LittleEndian>()?; // Reads 1 float = 4 bytes
    Ok(Plane { normal, constant }) // Total 16 bytes
}
pub fn read_quat_wxyz(cursor: &mut Cursor<&[u8]>) -> Result<Quaternion> {
    let w = cursor.read_f32::<LittleEndian>()?; // Read W component
    let x = cursor.read_f32::<LittleEndian>()?; // Read X component
    let y = cursor.read_f32::<LittleEndian>()?; // Read Y component
    let z = cursor.read_f32::<LittleEndian>()?; // Read Z component
    // Construct Bevy Quat using XYZW order
    Ok(Quat::from_xyzw(x, y, z, w))
}

pub fn read_key_quat(cursor: &mut Cursor<&[u8]>, key_type: KeyType) -> Result<KeyQuaternion> {
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
            fwd = Some(read_quat_wxyz(cursor)?); // Read forward tangent quat
            bwd = Some(read_quat_wxyz(cursor)?); // Read backward tangent quat
        }
        KeyType::TBC => {
            // Read TBC parameters (3 * f32 = 12 bytes)
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

pub fn read_key_vec3(cursor: &mut Cursor<&[u8]>, key_type: KeyType) -> Result<KeyVec3> {
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
            fwd = Some(read_vector3(cursor)?); // Read forward tangent vec3
            bwd = Some(read_vector3(cursor)?); // Read backward tangent vec3
        }
        KeyType::TBC => {
            // Read TBC parameters (3 * f32 = 12 bytes)
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
pub fn read_nif_transform(cursor: &mut Cursor<&[u8]>) -> Result<NiTransform> {
    let rotation = read_matrix3x3(cursor)?; // Reads 36 bytes
    let translation = read_vector3(cursor)?; // Reads 12 bytes
    let scale = cursor.read_f32::<LittleEndian>()?; // Reads 4 bytes
    Ok(NiTransform {
        rotation,
        translation,
        scale,
    })
}

/// Reads 'count' u32 values from the cursor.
pub fn read_u32_arr(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<u32>> {
    const MAX_ARRAY_COUNT: usize = 1_000_000;
    if count > MAX_ARRAY_COUNT {
        // Use the custom error type
        return Err(ParseError::InvalidData(format!(
            "Array count {} exceeds maximum allowed limit {}",
            count, MAX_ARRAY_COUNT
        )));
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        // Use '?' which leverages From<std::io::Error> for ParseError
        values.push(cursor.read_u32::<LittleEndian>()?);
    }
    Ok(values)
}

/// Reads a single NIF string, typically prefixed with a u32 length.

/// Reads 'count' length-prefixed NIF strings.
pub fn read_nif_string_arr(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<String>> {
    const MAX_ARRAY_COUNT: usize = 1024 * 16;
    if count > MAX_ARRAY_COUNT {
        return Err(ParseError::InvalidData(format!(
            "String array count {} exceeds maximum allowed limit {}",
            count, MAX_ARRAY_COUNT
        )));
    }
    let mut strings = Vec::with_capacity(count);
    for _ in 0..count {
        // Use '?' on the function which now returns Result<_, ParseError>
        let len = cursor.read_u32::<LittleEndian>()?;
        strings.push(read_nif_string(cursor, len)?);
    }
    Ok(strings)
}

/// Reads a NIF link/reference (index). Returns Option<usize>.
/// Treats u32::MAX (0xFFFFFFFF) as None, otherwise converts to usize.
fn read_nif_link(cursor: &mut Cursor<&[u8]>) -> Result<Option<usize>> {
    let link_u32 = cursor.read_u32::<LittleEndian>()?; // '?' handles IO error
    if link_u32 == u32::MAX {
        Ok(None)
    } else {
        Ok(Some(link_u32 as usize))
    }
}

/// Reads 'count' NIF links/references into a Vec<Option<usize>>.
/// Corrected version based on your snippet and error type.
fn read_nif_link_arr_options(
    cursor: &mut Cursor<&[u8]>,
    count: usize,
) -> Result<Vec<Option<usize>>> {
    // Return custom ParseError
    const MAX_ARRAY_COUNT: usize = 1024 * 64; // Example limit
    if count > MAX_ARRAY_COUNT {
        // Return the correct error type
        return Err(ParseError::InvalidData(format!(
            "Link array count {} exceeds maximum allowed limit {}",
            count, MAX_ARRAY_COUNT
        )));
    }
    let mut links = Vec::with_capacity(count);
    for _ in 0..count {
        links.push(read_nif_link(cursor)?);
    }
    Ok(links)
}
