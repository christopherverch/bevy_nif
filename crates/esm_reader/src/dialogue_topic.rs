// src/dialogue_topic.rs

use crate::esm_reader::{EsmError, EsmReader};
use crate::refid_types::RefId; // DIAL ID is a RefId

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueType {
    Topic = 0,      // Standard topic
    Voice = 1,      // Voice lines (Greeting, Persuasion etc.)
    Greeting = 2,   // Specific greeting
    Persuasion = 3, // Persuasion results
    Journal = 4,    // Journal updates
    Unknown = 255,
}

impl Default for DialogueType {
    fn default() -> Self {
        DialogueType::Unknown
    }
}

impl From<u8> for DialogueType {
    fn from(val: u8) -> Self {
        match val {
            0 => DialogueType::Topic,
            1 => DialogueType::Voice,
            2 => DialogueType::Greeting,
            3 => DialogueType::Persuasion,
            4 => DialogueType::Journal,
            _ => DialogueType::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DialogueTopic {
    pub id: RefId, // From NAME subrecord (Record ID)
    pub dialogue_type: DialogueType, // From DATA subrecord (a single byte)
                   // INFO records associated with this DIAL are separate records in the ESM.
}

impl DialogueTopic {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut dial = DialogueTopic::new();
        let mut id_found = false;
        let mut data_found = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_sub_name()?;

            match &sub_name.m_data {
                b"NAME" => {
                    dial.id = reader.get_ref_id()?; // DIAL ID is a RefId (string)
                    id_found = true;
                }
                b"DATA" => {
                    // Contains the dialogue type byte
                    reader.get_sub_header()?;
                    if reader.get_sub_size() != 1 {
                        return Err(reader.report_sub_size_mismatch(1, reader.get_sub_size()));
                    }
                    let type_byte = reader.get_t::<u8>()?;
                    dial.dialogue_type = DialogueType::from(type_byte);
                    data_found = true;
                }
                _ => {
                    reader.skip_h_sub()?;
                }
            }
        }

        if !id_found || dial.id.empty() {
            return Err(reader.fail("DIAL record missing mandatory NAME subrecord for its ID"));
        }
        if !data_found {
            return Err(reader.fail(&format!(
                "DIAL record '{}' missing DATA (type) subrecord",
                dial.id.to_debug_string()
            )));
        }

        Ok(dial)
    }
}
