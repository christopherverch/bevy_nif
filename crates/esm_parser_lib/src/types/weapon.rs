use crate::Load;
use crate::esm_reader::{ConsumptionType, EsmError, EsmReader};
use crate::esm_record::EsmRecord;
use crate::refid_types::RefId;
use bitflags::bitflags;

use super::flags::ObjectFlags;
// --- Enums, Flags, and Structs from loadweap.hpp ---

bitflags! {
    /// Corresponds to the enum Weapon::Flags, now as a typesafe bitflag.
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
    pub flags: WeaponFlags,
}

/// Reads the contents of a WPDT subrecord into a WeaponData struct.
impl Load for WeaponData {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        Ok(Self {
            weight: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            value: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            weapon_type: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            health: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            speed: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            reach: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            enchantment_points: reader.read_typed(ConsumptionType::SubrecordPayload)?,
            chop_damage: [
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
            ],
            slash_damage: [
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
            ],
            thrust_damage: [
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
            ],
            // Read the raw i32 and convert it into the WeaponFlags type.
            // from_bits_truncate is used to safely ignore any unknown flags from the file.
            flags: WeaponFlags::from_bits_truncate(
                reader.read_typed(ConsumptionType::SubrecordPayload)?,
            ),
        })
    }
}

/// Represents a WEAP (Weapon) record from an ESM file.
#[derive(Debug, Clone, Default)]
pub struct Weapon {
    /// Flags relating to the record itself
    pub flags: ObjectFlags,
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
impl EsmRecord for Weapon {
    fn id(&self) -> &RefId {
        &self.id
    }
}

impl Weapon {
    /// Return a string descriptor for this record type.
    pub fn get_record_type() -> &'static str {
        "Weapon"
    }

    /// Sets the record to a default state, similar to the C++ blank() method.
    pub fn blank(&mut self) {
        *self = Self::default();
    }
}
impl Load for Weapon {
    /// Loads weapon data from an EsmReader.
    /// The reader is expected to be at the start of the subrecords for a WEAP record.
    fn load(esm_reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut weapon = Weapon::default();
        weapon.flags = esm_reader.get_record_flags();
        let mut has_name = false;
        let mut has_data = false;

        while esm_reader.has_more_subs() {
            let sub_name = esm_reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    weapon.id = esm_reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    weapon.model = esm_reader.read_normalized_path_string()?;
                }
                b"FNAM" => {
                    weapon.name = esm_reader.read_header_decode_str()?.to_string();
                }
                b"WPDT" => {
                    esm_reader.read_sub_header()?;
                    weapon.data = esm_reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    weapon.script = esm_reader.read_refid()?;
                }
                b"ITEX" => {
                    weapon.icon = esm_reader.read_normalized_path_string()?;
                }
                b"ENAM" => {
                    weapon.enchantment = esm_reader.read_refid()?;
                }
                b"DELE" => {
                    esm_reader.skip_sub_data()?;
                    weapon.flags.insert(ObjectFlags::DELETED);
                }
                _ => {
                    return Err(EsmError::UnknownRecName(u32::from_le_bytes(sub_name.data)));
                }
            }
        }

        if !has_name {
            return Err(esm_reader.fail("Missing NAME subrecord in WEAP"));
        }
        if !has_data && !weapon.flags.contains(ObjectFlags::DELETED) {
            return Err(esm_reader.fail("Missing WPDT subrecord in WEAP"));
        }

        Ok(weapon)
    }
}
