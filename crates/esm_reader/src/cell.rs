// src/cell.rs

use crate::common_types::{MAX_STRING_REF_ID_FORMAT_VERSION_CONST, Name};
use crate::esm_reader::{EsmError, EsmReader}; // EsmReader is EsmReader<'a>
use crate::refid_types::{FormId, RefId};

// Matches ESM::Cell::DATAstruct (12 bytes)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct CellData {
    pub flags: u32, // Bitflags: 0x01=Interior, 0x02=HasWater, 0x04=IllegalToSleep, 0x80=BehaveLikeExterior
    pub grid_x: i32, // Exterior cell X coordinate (if not Interior flag)
    pub grid_y: i32, // Exterior cell Y coordinate (if not Interior flag)
}

// Matches ESM::Cell::AMBIstruct (16 bytes)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct AmbientLightData {
    pub ambient_r: u8,
    pub ambient_g: u8,
    pub ambient_b: u8,
    pub _padding_ambient1: u8,
    pub sunlight_r: u8,
    pub sunlight_g: u8,
    pub sunlight_b: u8,
    pub _padding_sunlight1: u8,
    pub fog_r: u8,
    pub fog_g: u8,
    pub fog_b: u8,
    pub _padding_fog1: u8,
    pub fog_density: f32,
}

// Matches ESM::Position (6 floats = 24 bytes)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct Position {
    pub pos: [f32; 3], // x, y, z
    pub rot: [f32; 3], // x, y, z in radians
}

/// Represents the adjusted RefNum for a CellRef (based on ESM::FormId after adjustment).
/// It holds the object's final index and its resolved content file's index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AdjustedRefNum {
    pub index: u32,              // The object index (FRMR lower 24 bits)
    pub content_file_idx: usize, // Index of the content file this ref belongs to
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellRef {
    pub ref_num_frmr_raw: u32,
    pub ref_num: FormId,
    pub base_object_id: Option<RefId>,
    pub scale: f32,
    pub owner_id: Option<RefId>,
    pub global_variable: Option<String>,
    pub soul_id: Option<RefId>,
    pub faction_id: Option<RefId>,
    pub faction_rank_required: i32,
    pub charge_int: i32,
    pub enchantment_charge: f32,
    pub stack_count: i32,
    pub is_door_teleport: bool,
    pub door_destination_pos: Option<Position>,
    pub door_destination_cell_name: Option<String>,
    pub lock_level: i32,
    pub is_locked: bool,
    pub key_id: Option<RefId>,
    pub trap_id: Option<RefId>,
    pub reference_blocked_status: i8,
    pub position_data: Option<Position>,
    pub is_deleted: bool,
    pub was_preceded_by_mvrf: bool,
    pub mvrf_original_frmr_index: Option<u32>,
    pub mvrf_target_coords: Option<(i32, i32)>,
}

