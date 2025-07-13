// src/race.rs

 // Using title case as requested
use crate::esm_reader::{EsmError, EsmReader};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct SkillBonus {
    pub skill_id: i32, // Enum for skills (0-26)
    pub bonus: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct RaceData {
    pub skill_bonuses: [SkillBonus; 7],
    pub attribute_male: [i32; 8], // STR, INT, WIL, AGI, SPD, END, PER, LUC
    pub attribute_female: [i32; 8], // STR, INT, WIL, AGI, SPD, END, PER, LUC
    pub height_male: f32,
    pub height_female: f32,
    pub weight_male: f32,
    pub weight_female: f32,
    pub flags: u32, // 0x1: Playable, 0x2: Beast Race
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Race {
    // The record ID (e.g., "dark elf") is read by EsmReader::get_rec_name()
    pub full_name: String,           // From FNAM subrecord
    pub data: RaceData,              // From RADT subrecord
    pub description: String,         // From DESC subrecord
    pub special_powers: Vec<String>, // From NPCS subrecords
}

impl Race {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut race_info = Race::new();
        let mut fnam_found = false;
        let mut radt_found = false;
        // DESC and NPCS can be optional or multiple

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;
            // println!("  [RACE DEBUG] Subrecord: {}", sub_name.to_string_lossy());

            match &sub_name.m_data {
                b"FNAM" => {
                    race_info.full_name = reader.get_h_string()?;
                    fnam_found = true;
                    // println!("    [RACE DEBUG] Full Name (FNAM): {}", race_info.full_name);
                }
                b"RADT" => {
                    reader.get_sub_header()?;
                    let expected_radt_size = std::mem::size_of::<RaceData>() as u32;
                    if reader.get_sub_size() != expected_radt_size {
                        return Err(reader
                            .report_sub_size_mismatch(expected_radt_size, reader.get_sub_size()));
                    }

                    for i in 0..7 {
                        race_info.data.skill_bonuses[i].skill_id = reader.get_t::<i32>()?;
                        race_info.data.skill_bonuses[i].bonus = reader.get_t::<i32>()?;
                    }
                    for i in 0..8 {
                        race_info.data.attribute_male[i] = reader.get_t::<i32>()?;
                    }
                    for i in 0..8 {
                        race_info.data.attribute_female[i] = reader.get_t::<i32>()?;
                    }
                    race_info.data.height_male = reader.get_t::<f32>()?;
                    race_info.data.height_female = reader.get_t::<f32>()?;
                    race_info.data.weight_male = reader.get_t::<f32>()?;
                    race_info.data.weight_female = reader.get_t::<f32>()?;
                    race_info.data.flags = reader.get_t::<u32>()?;

                    radt_found = true;
                    // println!("    [RACE DEBUG] Data (RADT): Parsed successfully");
                }
                b"DESC" => {
                    race_info.description = reader.get_h_string()?;
                    // let desc_preview = race_info.description.chars().take(30).collect::<String>();
                    // println!("    [RACE DEBUG] Description (DESC): '{}...'", desc_preview);
                }
                b"NPCS" => {
                    // Can occur multiple times
                    race_info.special_powers.push(reader.get_h_string()?);
                    // if let Some(last_power) = race_info.special_powers.last() {
                    //     println!("    [RACE DEBUG] Special Power (NPCS): {}", last_power);
                    // }
                }
                _ => {
                    // println!("    [RACE DEBUG] Skipping unknown subrecord: {}", sub_name.to_string_lossy());
                    reader.skip_h_sub()?;
                }
            }
        }

        if !fnam_found {
            return Err(reader.fail("RACE record missing FNAM (full name) subrecord"));
        }
        if !radt_found {
            return Err(reader.fail(&format!(
                "RACE record for '{}' missing RADT (data) subrecord",
                race_info.full_name
            )));
        }

        Ok(race_info)
    }
}
