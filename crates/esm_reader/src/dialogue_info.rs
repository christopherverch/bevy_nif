// src/dialogue_info.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // Re-use DialogueType enum

// Corrected DATA subrecord structure for INFO, matching OpenMW's 12-byte interpretation
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[repr(C, packed)]
pub struct DialogueInfoData {
    pub dialogue_type_raw: i32, // Corresponds to OpenMW mType (int32_t), use DialogueType enum after parsing
    pub disposition_or_journal_index: i32, // Corresponds to OpenMW union mDisposition/mJournalIndex (int32_t)
    pub rank_req: i8,                      // Required rank in faction (OpenMW: signed char mRank)
    pub sex_filter: i8,                    // -1=Any, 0=Male, 1=Female (OpenMW: signed char mGender)
    pub pc_rank_req: i8, // Player's required rank in their faction (OpenMW: signed char mPCrank)
    pub padding: u8,     // 1 byte of padding to make it 12 bytes, as per OpenMW decompose
} // Total: 4 + 4 + 1 + 1 + 1 + 1 = 12 bytes

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DialogueInfo {
    pub id: RefId,              // From INAM subrecord
    pub prev_id: Option<RefId>, // From PNAM
    pub next_id: Option<RefId>, // From NNAM

    pub data: Option<DialogueInfoData>, // From DATA subrecord

    pub actor_id: Option<RefId>, // From ONAM (OpenMW: mActor) - was SNAM in my prev. simplified, ONAM is correct.
    pub race_id: Option<RefId>,  // From RNAM
    pub class_id: Option<RefId>, // From CNAM
    pub faction_id: Option<RefId>, // From FNAM (Faction whose rank is checked by mRank)
    pub pc_faction_id: Option<RefId>, // From DNAM (Player's faction for mPCrank check)
    pub cell_id: Option<RefId>,  // From ANAM

    pub sound_path: Option<String>,    // From SNAM (OpenMW: mSound)
    pub response_text: Option<String>, // From NAME subrecord (OpenMW: mResponse)

    pub result_script_text: Option<String>, // From BNAM

    pub quest_status_flags: Option<u32>, // Bitmask for QSTN, QSTF, QSTR (not directly stored in OpenMW like this, but useful)
    // QSTN: Name (true) = 1
    // QSTF: Finished (true) = 2
    // QSTR: Restart (true) = 3
    // These are actually separate empty subrecords in the file.
    pub quest_name: bool,     // From QSTN subrecord (if present)
    pub quest_finished: bool, // From QSTF subrecord (if present)
    pub quest_restart: bool,  // From QSTR subrecord (if present)

                              // SCVR (Script Variable Filters / Conditions) are complex lists and omitted for now.
                              // OpenMW ESM::DialInfo has `std::vector<DialogueCondition> mSelects;`
}

impl DialogueInfo {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut info = DialogueInfo::new();
        let mut id_found = false;
        let mut data_sub_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"INAM" => {
                    info.id = reader.get_ref_id()?;
                    id_found = true;
                }
                b"PNAM" => {
                    info.prev_id = Some(reader.get_ref_id()?);
                }
                b"NNAM" => {
                    info.next_id = Some(reader.get_ref_id()?);
                }
                b"DATA" => {
                    reader.get_sub_header()?;
                    let expected_size = std::mem::size_of::<DialogueInfoData>() as u32; // 12 bytes
                    if reader.get_sub_size() != expected_size {
                        return Err(
                            reader.report_sub_size_mismatch(expected_size, reader.get_sub_size())
                        );
                    }
                    info.data = Some(reader.get_t::<DialogueInfoData>()?);
                    data_sub_found = true;
                }
                b"ONAM" => {
                    info.actor_id = Some(reader.get_ref_id()?);
                } // Speaker Actor
                b"RNAM" => {
                    info.race_id = Some(reader.get_ref_id()?);
                } // Speaker Race
                b"CNAM" => {
                    info.class_id = Some(reader.get_ref_id()?);
                } // Speaker Class
                b"FNAM" => {
                    info.faction_id = Some(reader.get_ref_id()?);
                } // Speaker Faction whose rank is checked
                b"ANAM" => {
                    info.cell_id = Some(reader.get_ref_id()?);
                } // Player in Cell
                b"DNAM" => {
                    info.pc_faction_id = Some(reader.get_ref_id()?);
                } // Player's Faction for PC rank check

                b"SNAM" => {
                    // Sound filename (voice for this line)
                    info.sound_path = Some(reader.get_h_string()?);
                }
                b"NAME" => {
                    // Response Text
                    info.response_text = Some(reader.get_h_string()?);
                }
                b"BNAM" => {
                    // Result Script Text
                    info.result_script_text = Some(reader.get_h_string()?);
                }
                // Quest status flags (these are empty subrecords, their presence is the flag)
                b"QSTN" => {
                    info.quest_name = true;
                    reader.skip_h_sub()?;
                } // Quest Name is known/active
                b"QSTF" => {
                    info.quest_finished = true;
                    reader.skip_h_sub()?;
                } // Quest is Finished
                b"QSTR" => {
                    info.quest_restart = true;
                    reader.skip_h_sub()?;
                } // Quest can be Restarted

                // SCVR (conditions) are complex and typically parsed into a list of DialogueCondition structs.
                // Skipping for this pass.
                b"SCVR" => {
                    reader.skip_h_sub()?;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || info.id.empty() {
            return Err(reader.fail("INFO record missing mandatory INAM subrecord for its ID"));
        }
        if !data_sub_found {
            return Err(reader.fail(&format!(
                "INFO record '{}' missing DATA subrecord",
                info.id.to_debug_string()
            )));
        }

        Ok(info)
    }
} 

