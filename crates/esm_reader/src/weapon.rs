// src/weapon.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)] // Ensures memory layout matches file if read as a block
pub struct WeaponData {
    pub weight: f32,      // 4 bytes
    pub value: i32,       // 4 bytes
    pub weapon_type: i16, // 2 bytes (enum: 0=ShortBlade, 1=LongBlade, etc.)
    pub health: u16,      // 2 bytes (condition, OpenMW uses uint16_t)
    pub speed: f32,       // 4 bytes
    pub reach: f32,       // 4 bytes
    pub enchant_pts: u16, // 2 bytes (enchantment capacity, OpenMW uses uint16_t)
    pub chop_min: u8,     // 1 byte
    pub chop_max: u8,     // 1 byte
    pub slash_min: u8,    // 1 byte
    pub slash_max: u8,    // 1 byte
    pub thrust_min: u8,   // 1 byte
    pub thrust_max: u8,   // 1 byte
    pub flags: i32,       // 4 bytes (Bitflags, OpenMW uses int32_t)
} // Total: 4+4+2+2+4+4+2+1+1+1+1+1+1+4 = 32 bytes.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Weapon {
    pub id: RefId,                     // From NAME subrecord (Record ID)
    pub model: Option<String>,         // From MODL subrecord
    pub full_name: Option<String>,     // From FNAM subrecord (Displayable name - OPTIONAL)
    pub data: Option<WeaponData>,      // From WPDT subrecord (Mandatory unless deleted)
    pub icon: Option<String>,          // From ITEX subrecord
    pub enchantment_id: Option<RefId>, // From ENAM subrecord (optional)
    pub script_id: Option<RefId>,      // From SCRI subrecord (optional)
}

impl Weapon {
    pub fn new() -> Self {
        Default::default()
    }

    /// Loads a WEAP record's data from the EsmReader.
    /// Based on OpenMW's Weapon::load method.
    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut weap = Weapon::new();
        let mut id_found = false; // Corresponds to OpenMW's hasName (for SREC_NAME)
        let mut wpdt_found = false; // Corresponds to OpenMW's hasData (for WPDT)
        let mut is_deleted = false; // For DELE subrecord

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    // SREC_NAME in OpenMW
                    weap.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    weap.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    weap.full_name = Some(reader.get_h_string()?);
                    // Not marking fnam_found as it's not essential for load success in OpenMW
                }
                b"WPDT" => {
                    reader.get_sub_header()?;
                    let expected_wpdt_size = std::mem::size_of::<WeaponData>() as u32;
                    if reader.get_sub_size() != expected_wpdt_size {
                        // OpenMW's esm.getSubComposite(mData) would implicitly check this.
                        // If the size is wrong, our get_t<WeaponData> will likely fail or read garbage.
                        return Err(reader
                            .report_sub_size_mismatch(expected_wpdt_size, reader.get_sub_size()));
                    }

                    // Attempt to read WeaponData as a block.
                    // This relies on #[repr(C, packed)] and matching field types/order.
                    weap.data = Some(reader.get_t::<WeaponData>()?);
                    wpdt_found = true;
                }
                b"ITEX" => {
                    weap.icon = Some(reader.get_h_string()?);
                }
                b"ENAM" => {
                    weap.enchantment_id = Some(reader.get_ref_id()?);
                }
                b"SCRI" => {
                    weap.script_id = Some(reader.get_ref_id()?);
                }
                b"DELE" => {
                    reader.skip_h_sub()?;
                    is_deleted = true;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || weap.id.empty() {
            return Err(reader.fail("WEAP record missing mandatory NAME subrecord for its ID"));
        }
        if !wpdt_found && !is_deleted {
            // WPDT is mandatory unless record is deleted
            return Err(reader.fail(&format!(
                "WEAP record '{}' missing WPDT subrecord",
                weap.id.to_debug_string()
            )));
        }
        // FNAM (full_name) is NOT checked as mandatory for load success, aligning with OpenMW.

        Ok(weap)
    }
}

