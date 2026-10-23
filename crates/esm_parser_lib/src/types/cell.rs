use std::collections::LinkedList;

use crate::{
    ESM3ExteriorCellRefId, EsmContext, EsmError, EsmReader, FormId, Load, RefId, StringRefId,
    esm_reader::ConsumptionType, esm_record::EsmRecord, name4, string_interning::get_or_intern,
    types::enums::RecInt,
};

use super::{
    cell_ref::{CellRef, skip_load_cell_ref},
    flags::{CellFlags, ObjectFlags},
};
pub type MovedCellRefTracker = LinkedList<MovedCellRef>;
pub type CellRefTracker = LinkedList<(CellRef, bool)>;
const FALSE_WIDE_REF_NUM: bool = false;
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    pub flags: ObjectFlags,
    pub name: String,
    pub data: CellData,
    pub region: Option<RefId>,
    pub id: RefId,
    pub map_color: Option<[u8; 4]>,
    pub water_height: Option<f32>,
    pub atmosphere_data: Option<AtmosphereData>,
    pub num_temp_refs: i32,
    pub leased_refs: LinkedList<(CellRef, bool)>,
    pub moved_refs: LinkedList<MovedCellRef>,
    pub context_list: Vec<EsmContext>,
}
impl EsmRecord for Cell {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Cell;
}
pub type RefNum = FormId;
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MovedCellRef {
    pub ref_num: RefNum,
    // Coordinates of target exterior cell
    pub target: [i32; 2],
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
        let flags = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let grid_x = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let grid_y = reader.read_typed(ConsumptionType::SubrecordPayload)?;

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
pub const fn unpack(packed_indices: u32) -> (u32, u32) {
    let mast_index = packed_indices >> 24;
    let refr_index = packed_indices & 0xFFFFFF;
    (mast_index, refr_index)
}
/// Identifies references belonging to a parent file and adapts the ID accordingly
/// by translating the temporary 8-bit plugin index into the permanent content_file index.
pub fn adjust_ref_num(reader: &mut EsmReader, ref_num: &mut FormId) {
    // Extract the highest 8 bits (the temporary plugin index).
    let local = (ref_num.index & 0xff000000) >> 24;

    let parent_file_count = reader.get_parent_file_indices().len() as u32;

    // Check if the 8-bit index is valid and refers to a parent file.
    if local != 0 && local <= parent_file_count {
        // Case 1: The reference was added by a parent file.

        // Strip the temporary 8-bit plugin index from the u32 ID.
        ref_num.index &= 0x00ffffff;

        // Set the permanent content file index using the parent file's ID.
        // The index is 1-based, so subtract 1 for 0-based array access.
        let parent_file_index = reader.get_parent_file_indices()[(local - 1) as usize];

        // Cast the usize index to the persistent i16 type. This is safe
        // because the number of master files is guaranteed to be less than 256.
        ref_num.content_file = Some(parent_file_index as i16);
    } else {
        // Case 2: The reference is an addition by the present plugin (or an invalid ID).

        // Set the permanent content file index to that of the currently loading plugin.
        ref_num.content_file = Some(reader.get_index());
    }
}
impl PartialEq<RefNum> for MovedCellRef {
    fn eq(&self, other: &RefNum) -> bool {
        &self.ref_num == other
    }
}
impl PartialEq<RefNum> for CellRef {
    fn eq(&self, other: &RefNum) -> bool {
        &self.ref_num == other
    }
}
#[derive(Debug, PartialEq)]
pub enum GetNextRefMode {
    LoadAll,
    LoadOnlyMoved,
    LoadOnlyNotMoved,
}

impl Cell {
    pub fn load(
        reader: &mut EsmReader,
        is_deleted: &mut bool,
        save_context: bool,
    ) -> Result<Cell, EsmError> {
        let mut cell = Cell::default();
        cell.load_name_and_data(reader, is_deleted)?;
        cell.load_cell(reader, save_context)?;
        return Ok(cell);
    }

    fn load_name_and_data(
        &mut self,
        reader: &mut EsmReader,
        is_deleted: &mut bool,
    ) -> Result<(), EsmError> {
        *is_deleted = false;
        self.blank();
        let mut has_data = false;
        let mut is_loaded = false;

        while !is_loaded && reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    self.name = reader.read_header_decode_string()?;
                }
                b"DATA" => {
                    self.data = reader.read_typed_header_load()?;
                    has_data = true;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => {
                    reader.cache_sub_name();
                    is_loaded = true;
                }
            }
        }

        if !has_data {
            return Err(reader.fail("Missing DATA subrecord"));
        }
        self.update_id();

        Ok(())
    }

    fn load_cell(&mut self, reader: &mut EsmReader, save_context: bool) -> Result<(), EsmError> {
        let mut is_loaded = false;
        let overriding = !self.name.is_empty();
        self.atmosphere_data = None;
        self.water_height = None;

        while !is_loaded && reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"INTV" => {
                    let water_level_i32: i32 = reader.read_typed_header()?;
                    self.water_height = Some(water_level_i32 as f32);
                }
                b"WHGT" => {
                    let water_level: f32 = reader.read_typed_header()?;
                    if !water_level.is_finite() {
                        eprint!(
                            "Warning: Encountered invalid water level in cell {} defined in {:?}",
                            self.name, reader.context.filename
                        );

                        if !overriding {
                            self.water_height = Some(f32::MAX);
                        }
                    } else {
                        self.water_height = Some(water_level);
                    }
                }
                b"AMBI" => {
                    reader.read_sub_header()?;
                    self.atmosphere_data = Some(reader.load()?);
                }
                b"RGNN" => self.region = Some(reader.read_refid()?),
                b"NAM5" => self.map_color = Some(reader.read_typed_header()?),
                b"NAM0" => {
                    self.num_temp_refs = reader.read_typed_header()?;
                }
                _ => {
                    reader.cache_sub_name();
                    is_loaded = true;
                }
            }
        }
        if save_context {
            self.context_list.push(reader.get_cloned_context()?);
            reader.skip_record()?;
        }
        Ok(())
    }
    fn update_id(&mut self) -> RefId {
        // Call the generation function with the resolved &str
        self.id = Self::generate_id_for_cell(
            self.is_exterior(),
            &self.name,
            self.data.grid.0,
            self.data.grid.1,
        );
        self.id.clone()
    }
    pub fn generate_id_for_cell(exterior: bool, cell_name: &str, x: i32, y: i32) -> RefId {
        if !exterior {
            // Intern the string
            let str = get_or_intern(cell_name);
            RefId::SizedString(StringRefId(str))
        } else {
            RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(x, y))
        }
    }

    pub fn post_load(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        // Save position of the cell references and move on
        self.context_list.push(reader.get_cloned_context()?);
        reader.skip_record()?;
        Ok(())
    }
    pub fn restore(&self, reader: &mut EsmReader, context_index: usize) -> Result<(), EsmError> {
        let context = self
            .context_list
            .get(context_index)
            .ok_or(reader.fail(&format!(
                "Cell tried to restore invalid context! {}",
                context_index
            )))?;

        reader.restore_context(context)
    }
    pub fn get_description(&self) -> String {
        if self.data.flags.contains(CellFlags::IS_INTERIOR) {
            return self.name.clone();
        }

        let cell_grid = format!("({}, {})", self.data.grid.0, self.data.grid.1);

        if !self.name.is_empty() {
            // C++: return nameString + ' ' + cellGrid;
            return format!("{} {}", self.name, cell_grid);
        }

        // Get the region name string from the Option<RefId>
        let region_ref_name = self
            .region
            .as_ref()
            // If the RefId exists, resolve its string value using the interner
            .map(|r| r.resolve_string())
            // If region is None, default to an empty string.
            .unwrap_or_default();

        // Check if the resolved region name is empty.
        let region_output = if region_ref_name.is_empty() {
            // TODO::
            // sDefaultCellname GMST should be used, but isn't available since
            // it's part of the actual usage of the esm reader
            "Wilderness".to_string()
        } else {
            // Use the string resolved from the RefId
            region_ref_name
        };

        format!("{} {}", region_output, cell_grid)
    }
    pub fn get_next_ref(
        reader: &mut EsmReader,
        cell_ref: &mut CellRef,
        is_deleted: &mut bool,
    ) -> Result<bool, EsmError> {
        *is_deleted = false;
        // TODO: Try and document reference numbering, I don't think this has been done anywhere else.
        if !reader.has_more_subs() {
            return Ok(false);
        }
        while reader.is_next_sub(name4!("MVRF"))? {
            // This is still read so the reader will be in the right spot,
            // but it's only returned in the "full" version of this function
            let mut _moved_cell_ref = MovedCellRef::default();
            _moved_cell_ref.ref_num.index = reader.read_typed_header()?;
            if let Some(val) = reader.read_typed_named_header_opt(name4!("CNDT"))? {
                _moved_cell_ref.target = val;
            }
            let mut skipped_cell_ref = CellRef::default();
            if reader.peek_next_sub(name4!("FRMR"))? {
                return Ok(false);
            }
            let mut skipped_deleted = false;
            skipped_cell_ref.load(reader, &mut skipped_deleted, FALSE_WIDE_REF_NUM)?;
        }
        if reader.peek_next_sub(name4!("FRMR"))? {
            cell_ref.load(reader, is_deleted, FALSE_WIDE_REF_NUM)?;
            // TODO: should count the number of temp refs and validate the number
            adjust_ref_num(reader, &mut cell_ref.ref_num);
            return Ok(true);
            // Identify references belonging to a parent file and adapt the ID accordingly.
        }
        Ok(false)
        // MVRF loop: Skip all moved CellRefs.
    }
    pub fn get_next_ref_full(
        reader: &mut EsmReader,
        // Output: The loaded reference data (only filled if not skipped)
        cell_ref: &mut CellRef,
        // Output: Flag indicating if the reference was deleted
        is_deleted: &mut bool,
        // Output: The loaded MovedCellRef data (only filled if MVRF was present)
        moved_cell_ref: &mut MovedCellRef,
        // Output: Flag indicating if an MVRF subrecord was encountered before the FRMR
        moved: &mut bool,
        // Input: The filtering mode
        mode: GetNextRefMode,
    ) -> Result<bool, EsmError> {
        *is_deleted = false;
        *moved = false;

        if !reader.has_more_subs() {
            return Ok(false);
        }

        if reader.is_next_sub(name4!("MVRF"))? {
            *moved = true;

            moved_cell_ref.ref_num.index = reader.read_typed_header()?;
            if let Some(val) = reader.read_typed_named_header_opt(name4!("CNDT"))? {
                moved_cell_ref.target = val;
            }
        }

        // The FRMR subrecord must immediately follow the MVRF (or be the next subrecord).
        if !reader.peek_next_sub(name4!("FRMR"))? {
            return Ok(false);
        }

        let should_skip = (!*moved && mode == GetNextRefMode::LoadOnlyMoved)
            || (*moved && mode == GetNextRefMode::LoadOnlyNotMoved);

        if should_skip {
            skip_load_cell_ref(reader, FALSE_WIDE_REF_NUM)?;

            return Ok(true);
        }

        // Execution reaches here only if the record is not skipped

        cell_ref.load(reader, is_deleted, FALSE_WIDE_REF_NUM)?;

        adjust_ref_num(reader, &mut cell_ref.ref_num);

        Ok(true)
    }
    fn blank(&mut self) {
        self.name.clear();
        self.region = None;
        self.water_height = Some(0.0);
        self.map_color = None;
        self.num_temp_refs = 0;
        self.flags = ObjectFlags::empty();
        self.data.grid.0 = 0;
        self.data.grid.1 = 0;
        self.atmosphere_data = Some(AtmosphereData {
            ambient_color: [0, 0, 0, 0],
            sunlight_color: [0, 0, 0, 0],
            fog_color: [0, 0, 0, 0],
            fog_density: 0.0,
        });
    }
    pub const fn is_interior(&self) -> bool {
        self.data.flags.contains(CellFlags::IS_INTERIOR)
    }

    pub const fn is_exterior(&self) -> bool {
        !self.is_interior()
    }
}
