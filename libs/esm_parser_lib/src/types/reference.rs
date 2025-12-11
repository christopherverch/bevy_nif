use super::aipackage::TravelDestination;
use crate::{
    Load, Name32,
    esm_reader::{EsmError, EsmReader},
};
use glam::Vec3;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reference {
    pub mast_index: u32,
    pub refr_index: u32,
    pub id: String,
    pub temporary: bool,
    pub translation: Vec3,
    pub rotation: Vec3,
    pub scale: Option<f32>,
    pub moved_cell: Option<(i32, i32)>,
    pub owner: Option<String>,
    pub owner_global: Option<String>,
    pub owner_faction: Option<String>,
    pub owner_faction_rank: Option<u32>,
    pub charge_left: Option<u32>,
    pub health_left: Option<i32>,
    pub object_count: Option<u32>,
    pub destination: Option<TravelDestination>,
    pub lock_level: Option<i32>,
    pub key: Option<String>,
    pub trap: Option<String>,
    pub soul: Option<String>,
    pub blocked: Option<u8>,
    pub deleted: Option<bool>,
}
impl Load for Reference {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut this: Self = Default::default();
        dbg!(reader.get_sub_size());

        while reader.has_more_subs()
            && !reader.peek_next_sub(Name32::new("FRMR"))?
            && !reader.peek_next_sub(Name32::new("MVRF"))?
            && !reader.peek_next_sub(Name32::new("DELE"))?
            && !reader.peek_next_sub(Name32::new("NAM0"))?
        {
            let sub_name = reader.get_or_read_sub_name()?;
            reader.read_sub_header()?;

            dbg!(sub_name.to_string_lossy());
            dbg!(reader.get_sub_size());
            match sub_name.as_bytes() {
                b"NAME" => {
                    this.id = reader.load()?;
                }
                b"UNAM" => {
                    this.blocked = Some(reader.load()?);
                }
                b"XSCL" => {
                    this.scale = Some(reader.load()?);
                }
                b"ANAM" => {
                    this.owner = Some(reader.load()?);
                }
                b"BNAM" => {
                    this.owner_global = Some(reader.load()?);
                }
                b"CNAM" => {
                    this.owner_faction = Some(reader.load()?);
                }
                b"INDX" => {
                    this.owner_faction_rank = Some(reader.load()?);
                }
                b"XSOL" => {
                    this.soul = Some(reader.load()?);
                }
                b"XCHG" => {
                    this.charge_left = Some(reader.load()?);
                }
                b"INTV" => {
                    this.health_left = Some(reader.load()?);
                }
                b"NAM9" => {
                    this.object_count = Some(reader.load()?);
                }
                b"DODT" => {
                    this.destination = Some(reader.load()?);
                }
                b"DNAM" => {
                    // This subrecord is a direct child of the Reference record,
                    // so it must be handled here.
                    if let Some(destination) = &mut this.destination {
                        destination.cell = reader.load()?;
                    } else {
                        // This case handles a malformed file where DNAM appears
                        // without a preceding DODT.
                        return Err(
                            reader.fail("DNAM subrecord found without a preceding DODT subrecord")
                        );
                    }
                }
                b"FLTV" => {
                    this.lock_level = Some(reader.load()?);
                }
                b"KNAM" => {
                    this.key = Some(reader.load()?);
                }
                b"TNAM" => {
                    this.trap = Some(reader.load()?);
                }
                b"DATA" => {
                    this.translation = reader.load()?;
                    this.rotation = reader.load()?;
                }
                b"DELE" => {
                    reader.skip_sub_data()?;
                    this.deleted = Some(true);
                }
                _ => {
                    reader.fail(&format!(
                        "Unexpected Tag: REFR::{}",
                        sub_name.to_string_lossy()
                    ));
                }
            }
        }

        // The TESCS can produce non-finite transforms which break JSON conversion.
        this.make_transforms_finite();

        Ok(this)
    }
}

impl Reference {
    pub const fn deleted(&self) -> bool {
        matches!(self.deleted, Some(true))
    }

    pub const fn persistent(&self) -> bool {
        // Moved references are always persistent.
        if self.moved_cell.is_some() {
            return true;
        }

        // Doors with destinations are always persistent.
        if self.destination.is_some() {
            return true;
        }

        // Note:
        //  NPCs and creatures should also always be persistent.
        //  We do not have the information to enforce that here.

        // For everything else trust the flag.
        !self.temporary
    }

    pub(crate) fn make_transforms_finite(&mut self) {
        if !self.translation.x.is_finite() {
            self.translation.x = 0.0;
        }
        if !self.translation.y.is_finite() {
            self.translation.y = 0.0;
        }
        if !self.translation.z.is_finite() {
            self.translation.z = 0.0;
        }
        if !self.rotation.x.is_finite() {
            self.rotation.x = 0.0;
        }
        if !self.rotation.y.is_finite() {
            self.rotation.y = 0.0;
        }
        if !self.rotation.z.is_finite() {
            self.rotation.z = 0.0;
        }
        if matches!(self.scale, Some(value) if !value.is_finite()) {
            self.scale = None;
        }
    }
}