impl Default for CellRef {
    fn default() -> Self {
        Self {
            ref_num_frmr_raw: 0,
            ref_num: FormId::default(),
            base_object_id: None,
            scale: 1.0,
            owner_id: None,
            global_variable: None,
            soul_id: None,
            faction_id: None,
            faction_rank_required: -2,
            charge_int: -1,
            enchantment_charge: -1.0,
            stack_count: 1,
            is_door_teleport: false,
            door_destination_pos: None,
            door_destination_cell_name: None,
            lock_level: 0,
            is_locked: false,
            key_id: None,
            trap_id: None,
            reference_blocked_status: -1,
            position_data: None,
            is_deleted: false,
            was_preceded_by_mvrf: false,
            mvrf_original_frmr_index: None,
            mvrf_target_coords: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    pub id: RefId,
    pub editor_id_name: Option<String>,
    pub data: Option<CellData>,
    pub region_id: Option<RefId>,
    pub water_level: Option<f32>,
    pub has_water_height_sub: bool,
    pub ambient_light: Option<AmbientLightData>,
    pub has_ambi_sub: bool,
    pub map_color: Option<i32>,
    pub ref_num_counter_original: Option<i32>,
    pub references: Vec<CellRef>,
}

impl Cell {
    pub fn new() -> Self {
        let mut cell = Self::default();
        cell.has_ambi_sub = false;
        cell.has_water_height_sub = false;
        cell
    }

    fn adjust_cell_ref_form_id(form_id: &mut FormId, reader: &EsmReader) {
        let raw_frmr_index = form_id.m_index;
        let file_marker_from_frmr = (raw_frmr_index & 0xFF000000) >> 24;

        if file_marker_from_frmr > 0
            && (file_marker_from_frmr as usize) <= reader.get_parent_file_indices().len()
        {
            let parent_idx_in_list = (file_marker_from_frmr - 1) as usize;
            form_id.m_content_file_index =
                reader.get_parent_file_indices()[parent_idx_in_list] as u32;
            form_id.m_index = raw_frmr_index & 0x00FFFFFF;
        } else {
            if file_marker_from_frmr == 0 {
                form_id.m_index = raw_frmr_index & 0x00FFFFFF;
            }
        }
    }

    fn adjust_ref_num_from_raw_frmr(frmr_index_raw: u32, reader: &EsmReader) -> AdjustedRefNum {
        let mut adj_ref_num = AdjustedRefNum {
            index: frmr_index_raw & 0x00FFFFFF,
            content_file_idx: reader.get_index(),
        };
        let original_content_file_marker = (frmr_index_raw & 0xFF000000) >> 24;

        if original_content_file_marker > 0 {
            let parent_idx_in_list = (original_content_file_marker - 1) as usize;
            if parent_idx_in_list < reader.get_parent_file_indices().len() {
                adj_ref_num.content_file_idx = reader.get_parent_file_indices()[parent_idx_in_list];
            }
        }
        adj_ref_num
    }

    fn parse_cell_ref_data(reader: &mut EsmReader, cell_ref: &mut CellRef) -> Result<(), EsmError> {
        'ref_data_loop: loop {
            // Label definition
            if !reader.has_more_subs() {
                break 'ref_data_loop; // CORRECTED: Break from labeled loop
            }
            if reader.peek_next_sub(Name::new("FRMR"))?
                || reader.peek_next_sub(Name::new("MVRF"))?
            {
                break 'ref_data_loop; // CORRECTED: Break from labeled loop
            }

            let ref_sub_name = reader.get_sub_name()?;
            match &ref_sub_name.m_data {
                b"DELE" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        let _ = reader.get_t::<i32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                    cell_ref.is_deleted = true;
                }
                b"DODT" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == std::mem::size_of::<Position>() as u32 {
                        cell_ref.door_destination_pos = Some(reader.get_t::<Position>()?);
                        cell_ref.is_door_teleport = true;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"DNAM" => {
                    cell_ref.door_destination_cell_name = Some(reader.get_h_string()?);
                }
                b"FLTV" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell_ref.lock_level = reader.get_t::<i32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"KNAM" => {
                    cell_ref.key_id = Some(reader.get_ref_id()?);
                }
                b"TNAM" => {
                    cell_ref.trap_id = Some(reader.get_ref_id()?);
                }
                b"UNAM" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 1 {
                        cell_ref.reference_blocked_status = reader.get_t::<i8>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"ANAM" => {
                    cell_ref.owner_id = Some(reader.get_ref_id()?);
                }
                b"BNAM" => {
                    cell_ref.global_variable = Some(reader.get_h_string()?);
                }
                b"XSOL" => {
                    cell_ref.soul_id = Some(reader.get_ref_id()?);
                }
                b"CNAM" => {
                    cell_ref.faction_id = Some(reader.get_ref_id()?);
                }
                b"INDX" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell_ref.faction_rank_required = reader.get_t::<i32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"XCHG" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell_ref.enchantment_charge = reader.get_t::<f32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"INTV" => {
                    // This INTV is for CellRef.charge_int
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell_ref.charge_int = reader.get_t::<i32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"NAM9" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell_ref.stack_count = reader.get_t::<i32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"XSCL" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell_ref.scale = reader.get_t::<f32>()?;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"DATA" => {
                    // This DATA is for CellRef position
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == std::mem::size_of::<Position>() as u32 {
                        cell_ref.position_data = Some(reader.get_t::<Position>()?);
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"NAM0" => {
                    // This NAM0 is for CellRef, distinct from Cell's NAM0
                    reader.skip_h_sub()?;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }
        let original_lock_level_for_check = cell_ref.lock_level;
        const ZERO_LOCK_SENTINEL: i32 = std::i32::MAX;

        if cell_ref.lock_level == ZERO_LOCK_SENTINEL {
            cell_ref.lock_level = 0;
        }

        if reader.get_header().m_format_version.0 <= MAX_STRING_REF_ID_FORMAT_VERSION_CONST.0 {
            cell_ref.is_locked = (cell_ref.key_id.is_some()
                && cell_ref.key_id.as_ref().map_or(false, |k| !k.empty()))
                || original_lock_level_for_check > 0;
        } else {
            cell_ref.is_locked = original_lock_level_for_check > 0;
        }
        Ok(())
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut cell = Cell::new();
        let mut cell_data_loaded = false;

        'cell_global_loop: loop {
            // Label definition
            if !reader.has_more_subs() {
                break 'cell_global_loop; // CORRECTED: Break from labeled loop
            }
            if reader.peek_next_sub(Name::new("FRMR"))?
                || reader.peek_next_sub(Name::new("MVRF"))?
            {
                break 'cell_global_loop; // CORRECTED: Break from labeled loop
            }

            let sub_name = reader.get_sub_name()?;
            match &sub_name.m_data {
                b"NAME" => {
                    let name_str = reader.get_h_string()?;
                    if !name_str.is_empty() {
                        cell.editor_id_name = Some(name_str);
                    }
                }
                b"DATA" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == std::mem::size_of::<CellData>() as u32 {
                        cell.data = Some(reader.get_t::<CellData>()?);
                        cell_data_loaded = true;
                    } else {
                        return Err(reader.report_sub_size_mismatch(
                            std::mem::size_of::<CellData>() as u32,
                            reader.get_sub_size(),
                        ));
                    }
                }
                b"RGNN" => {
                    cell.region_id = Some(reader.get_ref_id()?);
                }
                b"WHGT" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell.water_level = Some(reader.get_t::<f32>()?);
                        cell.has_water_height_sub = true;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"INTV" => {
                    // This INTV is for Cell's water_level
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 && !cell.has_water_height_sub {
                        cell.water_level = Some(reader.get_t::<i32>()? as f32);
                        cell.has_water_height_sub = true;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"AMBI" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == std::mem::size_of::<AmbientLightData>() as u32 {
                        cell.ambient_light = Some(reader.get_t::<AmbientLightData>()?);
                        cell.has_ambi_sub = true;
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"NAM5" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell.map_color = Some(reader.get_t::<i32>()?);
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"NAM0" => {
                    // This NAM0 is for the Cell itself
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        cell.ref_num_counter_original = Some(reader.get_t::<i32>()?);
                    } else {
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"FRMR" | b"MVRF" => {
                    reader.cache_sub_name();
                    break 'cell_global_loop; // CORRECTED: Break from labeled loop
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !cell_data_loaded {
            return Err(reader.fail("CELL record missing mandatory DATA subrecord"));
        }

        let (is_interior, grid_x, grid_y) = cell.data.map_or((true, 0, 0), |d| {
            (((d.flags & 0x01) != 0), d.grid_x, d.grid_y)
        });
        if !is_interior {
            cell.id = RefId::esm3_exterior_cell(grid_x, grid_y);
        } else {
            cell.id = RefId::string_ref_id(cell.editor_id_name.as_deref().unwrap_or(""));
        }

        // Phase 2: Load cell references
        while reader.has_more_subs() {
            let mut current_ref = CellRef::default();

            if reader.is_next_sub(Name::new("MVRF"))? {
                current_ref.was_preceded_by_mvrf = true;
                reader.get_sub_header()?;
                let mvrf_sub_size = reader.get_sub_size();
                if mvrf_sub_size >= 4 {
                    let original_frmr_idx_raw = reader.get_t::<i32>()?;
                    current_ref.mvrf_original_frmr_index = Some(original_frmr_idx_raw as u32);
                    if mvrf_sub_size == 12 {
                        let target_x = reader.get_t::<i32>()?;
                        let target_y = reader.get_t::<i32>()?;
                        current_ref.mvrf_target_coords = Some((target_x, target_y));
                    } else if mvrf_sub_size > 4 {
                        reader.skip(mvrf_sub_size - 4)?;
                    }
                } else {
                    reader.skip(mvrf_sub_size)?;
                }

                if !reader.is_next_sub(Name::new("FRMR"))? {
                    return Err(reader.fail(&format!(
                        "CELL record '{}': Missing FRMR after MVRF",
                        cell.id.to_debug_string()
                    )));
                }
            } else if !reader.is_next_sub(Name::new("FRMR"))? {
                if reader.has_more_subs() {
                    // This condition indicates an unexpected subrecord where FRMR was expected,
                    // or the end of subrecords for this cell if no more references.
                    // To be safe and avoid infinite loops if a subrecord isn't FRMR or MVRF,
                    // we should probably try to skip it if it's not what we expect,
                    // or simply break if we are certain no more valid references should follow.
                    // The original code had 'break;' here.
                }
                break;
            }

            reader.get_sub_header()?;
            if reader.get_sub_size() != 4 {
                return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
            }
            let frmr_index_raw = reader.get_t::<i32>()?;
            current_ref.ref_num_frmr_raw = frmr_index_raw as u32;

            let mut temp_form_id_for_adjust = FormId {
                m_index: frmr_index_raw as u32,
                m_content_file_index: reader.get_index() as u32,
            };
            Cell::adjust_cell_ref_form_id(&mut temp_form_id_for_adjust, reader);
            current_ref.ref_num = temp_form_id_for_adjust;

            if reader.has_more_subs() && reader.peek_next_sub(Name::new("NAM0"))? {
                reader.skip_h_sub()?;
            }

            if !reader.has_more_subs() || !reader.is_next_sub(Name::new("NAME"))? {
                return Err(reader.fail(&format!(
                    "CELL record '{}', Ref FRMR {}: Missing NAME subrecord for CellRef",
                    cell.id.to_debug_string(),
                    frmr_index_raw
                )));
            }
            current_ref.base_object_id = Some(reader.get_ref_id()?);

            Self::parse_cell_ref_data(reader, &mut current_ref)?;
            cell.references.push(current_ref);
        }
        Ok(cell)
    }
}
