// src/leveled_npc.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeveledNpcEntry {
    pub npc_id: RefId, // ID of the NPC or another leveled list (from CNAM/LCO1)
    pub pc_level: i16, // PC level requirement for this entry (from INTV/LCO2)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeveledNpc {
    pub id: RefId,          // From NAME subrecord (ID of this leveled list)
    pub flags: Option<u32>, // From DATA subrecord (optional)
    // 0x0001=CalcAllLevels<=PC'sLevel, 0x0002=CalcEachItemInCount
    pub chance_none: Option<u8>, // From NNAM subrecord (optional, chance of spawning nothing 0-100)
    // INDX (count of NPCs) is not stored directly by OpenMW; list length implies it.
    pub npcs: Vec<LeveledNpcEntry>, // From CNAM/INTV pairs (or LCO1/LCO2 as older aliases)
}

impl LeveledNpc {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut list = LeveledNpc::new();
        let mut id_found = false;
        let mut item_count_from_indx: Option<i32> = None;
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
                    // Number of NPC entries that follow
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    item_count_from_indx = Some(reader.get_t::<i32>()?);
                }
                // For LVLN, NPC ID is in CNAM (or LCO1) and PC level is in INTV (or LCO2)
                b"CNAM" | b"LCO1" => {
                    let npc_id_val = reader.get_ref_id()?;

                    if reader.has_more_subs() {
                        let level_sub_name = reader.get_sub_name()?;
                        match &level_sub_name.m_data {
                            b"INTV" | b"LCO2" => {
                                reader.get_sub_header()?;
                                if reader.get_sub_size() != 2 {
                                    // PC Level is short
                                    return Err(
                                        reader.report_sub_size_mismatch(2, reader.get_sub_size())
                                    );
                                }
                                let pc_level = reader.get_t::<i16>()?;
                                list.npcs.push(LeveledNpcEntry {
                                    npc_id: npc_id_val,
                                    pc_level,
                                });
                                items_read += 1;
                            }
                            _ => {
                                return Err(reader.fail(&format!(
                                    "LVLN record '{}': Expected INTV/LCO2 after CNAM/LCO1, found {}",
                                    list.id.to_debug_string(), level_sub_name.to_string_lossy()
                                )));
                            }
                        }
                    } else {
                        return Err(reader.fail(&format!(
                            "LVLN record '{}': Missing INTV/LCO2 after CNAM/LCO1",
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
            return Err(reader.fail("LVLN record missing mandatory NAME subrecord for its ID"));
        }
        if let Some(expected_count) = item_count_from_indx {
            if items_read != expected_count as usize && expected_count >= 0 {
                // Warning or error based on strictness
            }
        }

        Ok(list)
    }
}
