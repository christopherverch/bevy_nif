// src/read_utils.rs
use super::error::{ParseError, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

/// Reads a string prefixed with its length (u32 LE).
pub fn read_sized_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let len = cursor.read_u32::<LittleEndian>()?;
    // Basic sanity check for unusually large strings
    if len > 50000 {
        // Adjust limit as needed
        return Err(ParseError::InvalidData(format!(
            "Attempting to read overly long string ({} bytes)",
            len
        )));
    }
    if len == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; len as usize];
    cursor.read_exact(&mut buf)?;
    // Use from_utf8_lossy for robustness against potential non-utf8 data
    Ok(String::from_utf8_lossy(&buf).to_string())
}

// --- Future potential helpers ---
/*
use super::structs::{Vector3, Matrix33};

pub fn read_vector3(cursor: &mut Cursor<&[u8]>) -> Result<Vector3> {
    Ok(Vector3 {
        x: cursor.read_f32::<LittleEndian>()?,
        y: cursor.read_f32::<LittleEndian>()?,
        z: cursor.read_f32::<LittleEndian>()?,
    })
}

pub fn read_matrix33(cursor: &mut Cursor<&[u8]>) -> Result<Matrix33> {
    let mut data = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            data[i][j] = cursor.read_f32::<LittleEndian>()?;
        }
    }
    Ok(Matrix33 { data })
}
*/

