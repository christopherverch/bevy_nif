// src/sound_gen.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)] // Specify the underlying integer type for the enum
pub enum SoundGenType {
    LeftFoot = 0,
    RightFoot = 1,
    SwimLeft = 2,
    SwimRight = 3,
    Moan = 4,
    Roar = 5,
    Scream = 6,
    Land = 7,
    Unknown(i32), // Keep this to store unrecognized values
}

impl Default for SoundGenType {
    fn default() -> Self {
        SoundGenType::Unknown(-1)
    } // Default to an invalid type
}

impl From<i32> for SoundGenType {
    fn from(val: i32) -> Self {
        match val {
            0 => SoundGenType::LeftFoot,
            1 => SoundGenType::RightFoot,
            2 => SoundGenType::SwimLeft,
            3 => SoundGenType::SwimRight,
            4 => SoundGenType::Moan,
            5 => SoundGenType::Roar,
            6 => SoundGenType::Scream,
            7 => SoundGenType::Land,
            _ => SoundGenType::Unknown(val),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SoundGen {
    pub id: RefId, // From NAME subrecord (Record ID for this sound gen definition)
    pub sound_type: SoundGenType, // From DATA subrecord (e.g., LeftFoot, Moan) - This is an i32 in the file
    pub creature_id: Option<RefId>, // From CNAM subrecord (Creature this applies to, optional)
    pub sound_to_play_id: Option<RefId>, // From SNAM subrecord (Sound record ID to play, optional)
}

impl SoundGen {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut sndg = SoundGen::new();
        let mut id_found = false;
        let mut data_found = false; // For the sound type from DATA subrecord

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    sndg.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"DATA" => {
                    // SoundGenerator::Type (int32_t)
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        // Expecting int32_t (4 bytes)
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    let type_val = reader.get_t::<i32>()?;
                    sndg.sound_type = SoundGenType::from(type_val);
                    data_found = true;
                }
                b"CNAM" => {
                    // Creature RefId this applies to (optional)
                    sndg.creature_id = Some(reader.get_ref_id()?);
                }
                b"SNAM" => {
                    // Sound RefId to play (optional)
                    sndg.sound_to_play_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || sndg.id.empty() {
            return Err(reader.fail("SNDG record missing mandatory NAME subrecord for its ID"));
        }
        if !data_found {
            // DATA subrecord (sound type) is mandatory
            return Err(reader.fail(&format!(
                "SNDG record '{}' missing DATA (sound type) subrecord",
                sndg.id.to_debug_string()
            )));
        }
        // CNAM and SNAM are optional according to OpenMW (mCreature and mSound are RefId() by default in blank())

        Ok(sndg)
    }
}

