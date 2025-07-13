// src/creature.rs

 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;
// Re-use AiData from npc.rs, ensure npc.rs is created and lib.rs exports AiData
// For NpcItem, we can define it here or also re-use from npc.rs
use crate::ai_wander_data::AiWanderData;
use crate::npc::{AiData, NpcItem};

// Corrected CreatureData based on OpenMW's 96-byte NPDTstruct for CREA
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct CreatureData {
    pub creature_type: i32, // 0=Creature, 1=Daedra, 2=Undead, 3=Humanoid
    pub level: i32,
    pub attributes: [i32; 8], // Strength to Luck
    pub health: i32,
    pub spell_pts: i32, // mMana in OpenMW
    pub fatigue: i32,
    pub soul: i32,
    pub combat_ai: i32,           // mCombat
    pub magic_ai: i32,            // mMagic
    pub stealth_ai: i32,          // mStealth
    pub attack_damages: [i32; 6], // mAttack[6] (Min1,Max1, Min2,Max2, Min3,Max3)
    pub gold: i32,
} // Total: 4+4 + (8*4) + (3*4) + 4 + (3*4) + (6*4) + 4 = 4+4+32+12+4+12+24+4 = 96 bytes.

// Bloodmoon data is not part of OpenMW's main NPDTstruct for Creature.
// It's handled via mFlags and mBloodType (derived from FLAG subrecord)
// and specific werewolf abilities, not an extension of NPDT in their struct.
// So, we won't have a separate CreatureBloodmoonData struct derived from NPDT size.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Creature {
    pub id: RefId,                           // From NAME subrecord
    pub model: Option<String>,               // From MODL
    pub original_creature_id: Option<RefId>, // From CNAM (base creature this is a mod of)
    pub full_name: Option<String>,           // From FNAM (optional)
    pub script_id: Option<RefId>,            // From SCRI (optional)

    pub data: Option<CreatureData>, // From NPDT (96-byte version)
    pub creature_flags: Option<u8>, // From FLAG (lower byte)
    pub blood_type: Option<i32>,    // From FLAG (derived from second byte)

    pub inventory: Vec<NpcItem>, // From NPCO subrecords (loot/items)
    pub spells: Vec<RefId>,      // From NPCS subrecords

    pub ai_data: Option<AiData>,   // From AIDT (6 bytes of data)
    pub ai_packages: Vec<Vec<u8>>, // Placeholder for raw AI package data (AI_W, AI_T etc.)
    // Proper parsing of these needs their own structs.
    // For now, we can store the raw bytes if subrecord size is known.
    // Or use AiWanderData if only AI_W is parsed.
    pub ai_wander: Option<AiWanderData>,

    pub scale: Option<f32>, // From XSCL (optional)
    // SNAM (Sound Gen) is not explicitly in OpenMW's Creature struct fields in crea.hpp
    // but is a valid subrecord.
    pub sound_gen_id: Option<RefId>,
}

