use std::collections::HashMap;

use crate::{EsmError, EsmReader, Load, RefId, StringRefId, esm_reader::ConsumptionType};

use super::{
    flags::{CellFlags, ObjectFlags},
    reference::Reference,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    pub flags: ObjectFlags,
    pub name: Option<StringRefId>,
    pub data: Option<CellData>,
    pub region: Option<String>,
    pub map_color: Option<[u8; 4]>,
    pub water_height: Option<f32>,
    pub atmosphere_data: Option<AtmosphereData>,
    pub references: HashMap<(u32, u32), Reference>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellData {
    pub flags: CellFlags,
    pub grid: (i32, i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AtmosphereData {
    pub ambient_color: [u8; 4],
    pub sunlight_color: [u8; 4],
    pub fog_color: [u8; 4],
    pub fog_density: f32,
}

impl Load for CellData {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let flags = reader.get_u32_le(ConsumptionType::SubrecordPayload)?;
        let grid_x = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
        let grid_y = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;

        Ok(CellData {
            flags: CellFlags::from_bits_truncate(flags),
            grid: (grid_x, grid_y),
        })
    }
}

impl Load for AtmosphereData {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let ambient_color = reader.load()?;
        let sunlight_color = reader.load()?;
        let fog_color = reader.load()?;
        let fog_density = reader.load()?;
        Ok(Self {
            ambient_color,
            sunlight_color,
            fog_color,
            fog_density,
        })
    }
}

// Helper function to unpack packed indices
// This is a free function as in the reference, since it doesn't need 'self'
const fn unpack(packed_indices: u32) -> (u32, u32) {
    let mast_index = packed_indices >> 24;
    let refr_index = packed_indices & 0xFFFFFF;
    (mast_index, refr_index)
}

impl Load for Cell {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut cell = Cell::default();
        let mut num_temp_refs: i32 = 0;
        let mut moved_refs = Vec::<(u32, u32, (i32, i32))>::new();

        // The top-level record flags are part of the record header, not a subrecord.
        let rec_flags = reader.get_record_flags();
        if rec_flags.contains(ObjectFlags::DELETED) {
            cell.flags.insert(ObjectFlags::DELETED);
        }

        while reader.has_more_subs() {
            let current_sub_name = reader.get_sub_name()?;
            reader.get_sub_header()?; // This sets the remaining_subrec_data in the context
            let payload_size = reader.context.remaining_subrec_data;

            match current_sub_name.as_bytes() {
                b"NAME" => {
                    let s = reader.get_string_with_size(payload_size as usize)?;
                    cell.name = Some(StringRefId::new(&s));
                }
                b"DATA" => {
                    const EXPECTED_SIZE: usize = 12;
                    if payload_size as usize != EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "DATA",
                        ));
                    }
                    cell.data = Some(CellData::load(reader)?);
                }
                b"RGNN" => {
                    cell.region = Some(reader.get_string_with_size(payload_size as usize)?);
                }
                b"NAM5" => {
                    const EXPECTED_SIZE: usize = 4;
                    if payload_size as usize != EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "NAM5",
                        ));
                    }
                    let mut color_data = [0u8; 4];
                    reader.get_exact(&mut color_data, ConsumptionType::SubrecordPayload)?;
                    cell.map_color = Some(color_data);
                }
                b"WHGT" => {
                    const EXPECTED_SIZE: usize = 4;
                    if payload_size as usize != EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "WHGT",
                        ));
                    }
                    let water_height: i32 = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
                    // The reference code converts i32 to f32.
                    cell.water_height = Some(water_height as f32);
                }
                b"AMBI" => {
                    const EXPECTED_SIZE: usize = 16;
                    if (payload_size as usize) < EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "AMBI",
                        ));
                    }
                    cell.atmosphere_data = Some(AtmosphereData::load(reader)?);
                    // Handle potential padding by skipping remaining bytes
                    if payload_size as usize > EXPECTED_SIZE {
                        reader.skip_raw_bytes(
                            (payload_size as usize - EXPECTED_SIZE) as i64,
                            ConsumptionType::SubrecordPayload,
                        )?;
                    }
                }
                b"NAM0" => {
                    const EXPECTED_SIZE: usize = 4;
                    if payload_size as usize != EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "NAM0",
                        ));
                    }
                    num_temp_refs = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
                }
                b"MVRF" => {
                    const EXPECTED_SIZE: usize = 4;
                    if payload_size as usize != EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "MVRF",
                        ));
                    }
                    let packed_indices = reader.get_u32_le(ConsumptionType::SubrecordPayload)?;

                    // "MVRF" is always followed by "CNDT"
                    let cndt_name = reader.get_sub_name()?;
                    if cndt_name.as_bytes() != b"CNDT" {
                        return Err(reader.fail("Expected CNDT subrecord after MVRF"));
                    }
                    const CNDT_SIZE: usize = 8;
                    reader.get_sub_header()?;
                    let cndt_size = reader.context.remaining_subrec_data;
                    if cndt_size as usize != CNDT_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            cndt_size as usize,
                            CNDT_SIZE,
                            "CNDT",
                        ));
                    } // Correctly load the moved cell coordinates as two i32s
                    let moved_cell_x = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
                    let moved_cell_y = reader.get_i32_le(ConsumptionType::SubrecordPayload)?;
                    moved_refs.push((
                        unpack(packed_indices).0,
                        unpack(packed_indices).1,
                        (moved_cell_x, moved_cell_y),
                    ));
                }
                b"FRMR" => {
                    const EXPECTED_SIZE: usize = 4;
                    if payload_size as usize != EXPECTED_SIZE {
                        return Err(reader.report_sub_size_mismatch_custom(
                            payload_size as usize,
                            EXPECTED_SIZE,
                            "FRMR",
                        ));
                    }
                    let packed_indices = reader.get_u32_le(ConsumptionType::SubrecordPayload)?;
                    let indices = unpack(packed_indices);

                    // FRMR is followed by a nested REFR record's data
                    dbg!(reader.context.file_pos);
                    let mut reference: Reference = reader.load().unwrap();
                    reference.mast_index = indices.0;
                    reference.refr_index = indices.1;
                    reference.temporary = num_temp_refs > 0;
                    num_temp_refs -= 1;

                    // Check if this FRMR should override a moved reference
                    if let Some(moved_ref) = moved_refs.last_mut() {
                        if moved_ref.0 == 0 {
                            // Check the master index
                            moved_ref.0 = indices.0;
                            moved_ref.1 = indices.1;
                        }
                    }

                    cell.references.insert(indices, reference);
                }
                b"INTV" => {
                    let water_height: i32 = reader.load()?;
                    #[allow(clippy::cast_precision_loss)]
                    {
                        // esp version 1.3 uses float rather than int
                        cell.water_height = Some(water_height as f32);
                    }
                }
                b"DELE" => {
                    reader.skip_sub_data()?;
                    cell.flags.insert(ObjectFlags::DELETED);
                }
                _ => {
                    // Unknown or unexpected subrecord, skip it.
                    reader.skip_sub_data()?;
                }
            }
        }

        // Assign moved references after the main loop to ensure all are loaded
        for (mast_idx, refr_idx, moved_cell) in moved_refs {
            if let Some(reference) = cell.references.get_mut(&(mast_idx, refr_idx)) {
                // This will now correctly work because moved_cell is a (i32, i32) tuple
                reference.moved_cell = Some(moved_cell);
            } else {
                return Err(reader.fail(&format!(
                    "Unable to resolve moved reference ({}, {}) for cell {:?}",
                    mast_idx,
                    refr_idx,
                    cell.name.as_ref().map(|s| &s.value)
                )));
            }
        }

        Ok(cell)
    }
}
