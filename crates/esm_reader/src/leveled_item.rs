// src/leveled_item.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeveledItemEntry {
    pub item_id: RefId, // ID of the item or leveled list to spawn (from NPCO/PCO1)
    pub pc_level: i16,  // PC level requirement for this entry (from INTV/PCO2)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeveledItem {
    pub id: RefId,          // From NAME subrecord (ID of this leveled list)
    pub flags: Option<u32>, // From DATA subrecord (optional)
    // 0x0001=CalcAllLevels<=PC'sLevel, 0x0002=CalcEachItemInCount
    // 0x0004=MustSpawn (UESP says this flag is for LEVC only)
    pub chance_none: Option<u8>, // From NNAM subrecord (optional, chance of spawning nothing)
    // INDX (count of items) is not stored directly; list length implies it.
    pub items: Vec<LeveledItemEntry>, // From NPCO/INTV pairs
}

impl LeveledItem {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut list = LeveledItem::new();
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
                    // Number of item entries that follow
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    item_count_from_indx = Some(reader.get_t::<i32>()?);
                }
                b"NPCO" | b"PCO1" => {
                    // Item ID
                    // This subrecord should be followed by INTV or PCO2 for PC level
                    let item_id = reader.get_ref_id()?; // Assuming entire subrecord data is the RefId

                    // Now expect INTV or PCO2
                    if reader.has_more_subs() {
                        let level_sub_name = reader.get_sub_name()?;
                        match &level_sub_name.m_data {
                            b"INTV" | b"PCO2" => {
                                reader.get_sub_header()?;
                                if reader.get_sub_size() != 2 {
                                    // PC Level is short
                                    return Err(
                                        reader.report_sub_size_mismatch(2, reader.get_sub_size())
                                    );
                                }
                                let pc_level = reader.get_t::<i16>()?;
                                list.items.push(LeveledItemEntry { item_id, pc_level });
                                items_read += 1;
                            }
                            _ => {
                                return Err(reader.fail(&format!(
                                    "LEVI record '{}': Expected INTV/PCO2 after NPCO/PCO1, found {}",
                                    list.id.to_debug_string(), level_sub_name.to_string_lossy()
                                )));
                            }
                        }
                    } else {
                        return Err(reader.fail(&format!(
                            "LEVI record '{}': Missing INTV/PCO2 after NPCO/PCO1",
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
            return Err(reader.fail("LEVI record missing mandatory NAME subrecord for its ID"));
        }
        if let Some(expected_count) = item_count_from_indx {
            if items_read != expected_count as usize {
                // This could be a warning or an error, depending on strictness.
                // OpenMW seems to trust the number of NPCO/INTV pairs it finds.
                // println!("[LEVI WARNING] Record '{}': INDX count {} but found {} items.",
                //     list.id.to_debug_string(), expected_count, items_read);
            }
        } else if !list.items.is_empty() { // Items found but no INDX
            // OpenMW doesn't require INDX if list items are present.
            // println!("[LEVI WARNING] Record '{}': Items present but INDX subrecord missing.", list.id.to_debug_string());
        }

        Ok(list)
    }
}
