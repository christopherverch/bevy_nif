// src/leveled_creature.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeveledCreatureEntry {
    pub creature_id: RefId, // ID of the creature or another leveled list (from CNAM/CCO1)
    pub pc_level: i16,      // PC level requirement for this entry (from INTV/CCO2)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeveledCreature {
    pub id: RefId,          // From NAME subrecord (ID of this leveled list)
    pub flags: Option<u32>, // From DATA subrecord (optional)
    // 0x0001=CalcAllLevels<=PC'sLevel, 0x0002=CalcEachItemInCount
    // 0x0004=MustSpawn (This flag is actually significant for LEVC)
    pub chance_none: Option<u8>, // From NNAM subrecord (optional, chance of spawning nothing)
    // INDX (count of creatures) is not typically stored directly by OpenMW; list length implies it.
    pub creatures: Vec<LeveledCreatureEntry>, // From CNAM/INTV pairs (or CCO1/CCO2)
}

impl LeveledCreature {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut list = LeveledCreature::new();
        let mut id_found = false;
        let mut item_count_from_indx: Option<i32> = None; // INDX is optional for list validity in OpenMW
        let mut items_read = 0;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    list.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"DATA" => {
                    // Flags
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    list.flags = Some(reader.get_t::<u32>()?);
                }
                b"NNAM" => {
                    // Chance None
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 1 {
                        return Err(reader.report_sub_size_mismatch(1, reader.get_sub_size()));
                    }
                    list.chance_none = Some(reader.get_t::<u8>()?);
                }
                b"INDX" => {
                    // Number of creature entries that follow
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    item_count_from_indx = Some(reader.get_t::<i32>()?);
                }
                // In LEVC, creature ID is in CNAM (or CCO1 as an older alias)
                // and PC level is in INTV (or CCO2 as an older alias)
                b"CNAM" | b"CCO1" => {
                    let creature_id = reader.get_ref_id()?; // Assumes entire subrecord data is RefId

                    if reader.has_more_subs() {
                        let level_sub_name = reader.get_sub_name()?;
                        match &level_sub_name.m_data {
                            b"INTV" | b"CCO2" => {
                                reader.get_sub_header()?;
                                if reader.get_sub_size() != 2 {
                                    // PC Level is short
                                    return Err(
                                        reader.report_sub_size_mismatch(2, reader.get_sub_size())
                                    );
                                }
                                let pc_level = reader.get_t::<i16>()?;
                                list.creatures.push(LeveledCreatureEntry {
                                    creature_id,
                                    pc_level,
                                });
                                items_read += 1;
                            }
                            _ => {
                                return Err(reader.fail(&format!(
                                    "LEVC record '{}': Expected INTV/CCO2 after CNAM/CCO1, found {}",
                                    list.id.to_debug_string(), level_sub_name.to_string_lossy()
                                )));
                            }
                        }
                    } else {
                        return Err(reader.fail(&format!(
                            "LEVC record '{}': Missing INTV/CCO2 after CNAM/CCO1",
                            list.id.to_debug_string()
                        )));
                    }
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || list.id.empty() {
            return Err(reader.fail("LEVC record missing mandatory NAME subrecord for its ID"));
        }
        if let Some(expected_count) = item_count_from_indx {
            if items_read != expected_count as usize && expected_count > 0 { // Only warn if INDX was present and non-zero
                // OpenMW doesn't strictly enforce INDX match if list items are present.
                // println!("[LEVC WARNING] Record '{}': INDX count {} but found {} creature entries.",
                //     list.id.to_debug_string(), expected_count, items_read);
            }
        } else if list.creatures.is_empty() && item_count_from_indx.map_or(true, |c| c > 0) {
            // If no items and INDX was present and >0, or INDX absent but list still expected items.
            // OpenMW allows empty lists (mChanceNone would be 100 then).
            // For now, not erroring on empty list if INDX wasn't explicitly positive.
        }

        Ok(list)
    }
}
