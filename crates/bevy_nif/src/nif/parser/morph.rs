use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::nif::{
    error::ParseError,
    parser::helpers::{read_key_float, read_link, read_vector3},
    types::{KeyType, MorphTarget, NiGeomMorpherController, NiMorphData},
};

pub fn parse_nimorphdata_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiMorphData, ParseError> {
    let num_morph_targets = cursor.read_u32::<LittleEndian>()?;
    let num_vertices = cursor.read_u32::<LittleEndian>()?;
    let relative_targets = cursor.read_u8()? != 0;

    // Sanity checks
    if num_morph_targets > 1000 {
        return Err(ParseError::InvalidData(
            "Too many morph targets".to_string(),
        ));
    }
    if num_vertices > 100_000 {
        return Err(ParseError::InvalidData(
            "Too many vertices in morph".to_string(),
        ));
    }

    // Read the morph target data
    let mut morph_targets_struct_vec = Vec::with_capacity(num_morph_targets as usize);
    for _target_idx in 0..num_morph_targets {
        // *** Read MorphTarget Key Info FIRST ***
        let num_keys = cursor.read_u32::<LittleEndian>()?;
        let interpolation_raw = cursor.read_u32::<LittleEndian>()?;
        let interpolation = KeyType::from(interpolation_raw);

        let mut keys_vec = Vec::with_capacity(num_keys as usize);
        if num_keys > 10000 {
            return Err(ParseError::InvalidData("Too many morph keys".to_string()));
        }
        for _key_idx in 0..num_keys {
            // Pass interpolation type to handle potential extra data reads/skips
            keys_vec.push(read_key_float(cursor, interpolation)?);
        }

        // *** THEN Read Vertex Vectors for this target ***
        let mut vertex_vec = Vec::with_capacity(num_vertices as usize);
        for _ in 0..num_vertices {
            vertex_vec.push(read_vector3(cursor)?); // Reads 12 bytes
        }

        // Create and push the MorphTarget struct containing keys AND vertices
        morph_targets_struct_vec.push(MorphTarget {
            num_keys,
            interpolation,
            keys: keys_vec,
            vertices: vertex_vec,
        });
    } // End loop over morph targets

    Ok(NiMorphData {
        num_morph_targets,
        num_vertices,
        relative_targets,
        morph_targets: morph_targets_struct_vec, // Assign Vec<MorphTarget>
    })
}
pub fn parse_nigeommorphercontroller_fields(
    cursor: &mut Cursor<&[u8]>,
    _block_index: u32,
) -> Result<NiGeomMorpherController, ParseError> {
    // 1. Read NiTimeController fields
    let next_controller = read_link(cursor)?;
    let flags = cursor.read_u16::<LittleEndian>()?;
    let frequency = cursor.read_f32::<LittleEndian>()?;
    let phase = cursor.read_f32::<LittleEndian>()?;
    let start_time = cursor.read_f32::<LittleEndian>()?;
    let stop_time = cursor.read_f32::<LittleEndian>()?;
    let target = read_link(cursor)?;

    // 2. Read Specific field
    let morph_data = read_link(cursor)?;

    // *** ADDED: Read Always Update flag (byte) based on NifSkope ***
    let always_update_u8 = cursor.read_u8()?; // Reads 1 byte
    let always_update = always_update_u8 != 0; // Convert byte to bool
    // *** END ADDED ***

    Ok(NiGeomMorpherController {
        next_controller,
        flags,
        frequency,
        phase,
        start_time,
        stop_time,
        target,
        morph_data,
        always_update, // Store the value read
    })
}