impl Creature {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut crea = Creature::new();
        let mut id_found = false;
        let mut npdt_found = false;
        let mut flags_found = false; // For the FLAG subrecord

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    crea.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    crea.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    crea.full_name = Some(reader.get_h_string()?);
                }
                b"CNAM" => {
                    // Original creature ID
                    crea.original_creature_id = Some(reader.get_ref_id()?);
                }
                b"SCRI" => {
                    crea.script_id = Some(reader.get_ref_id()?);
                }
                b"NPDT" => {
                    reader.get_sub_header()?;
                    let sub_size = reader.get_sub_size();
                    let expected_size = std::mem::size_of::<CreatureData>() as u32; // 96 bytes

                    if sub_size != expected_size {
                        // OpenMW uses getSubComposite, which would error if struct size doesn't match subrecord size
                        // unless decompose handles partial reads (it doesn't usually for fixed structs).
                        return Err(reader.report_sub_size_mismatch(expected_size, sub_size));
                    }

                    let mut cdata = CreatureData::default();
                    cdata.creature_type = reader.get_t::<i32>()?;
                    cdata.level = reader.get_t::<i32>()?;
                    for i in 0..8 {
                        cdata.attributes[i] = reader.get_t::<i32>()?;
                    }
                    cdata.health = reader.get_t::<i32>()?;
                    cdata.spell_pts = reader.get_t::<i32>()?;
                    cdata.fatigue = reader.get_t::<i32>()?;
                    cdata.soul = reader.get_t::<i32>()?;
                    cdata.combat_ai = reader.get_t::<i32>()?;
                    cdata.magic_ai = reader.get_t::<i32>()?;
                    cdata.stealth_ai = reader.get_t::<i32>()?;
                    for i in 0..6 {
                        cdata.attack_damages[i] = reader.get_t::<i32>()?;
                    }
                    cdata.gold = reader.get_t::<i32>()?;
                    crea.data = Some(cdata);
                    npdt_found = true;
                }
                b"FLAG" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        // Flags are stored in an int32
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    let flags_raw = reader.get_t::<i32>()?;
                    crea.creature_flags = Some((flags_raw & 0xFF) as u8);
                    crea.blood_type = Some(((flags_raw >> 8) & 0xFF) >> 2); // as i32
                    flags_found = true;
                }
                b"XSCL" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        crea.scale = Some(reader.get_t::<f32>()?);
                    } else {
                        // Skip if not 4 bytes, or handle u8 if necessary
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                b"NPCO" => {
                    reader.get_sub_header()?;
                    let total_npco_size = reader.get_sub_size();
                    if total_npco_size < 4 {
                        return Err(reader.fail("NPCO subrecord in CREA too small for count"));
                    }
                    let count = reader.get_t::<i32>()?;
                    let item_id_str_size = total_npco_size.saturating_sub(4);
                    let item_id_str = reader.get_string_view_internal(item_id_str_size as usize)?;
                    crea.inventory.push(NpcItem {
                        count,
                        item_id: RefId::string_ref_id(&item_id_str),
                    });
                }
                b"NPCS" => {
                    reader.get_sub_header()?;
                    // NPCS (spell ID) is a 32-byte null-padded string in Morrowind files
                    if reader.get_sub_size() != 32 {
                        return Err(reader.report_sub_size_mismatch(32, reader.get_sub_size()));
                    }
                    let spell_id_str = reader.get_string_view_internal(32)?;
                    crea.spells
                        .push(RefId::string_ref_id(&spell_id_str.trim_matches('\0'))); // Trim nulls
                }
                b"AIDT" => {
                    reader.get_sub_header()?;
                    let sub_size = reader.get_sub_size();
                    let expected_aidata_size = std::mem::size_of::<AiData>() as u32; // 6 bytes

                    if sub_size >= expected_aidata_size {
                        let mut aidt_data = AiData::default();
                        aidt_data.hello = reader.get_t::<u8>()?;
                        aidt_data.fight = reader.get_t::<u8>()?;
                        aidt_data.flee = reader.get_t::<u8>()?;
                        aidt_data.alarm = reader.get_t::<u8>()?;
                        aidt_data.services_flags = reader.get_t::<i16>()?;
                        crea.ai_data = Some(aidt_data);

                        if sub_size > expected_aidata_size {
                            // The file has more data for AIDT than OpenMW's base AiData struct (e.g., 12 bytes)
                            // We need to skip the remainder of this subrecord.
                            reader.skip(sub_size - expected_aidata_size)?;
                        }
                    } else {
                        return Err(reader.report_sub_size_mismatch(expected_aidata_size, sub_size));
                    }
                }
                b"AI_W" => {
                    // AI Wander
                    // OpenMW uses mAiPackage.add(esm); which reads a variable structure.
                    // For simplicity, if AiWanderData matches a known fixed size for AI_W:
                    reader.get_sub_header()?;
                    let ai_w_size = std::mem::size_of::<AiWanderData>() as u32;
                    if reader.get_sub_size() == ai_w_size {
                        crea.ai_wander = Some(reader.get_t::<AiWanderData>()?);
                    } else {
                        // Fallback: skip if size doesn't match simple AiWanderData
                        // println!("[CREA WARNING] AI_W for '{}' has size {}, expected {}. Skipping.",
                        //     crea.id.to_debug_string(), reader.get_sub_size(), ai_w_size);
                        reader.skip(reader.get_sub_size())?;
                    }
                }
                // Placeholder for other AI packages (AI_T, AI_F, AI_E, AI_A)
                // These would require their own struct definitions and parsing logic.
                // Example: if AI_T is also a fixed block:
                // b"AI_T" => {
                //     reader.get_sub_header()?;
                //     // Define AiTravelData struct and parse it
                //     // For now, just skip
                //     reader.skip(reader.get_sub_size())?;
                // }
                b"SNAM" => {
                    // Sound Gen ID
                    crea.sound_gen_id = Some(reader.get_ref_id()?);
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || crea.id.empty() {
            return Err(reader.fail("CREA record missing mandatory NAME subrecord for its ID"));
        }
        if !npdt_found {
            // NPDT is mandatory
            return Err(reader.fail(&format!(
                "CREA record '{}' missing NPDT subrecord",
                crea.id.to_debug_string()
            )));
        }
        if !flags_found {
            // FLAG is mandatory
            return Err(reader.fail(&format!(
                "CREA record '{}' missing FLAG subrecord",
                crea.id.to_debug_string()
            )));
        }

        Ok(crea)
    }
}

