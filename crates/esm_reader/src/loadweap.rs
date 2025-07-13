// src/loadweap.rs

use crate::esm_reader::{ConsumptionType, EsmError, EsmReader};
use crate::refid_types::RefId;
use bitflags::bitflags;
// --- Enums, Flags, and Structs from loadweap.hpp ---

bitflags! {
    /// Corresponds to the enum Weapon::Flags, now as a typesafe bitflag.
    /// The struct is given standard derives for convenience.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct WeaponFlags: i32 {
        const MAGICAL = 0x01;
        const SILVER = 0x02;
    }
}

/// Corresponds to the enum Weapon::Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum WeaponType {
    ShortBladeOneHand = 0,
    LongBladeOneHand = 1,
    LongBladeTwoHand = 2,
    BluntOneHand = 3,
    BluntTwoClose = 4,
    BluntTwoWide = 5,
    SpearTwoWide = 6,
    AxeOneHand = 7,
    AxeTwoHand = 8,
    MarksmanBow = 9,
    MarksmanCrossbow = 10,
    MarksmanThrown = 11,
    Arrow = 12,
    Bolt = 13,
}

/// Corresponds to the WPDTstruct.
/// This struct must be exactly 32 bytes long.
#[derive(Debug, Clone, Default)]
pub struct WeaponData {
    pub weight: f32,
    pub value: i32,
    pub weapon_type: i16, // Can be mapped to WeaponType enum
    pub health: u16,
    pub speed: f32,
    pub reach: f32,
    pub enchantment_points: u16,
    pub chop_damage: [u8; 2],   // min, max
    pub slash_damage: [u8; 2],  // min, max
    pub thrust_damage: [u8; 2], // min, max
    pub flags: WeaponFlags,     // <-- Type changed from i32 to WeaponFlags
}

impl WeaponData {
    /// Reads the contents of a WPDT subrecord into a WeaponData struct.
    pub fn from_reader(reader: &mut EsmReader) -> Result<Self, EsmError> {
        Ok(Self {
            weight: reader.get_f32_le(ConsumptionType::SubrecordPayload)?,
            value: reader.get_i32_le(ConsumptionType::SubrecordPayload)?,
            weapon_type: reader.get_i16_le(ConsumptionType::SubrecordPayload)?,
            health: reader.get_u16_le(ConsumptionType::SubrecordPayload)?,
            speed: reader.get_f32_le(ConsumptionType::SubrecordPayload)?,
            reach: reader.get_f32_le(ConsumptionType::SubrecordPayload)?,
            enchantment_points: reader.get_u16_le(ConsumptionType::SubrecordPayload)?,
            chop_damage: [
                reader.get_u8_le(ConsumptionType::SubrecordPayload)?,
                reader.get_u8_le(ConsumptionType::SubrecordPayload)?,
            ],
            slash_damage: [
                reader.get_u8_le(ConsumptionType::SubrecordPayload)?,
                reader.get_u8_le(ConsumptionType::SubrecordPayload)?,
            ],
            thrust_damage: [
                reader.get_u8_le(ConsumptionType::SubrecordPayload)?,
                reader.get_u8_le(ConsumptionType::SubrecordPayload)?,
            ],
            // Read the raw i32 and convert it into the WeaponFlags type.
            // from_bits_truncate is used to safely ignore any unknown flags from the file.
            flags: WeaponFlags::from_bits_truncate(
                reader.get_i32_le(ConsumptionType::SubrecordPayload)?,
            ),
        })
    }
}

/// Represents a WEAP (Weapon) record from an ESM file.
#[derive(Debug, Clone, Default)]
pub struct Weapon {
    /// Flags relating to the record itself
    pub record_flags: u32,
    /// From NAME subrecord: The editor ID of this weapon.
    pub id: RefId,
    /// From MODL subrecord: The model file for this weapon.
    pub model: String,
    /// From FNAM subrecord: The friendly name of the weapon.
    pub name: String,
    /// From WPDT subrecord: Core weapon statistics.
    pub data: WeaponData,
    /// From SCRI subrecord: The script attached to this weapon.
    pub script: RefId,
    /// From ITEX subrecord: The inventory icon for this weapon.
    pub icon: String,
    /// From ENAM subrecord: The enchantment on this weapon.
    pub enchantment: RefId,
}

impl Weapon {
    /// Return a string descriptor for this record type.
    pub fn get_record_type() -> &'static str {
        "Weapon"
    }

    /// Loads weapon data from an EsmReader.
    /// The reader is expected to be at the start of the subrecords for a WEAP record.
    pub fn load(esm_reader: &mut EsmReader) -> Result<(Self, bool), EsmError> {
        let record_flags = esm_reader.get_record_flags(); // <-- GET FLAGS
        let mut weapon = Weapon::default();
        weapon.record_flags = record_flags;
        let mut is_deleted = false;
        let mut has_name = false;
        let mut has_data = false;

        while esm_reader.has_more_subs() {
            let sub_name = esm_reader.get_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    weapon.id = esm_reader.get_ref_id()?;
                    has_name = true;
                }
                b"MODL" => {
                    weapon.model = esm_reader.get_normalized_path_string()?;
                }
                b"FNAM" => {
                    weapon.name = esm_reader.get_h_string()?;
                }
                b"WPDT" => {
                    esm_reader.get_sub_header()?;
                    let expected_size = 32; // WPDT is fixed at 32 bytes
                    if esm_reader.get_sub_size() != expected_size {
                        return Err(esm_reader
                            .report_sub_size_mismatch(expected_size, esm_reader.get_sub_size()));
                    }
                    weapon.data = WeaponData::from_reader(esm_reader)?;
                    has_data = true;
                }
                b"SCRI" => {
                    weapon.script = esm_reader.get_ref_id()?;
                }
                b"ITEX" => {
                    weapon.icon = esm_reader.get_normalized_path_string()?;
                }
                b"ENAM" => {
                    weapon.enchantment = esm_reader.get_ref_id()?;
                }
                b"DELE" => {
                    esm_reader.skip_sub_data()?;
                    is_deleted = true;
                }
                _ => {
                    let msg = format!("Unknown WEAP subrecord: {}", sub_name.to_string_lossy());
                    return Err(esm_reader.fail(&msg));
                }
            }
        }

        if !has_name {
            return Err(esm_reader.fail("Missing NAME subrecord in WEAP"));
        }
        if !has_data && !is_deleted {
            return Err(esm_reader.fail("Missing WPDT subrecord in WEAP"));
        }

        Ok((weapon, is_deleted))
    }

    /// Sets the record to a default state, similar to the C++ blank() method.
    pub fn blank(&mut self) {
        *self = Self::default();
    }
}
