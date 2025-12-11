use glam::{EulerRot, Quat, Vec3};

use crate::{
    EsmError, EsmReader, Load, RefId, format_version_defs::DEFAULT_FORMAT_VERSION, name32,
};

use super::cell::RefNum;

#[derive(Debug, Clone, PartialEq)]
pub struct CellRef {
    pub ref_num: RefNum,
    pub ref_id: RefId,
    pub scale: f32,
    pub owner: RefId,
    pub global_variable: String,
    pub soul: RefId,
    pub faction: RefId,
    pub faction_rank: Option<i32>,
    pub charge: Charge,
    pub charge_int_remainder: f32,
    pub enchantment_charge: Option<f32>,
    pub count: i32,
    pub teleport: bool,
    pub door_dest: Position,
    pub dest_cell: String,
    pub lock_level: i32,
    pub is_locked: bool,
    pub key: RefId,
    pub trap: RefId,
    pub reference_blocked: Option<i8>,
    pub pos: Position,
}
impl Default for CellRef {
    fn default() -> Self {
        Self {
            ref_num: RefNum::default(),
            ref_id: RefId::default(),
            scale: 1.0,
            owner: RefId::default(),
            global_variable: String::new(),
            soul: RefId::default(),
            faction: RefId::default(),
            faction_rank: None,
            charge: Charge::default(),
            charge_int_remainder: 0.0,
            enchantment_charge: None,
            count: 1,
            teleport: false,
            door_dest: Position::default(),
            dest_cell: String::new(),
            lock_level: 0,
            is_locked: false,
            key: RefId::default(),
            trap: RefId::default(),
            reference_blocked: None,
            pos: Position::default(),
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Position {
    translation: Vec3,
    rotation: Quat,
}
impl Load for Position {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError>
    where
        Self: Sized,
    {
        // 6 floats: float rot[3] followed by float pos[3].

        let rot_x = reader.load::<f32>()?;
        let rot_y = reader.load::<f32>()?;
        let rot_z = reader.load::<f32>()?;

        let pos_x = reader.load::<f32>()?;
        let pos_y = reader.load::<f32>()?;
        let pos_z = reader.load::<f32>()?;

        // The game stores rotation as Euler angles. These must be converted to a Quat.
        // Assuming a ZYX (Yaw-Pitch-Roll) convention for conversion
        let rotation = Quat::from_euler(
            EulerRot::ZYX,
            rot_z.to_radians(), // Z-rotation (Yaw)
            rot_y.to_radians(), // Y-rotation (Pitch)
            rot_x.to_radians(), // X-rotation (Roll)
        );

        let translation = Vec3::new(pos_x, pos_y, pos_z);

        Ok(Position {
            translation,
            rotation,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Charge {
    Int(i32),
    Float(f32),
}
// Signals "full charge / charge not yet consumed" and is used in saving/game logic.
impl Default for Charge {
    fn default() -> Self {
        Charge::Int(-1)
    }
}

impl CellRef {
    pub fn load(
        &mut self,
        reader: &mut EsmReader,
        is_deleted: &mut bool,
        wide_ref_num: bool,
    ) -> Result<(), EsmError> {
        self.load_id(reader, wide_ref_num)?;
        self.load_data(reader, is_deleted)?;
        Ok(())
    }

    pub fn load_id(&mut self, reader: &mut EsmReader, wide_ref_num: bool) -> Result<(), EsmError> {
        if reader.is_next_sub(name32!("NAM0"))? {
            reader.skip_sub_header()?;
        }
        self.blank();
        self.ref_num = reader.read_form_id(wide_ref_num, name32!("FRMR"))?;
        self.ref_id = reader.read_named_sub_ref_id(name32!("NAME"))?;

        Ok(())
    }
    fn skip_load_id(reader: &mut EsmReader, wide_ref_num: bool) -> Result<(), EsmError> {
        if reader.is_next_sub(name32!("NAM0"))? {
            reader.skip_sub_header()?; // Consumes the NAM0 header and its (zero) data
        }
        let _ = reader.read_form_id(wide_ref_num, name32!("FRMR"))?;
        reader.skip_named_header_ref_id(name32!("NAME"))?;

        Ok(())
    }
    pub fn blank(&mut self) {
        *self = Self::default();
    }
    pub fn load_data(
        &mut self,
        reader: &mut EsmReader,
        is_deleted: &mut bool,
    ) -> Result<(), EsmError> {
        *is_deleted = false;

        const ZERO_LOCK: i32 = i32::MAX;
        const SCALE_MIN: f32 = 0.5;
        const SCALE_MAX: f32 = 2.0;

        let mut is_loaded = false;

        while !is_loaded && reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"UNAM" => {
                    let raw_val: i8 = reader.read_typed_header()?;

                    if raw_val == -1 {
                        self.reference_blocked = None;
                    } else {
                        self.reference_blocked = Some(raw_val);
                    }
                }

                b"XSCL" => {
                    self.scale = reader.read_typed_header()?;
                    self.scale = self.scale.clamp(SCALE_MIN, SCALE_MAX);
                }

                b"ANAM" => self.owner = reader.read_ref_id()?,
                b"BNAM" => self.global_variable = reader.read_header_decode_str()?.to_string(),
                b"XSOL" => self.soul = reader.read_ref_id()?,
                b"CNAM" => self.faction = reader.read_ref_id()?,
                b"INDX" => {
                    let raw_val: i32 = reader.read_typed_header()?;

                    if raw_val == -2 {
                        self.faction_rank = None;
                    } else {
                        self.faction_rank = Some(raw_val);
                    }
                }
                b"XCHG" => {
                    let raw_val: f32 = reader.read_typed_header()?;

                    if raw_val == -1.0 {
                        self.enchantment_charge = None;
                    } else {
                        self.enchantment_charge = Some(raw_val);
                    }
                }
                b"INTV" => {
                    let charge_int: i32 = reader.read_typed_header()?;
                    self.charge = Charge::Int(charge_int);
                }
                b"NAM9" => self.count = reader.read_typed_header()?,
                b"DODT" => {
                    self.door_dest = reader.read_sub_header_and_load()?;
                    self.teleport = true;
                }
                b"DNAM" => self.dest_cell = reader.read_header_decode_str()?.to_string(),
                b"FLTV" => self.lock_level = reader.read_typed_header()?,
                b"KNAM" => self.key = reader.read_ref_id()?,
                b"TNAM" => self.trap = reader.read_ref_id()?,
                b"DATA" => self.pos = reader.read_sub_header_and_load()?,

                // NAM0 / DELE: Skip headers (no data read)
                b"NAM0" | b"DELE" => {
                    // C++: esm.skipHSub();
                    reader.skip_sub_header()?;
                    if sub_name.as_bytes() == b"DELE" {
                        *is_deleted = true;
                    }
                }

                // Default: Stop and cache the subrecord name
                _ => {
                    reader.cache_sub_name();
                    is_loaded = true;
                }
            }
        }

        let is_esm3_format = reader.header.format_version == DEFAULT_FORMAT_VERSION;

        if is_esm3_format {
            self.is_locked = !self.key.is_empty() || self.lock_level > 0;
        } else {
            self.is_locked = self.lock_level > 0;
        }

        if self.lock_level == ZERO_LOCK {
            self.lock_level = 0;
        }

        Ok(())
    }
    pub fn skip_load_data(reader: &mut EsmReader) -> Result<(), EsmError> {
        let mut is_loaded = false;

        while !is_loaded && reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                // String Subrecords requiring special skip handling
                b"BNAM" | b"DNAM" => reader.skip_header_string()?,
                // All other subrecords (fixed-size, RefIds, composites, and standard skips)
                b"UNAM" | b"XSCL" | b"ANAM" | b"XSOL" | b"CNAM" | b"INDX" | b"XCHG" | b"INTV"
                | b"NAM9" | b"DODT" | b"FLTV" | b"KNAM" | b"TNAM" | b"DATA" | b"NAM0" | b"DELE" => {
                    reader.skip_sub_header()?
                }

                _ => {
                    reader.cache_sub_name();
                    is_loaded = true;
                }
            }
        }
        Ok(())
    }
}
pub fn skip_load_cell_ref(reader: &mut EsmReader, wide_ref_num: bool) -> Result<(), EsmError> {
    CellRef::skip_load_id(reader, wide_ref_num)?;
    CellRef::skip_load_data(reader)?;

    Ok(())
}
