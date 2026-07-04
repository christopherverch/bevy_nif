use crate::esm_reader::{ConsumptionType, EsmError, EsmReader};
use crate::esm_record::EsmRecord;
use crate::refid_types::RefId;
use crate::types::enums::WeaponType;
use crate::{Load, name4};
use bitflags::bitflags;

use super::flags::ObjectFlags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct WeaponFlags: i32 {
        const MAGICAL = 0x01;
        const SILVER = 0x02;
    }
}

/// Corresponds to the WPDTstruct.
/// This struct must be exactly 32 bytes long.
#[derive(Debug, Clone, Default)]
pub struct WeaponData {
    pub weight: f32,
    pub value: i32,
    pub weapon_type: WeaponType,
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
            weapon_type: {
                let raw_val: i16 = reader.read_typed(ConsumptionType::SubrecordPayload)?;
                WeaponType::try_from(raw_val).unwrap_or(WeaponType::None)
            },
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
    /// From FNAM subrecord: The in-game name of the weapon.
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
    const RECORD_ID: u32 = name4!("WEAP").to_u32();
}

impl Weapon {
    /// Return a string descriptor for this record type.
    pub fn get_record_type() -> &'static str {
        "Weapon"
    }
}
impl Load for Weapon {
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
