// src/faction.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId;

pub const MAX_FACTION_RANKS: usize = 10;
pub const NUM_FACTION_FAVORED_SKILLS: usize = 7;

// Corresponds to OpenMW's ESM::RankData (20 bytes)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct RankData {
    pub attribute1_req: i32,
    pub attribute2_req: i32,
    pub primary_skill_req: i32,      // One skill at this level
    pub favored_skill_req: i32,      // Two skills at this level
    pub faction_reaction_bonus: i32, // Reaction bonus from members at this rank
}

// Corresponds to OpenMW's ESM::Faction::FADTstruct (240 bytes)
// This is the entire content of the "FADT" subrecord.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct FactionFileSpecificData {
    pub favored_attributes: [i32; 2], // Attribute IDs
    pub rank_specific_data: [RankData; MAX_FACTION_RANKS],
    pub favored_skill_ids: [i32; NUM_FACTION_FAVORED_SKILLS], // Skill IDs
    pub flags: u32, // OpenMW mIsHidden (int32). UESP: 0x1=HiddenFromPlayer, 0x2=Evil
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactionReaction {
    pub faction_id: RefId, // From ANAM subrecord
    pub reaction: i32,     // From INTV subrecord
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Faction {
    pub id: RefId,                                       // From NAME subrecord
    pub full_name: Option<String>,                       // From FNAM subrecord
    pub rank_names: [Option<String>; MAX_FACTION_RANKS], // From RNAM subrecords
    pub data: Option<FactionFileSpecificData>,           // From FADT subrecord
    pub reactions: Vec<FactionReaction>,                 // From ANAM/INTV pairs
}

impl Faction {
    pub fn new() -> Self {
        // Initialize rank_names with None
        const NONE_STRING: Option<String> = None;
        Self {
            rank_names: [NONE_STRING; MAX_FACTION_RANKS],
            ..Default::default()
        }
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut fact = Faction::new();
        let mut id_found = false;
        let mut fadt_found = false;
        let mut fnam_found = false;
        let mut current_rank_idx = 0;
        let mut pending_anam_for_reaction: Option<RefId> = None;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    fact.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"FNAM" => {
                    fact.full_name = Some(reader.get_h_string()?);
                    fnam_found = true;
                }
                b"RNAM" => {
                    // Rank Name, occurs up to 10 times
                    if current_rank_idx < MAX_FACTION_RANKS {
                        fact.rank_names[current_rank_idx] = Some(reader.get_h_string()?);
                        current_rank_idx += 1;
                    } else {
                        reader.skip_h_sub()?; // Skip extra RNAMs beyond 10
                    }
                }
                b"FADT" => {
                    // Faction Data (the large 240-byte block)
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<FactionFileSpecificData>() as u32;
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    // Read the entire block into the struct
                    fact.data = Some(reader.get_t::<FactionFileSpecificData>()?);
                    fadt_found = true;
                }
                b"ANAM" => {
                    // Faction ID for reaction
                    if pending_anam_for_reaction.is_some() {
                        return Err(reader.fail(&format!(
                            "FACT record '{}': Found ANAM without preceding INTV for previous ANAM",
                            fact.id.to_debug_string()
                        )));
                    }
                    pending_anam_for_reaction = Some(reader.get_ref_id()?);
                }
                b"INTV" => {
                    // Reaction value for the preceding ANAM
                    if let Some(faction_id_for_reaction) = pending_anam_for_reaction.take() {
                        reader.get_sub_header()?;
                        if reader.get_sub_size() != 4 {
                            // Reaction is int32
                            return Err(reader.report_sub_size_mismatch(4, reader.get_sub_size()));
                        }
                        let reaction_value = reader.get_t::<i32>()?;
                        fact.reactions.push(FactionReaction {
                            faction_id: faction_id_for_reaction,
                            reaction: reaction_value,
                        });
                    } else {
                        return Err(reader.fail(&format!(
                            "FACT record '{}': Found INTV without preceding ANAM",
                            fact.id.to_debug_string()
                        )));
                    }
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if pending_anam_for_reaction.is_some() {
            // This indicates an ANAM without a following INTV at the end of the record, which is an error.
            return Err(reader.fail(&format!(
                "FACT record '{}': Found ANAM without a following INTV at end of record",
                fact.id.to_debug_string()
            )));
        }

        if !id_found || fact.id.empty() {
            return Err(reader.fail("FACT record missing mandatory NAME subrecord for its ID"));
        }
        if !fnam_found {
            return Err(reader.fail(&format!(
                "FACT record '{}' missing FNAM (full name) subrecord",
                fact.id.to_debug_string()
            )));
        }
        if !fadt_found {
            return Err(reader.fail(&format!(
                "FACT record '{}' missing FADT (faction data) subrecord",
                fact.id.to_debug_string()
            )));
        }

        Ok(fact)
    }
}

