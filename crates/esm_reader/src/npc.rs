// src/npc.rs

use crate::ai_wander_data::AiWanderData;
 // Using title case Name
use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct NpcData {
    pub level: i16,
    pub attributes: [u8; 8],
    pub skills: [u8; 27],
    pub reputation: u8,
    pub health: i16,
    pub spell_pts: i16,
    pub fatigue: i16,
    pub disposition: i8,
    pub faction_id_idx: u8,
    pub rank: u8,
    pub unknown_padding: u8,
    pub gold: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NpcItem {
    pub count: i32,
    pub item_id: RefId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct AiData {
    // OpenMW AIDTstruct is 6 bytes
    pub hello: u8,
    pub fight: u8,
    pub flee: u8,
    pub alarm: u8,
    pub services_flags: i16,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Npc {
    pub id: RefId,
    pub model: Option<String>,
    pub full_name: Option<String>,
    pub race_id: Option<RefId>,
    pub class_id: Option<RefId>,
    pub faction_id: Option<RefId>,
    pub head_model_id: Option<RefId>,
    pub hair_model_id: Option<RefId>,
    pub script_id: Option<RefId>,

    pub data: Option<NpcData>,
    pub flags: Option<u32>,

    pub inventory: Vec<NpcItem>,
    pub spells: Vec<RefId>,

    pub ai_data: Option<AiData>,
    pub ai_wander: Option<AiWanderData>,

    pub travel_destinations: Vec<(f32, f32, f32, i32, i32, i32)>,

    pub default_pos: Option<(f32, f32, f32, f32, f32, f32)>,
    pub cell_if_no_doodt: Option<RefId>,

    pub scale: Option<f32>,
}

impl Npc {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut npc = Npc::new();
        let mut id_found = false;
        let mut npdt_found = false; // To ensure NPDT is processed
        let mut is_deleted = false; // Declare is_deleted locally

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    npc.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"MODL" => {
                    npc.model = Some(reader.get_h_string()?);
                }
                b"FNAM" => {
                    npc.full_name = Some(reader.get_h_string()?);
                }
                b"RNAM" => {
                    npc.race_id = Some(reader.get_ref_id()?);
                }
                b"CNAM" => {
                    npc.class_id = Some(reader.get_ref_id()?);
                }
                b"ANAM" => {
                    npc.faction_id = Some(reader.get_ref_id()?);
                }
                b"BNAM" => {
                    npc.head_model_id = Some(reader.get_ref_id()?);
                }
                b"KNAM" => {
                    npc.hair_model_id = Some(reader.get_ref_id()?);
                }
                b"SCRI" => {
                    npc.script_id = Some(reader.get_ref_id()?);
                }
                b"NPDT" => {
                    reader.get_sub_header()?;
                    let sub_size = reader.get_sub_size();
                    let expected_size = std::mem::size_of::<NpcData>() as u32;
                    if sub_size >= expected_size {
                        npc.data = Some(reader.get_t::<NpcData>()?);
                        if sub_size > expected_size {
                            reader.skip(sub_size - expected_size)?;
                        }
                        npdt_found = true;
                    } else {
                        return Err(reader.report_sub_size_mismatch(expected_size, sub_size));
                    }
                }
                b"FLAG" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 4 {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                    npc.flags = Some(reader.get_t::<u32>()?);
                }
                b"NPCO" => {
                    reader.get_sub_header()?;
                    let total_npco_size = reader.get_sub_size();
                    if total_npco_size < 4 {
                        return Err(reader.fail("NPCO subrecord in NPC_ too small for count"));
                    }
                    let count = reader.get_t::<i32>()?;
                    let item_id_str_size = total_npco_size.saturating_sub(4);

                    let item_id_str = reader.get_string_view_internal(item_id_str_size as usize)?;
                    npc.inventory.push(NpcItem {
                        count,
                        item_id: RefId::string_ref_id(&item_id_str),
                    });
                }
                b"NPCS" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 32 {
                        // NPCS Spell ID is a 32-byte string
                        return Err(reader.report_sub_size_mismatch(32, reader.get_sub_size()));
                    }
                    let spell_id_str = reader.get_string_view_internal(32)?;
                    npc.spells.push(RefId::string_ref_id(&spell_id_str));
                }
                b"AIDT" => {
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<AiData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    let mut aidt_data = AiData::default();
                    aidt_data.hello = reader.get_t::<u8>()?;
                    aidt_data.fight = reader.get_t::<u8>()?;
                    aidt_data.flee = reader.get_t::<u8>()?;
                    aidt_data.alarm = reader.get_t::<u8>()?;
                    aidt_data.services_flags = reader.get_t::<i16>()?;
                    npc.ai_data = Some(aidt_data);
                }
                b"AI_W" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != std::mem::size_of::<AiWanderData>() as u32 {
                        return Err(reader.report_sub_size_mismatch(
                            std::mem::size_of::<AiWanderData>() as u32,
                            reader.get_sub_size(),
                        ));
                    }
                    npc.ai_wander = Some(reader.get_t::<AiWanderData>()?); // Now works with Copy
                }
                b"AI_T" => {
                    reader.get_sub_header()?;
                    // OpenMW AIDestination is 3 floats (12) + 3 u16 (6) = 18 bytes.
                    // If file reports 20, 2 are padding.
                    let sub_size = reader.get_sub_size();
                    if sub_size != 18 && sub_size != 20 {
                        return Err(reader.fail(&format!(
                            "AI_T subrecord has unexpected size {} (expected 18 or 20)",
                            sub_size
                        )));
                    }
                    let x = reader.get_t::<f32>()?;
                    let y = reader.get_t::<f32>()?;
                    let z = reader.get_t::<f32>()?;
                    let rx = reader.get_t::<u16>()?;
                    let ry = reader.get_t::<u16>()?;
                    let rz = reader.get_t::<u16>()?;
                    if sub_size == 20 {
                        // Skip 2 padding bytes if present
                        reader.skip(2)?;
                    }
                    npc.travel_destinations
                        .push((x, y, z, rx as i32, ry as i32, rz as i32));
                }
                b"DODT" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 24 {
                        return Err(reader.report_sub_size_mismatch(24, reader.get_sub_size()));
                    }
                    let x = reader.get_t::<f32>()?;
                    let y = reader.get_t::<f32>()?;
                    let z = reader.get_t::<f32>()?;
                    let rx = reader.get_t::<f32>()?;
                    let ry = reader.get_t::<f32>()?;
                    let rz = reader.get_t::<f32>()?;
                    npc.default_pos = Some((x, y, z, rx, ry, rz));
                }
                b"DNAM" => {
                    npc.cell_if_no_doodt = Some(reader.get_ref_id()?);
                }
                b"XSCL" => {
                    reader.get_sub_header()?;
                    if reader.get_sub_size() == 4 {
                        npc.scale = Some(reader.get_t::<f32>()?);
                    } else if reader.get_sub_size() == 1 {
                        npc.scale = Some(reader.get_t::<u8>()? as f32);
                    } else {
                        return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                    }
                }
                b"DELE" => {
                    // Handle DELE subrecord
                    reader.skip_h_sub()?;
                    is_deleted = true;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || npc.id.empty() {
            return Err(reader.fail("NPC_ record missing mandatory NAME subrecord for its ID"));
        }
        if npc.data.is_none() && !is_deleted {
            // Use the local is_deleted flag
            return Err(reader.fail(&format!(
                "NPC_ record '{}' missing NPDT subrecord",
                npc.id.to_debug_string()
            )));
        }

        Ok(npc)
    }
}

