use crate::common_types::Name32;
use crate::{EsmError, EsmReader, types::flags::ServiceFlags};
use esm_parser_macros::LoadSave;
use glam::Vec3;
use smart_default::SmartDefault;

#[derive(LoadSave, Clone, Debug, Default, PartialEq)]
pub struct TravelDestination {
    pub translation: Vec3,
    pub rotation: Vec3,
    pub cell: String,
}
#[derive(Debug, SmartDefault, Clone, PartialEq)]
pub enum AIPackage {
    #[default]
    Wander(AIWander),
    Travel(AITravel),
    Follow(AITarget, Option<String>), // Some follow 0 have no cell
    Escort(AITarget, Option<String>),
    Activate(AIActivate),
}
#[derive(LoadSave, Clone, Debug, Default, PartialEq)]
pub struct AIWander {
    distance: i16,
    duration: i16,
    game_hour: u8,
    idle: [u8; 8],
    should_repeat: u8,
}
#[derive(LoadSave, Clone, Debug, Default, PartialEq)]
pub struct AITravel {
    pub location: Vec3,
    pub should_repeat: u8,
    pub _padding: [u8; 3], // Align to next subrecord
}
#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct AITarget {
    pub location: Vec3,
    pub duration: i16,
    pub target_id: Name32,
    pub should_repeat: u8,
    pub _padding: u8,
}
#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct AIActivate {
    pub target_id: Name32,
    pub should_repeat: u8,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, LoadSave)]
pub struct AIData {
    pub greeting_distance: u16,
    pub fight: u8,
    pub flee: u8,
    pub alarm: u8,
    // Note: The C++ 'decompose' shows 3 bytes of padding here to hit 12 bytes
    pub _padding: [u8; 3],
    pub services: ServiceFlags,
}
#[derive(Debug, Default, Clone, PartialEq)]
pub struct AIPackageList(pub Vec<AIPackage>);
impl AIPackageList {
    pub fn add(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        let sub_name = reader.get_sub_name();

        match sub_name.as_bytes() {
            b"CNDT" => {
                if self.0.is_empty() {
                    return Err(reader.fail("AIPackage with an AI_CNDT applying to no cell"));
                }
                let cell_name = reader.read_header_decode_string()?;
                if let Some(last_package) = self.0.last_mut() {
                    match last_package {
                        AIPackage::Follow(_, cell) | AIPackage::Escort(_, cell) => {
                            *cell = Some(cell_name);
                        }
                        _ => {
                            // Technically OpenMW allows this but it's logically weird.
                            // You could choose to ignore it or error out.
                        }
                    }
                }
            }
            b"AI_W" => {
                reader.read_sub_header()?;
                self.0.push(AIPackage::Wander(reader.load()?));
            }
            b"AI_T" => {
                reader.read_sub_header()?;
                self.0.push(AIPackage::Travel(reader.load()?));
            }
            b"AI_F" => {
                reader.read_sub_header()?;
                self.0.push(AIPackage::Follow(reader.load()?, None));
            }
            b"AI_E" => {
                reader.read_sub_header()?;
                self.0.push(AIPackage::Escort(reader.load()?, None));
            }
            b"AI_A" => {
                reader.read_sub_header()?;
                self.0.push(AIPackage::Activate(reader.load()?));
            }
            _ => return Err(reader.fail("Not a valid AI subrecord")),
        }
        Ok(())
    }
}
