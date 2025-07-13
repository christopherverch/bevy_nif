// src/ammunition.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct AmmoData {
    // AMDT subrecord
    pub ammo_type: i16, // 0 for Arrow, 1 for Bolt (OpenMW: uint16_t mType)
    pub speed: f32,
    pub flags: u32, // 0x0001 = Ignores Normal Weapon Resistance
    pub damage: u16, // Damage points (OpenMW: uint16_t mDamage)
                    // Note: UESP mentions damage min/max ubytes, but OpenMW AMDTstruct has u16 mDamage.
                    // File format seems to be short for damage.
} // Total: 2 (type) + 4 (speed) + 4 (flags) + 2 (damage) = 12 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ammunition {
    pub id: RefId,                     // From NAME subrecord
    pub model: Option<String>,         // From MODL subrecord
    pub full_name: Option<String>,     // From FNAM subrecord
    pub data: Option<AmmoData>,        // From AMDT subrecord
    pub icon: Option<String>,          // From ITEX subrecord
    pub enchantment_id: Option<RefId>, // From ENAM subrecord (optional)
    pub script_id: Option<RefId>,      // From SCRI subrecord (optional)
}

impl Ammunition {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut ammo = Ammunition::new();
        let mut id_found = false;
        let mut amdt_found = false;
        let mut fnam_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    ammo.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    ammo.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    ammo.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"AMDT" => {
                    // Ammunition Data
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<AmmoData>() as u32; // 12 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    ammo.data = Some(reader.get_t::<AmmoData>()?);
                    amdt_found = true;
                }
                b"ITEX" => {
                    ammo.icon = Some(reader.get_h_string()?);
                }
                b"ENAM" => {
                    ammo.enchantment_id = Some(reader.get_ref_id()?);
                }
                b"SCRI" => {
                    ammo.script_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || ammo.id.empty() {
            return Err(reader.fail("AMMO record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "AMMO record '{}' missing FNAM subrecord",
                ammo.id.to_debug_string()
            )));
        }
        if !amdt_found {
            return Err(reader.fail(&format!(
                "AMMO record '{}' missing AMDT subrecord",
                ammo.id.to_debug_string()
            )));
        }

        Ok(ammo)
    }
}
