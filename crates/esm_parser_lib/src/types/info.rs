use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    name4,
    types::{
        dialogue_condition::DialogueCondition,
        enums::{DialogueType, RecInt, Sex},
        flags::ObjectFlags,
    },
};
use esm_parser_macros::LoadSave;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DialInfo {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub prev: RefId,
    pub next: RefId,
    pub data: DATAstruct,
    pub actor: RefId,
    pub race: RefId,
    pub class: RefId,
    pub faction: RefId,
    pub pc_faction: RefId,
    pub cell: RefId,
    pub sound: String,
    pub response: String,
    pub result_script: String,
    pub faction_less: bool,
    pub quest_status: QuestStatus,
    pub selects: Vec<DialogueCondition>,
}

impl EsmRecord for DialInfo {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Dial;
}

impl DialInfo {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut dial_info = DialInfo::default();

        // Match the C++ header reads: INAM, PNAM, NNAM
        dial_info.id = reader.read_header_refid_check_name(name4!("INAM"))?;
        dial_info.prev = reader.read_header_refid_check_name(name4!("PNAM"))?;
        dial_info.next = reader.read_header_refid_check_name(name4!("NNAM"))?;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"DATA" => {
                    reader.read_sub_header()?;
                    dial_info.data = reader.load()?;
                }
                b"ONAM" => {
                    dial_info.actor = reader.read_refid()?;
                }
                b"RNAM" => {
                    dial_info.race = reader.read_refid()?;
                }
                b"CNAM" => {
                    dial_info.class = reader.read_refid()?;
                }
                b"FNAM" => {
                    dial_info.faction = reader.read_refid()?;
                    if dial_info.faction == RefId::from_str("FFFF") {
                        dial_info.faction_less = true;
                    }
                }
                b"ANAM" => {
                    dial_info.cell = reader.read_refid()?;
                }
                b"DNAM" => {
                    dial_info.pc_faction = reader.read_refid()?;
                }
                b"SNAM" => {
                    dial_info.sound = reader.read_header_decode_string()?;
                }
                b"NAME" => {
                    dial_info.response = reader.read_header_decode_string()?;
                }
                b"SCVR" => {
                    if let Some(filter) = DialogueCondition::load(reader, &dial_info.id)? {
                        dial_info.selects.push(filter);
                    }
                }
                b"BNAM" => {
                    dial_info.result_script = reader.read_header_decode_string()?;
                }
                b"QSTN" => {
                    reader.skip_sub_header()?;
                    dial_info.quest_status = QuestStatus::Name;
                }
                b"QSTF" => {
                    reader.skip_sub_header()?;
                    dial_info.quest_status = QuestStatus::Finished;
                }
                b"QSTR" => {
                    reader.skip_sub_header()?;
                    dial_info.quest_status = QuestStatus::Restart;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => {
                    return Err(
                        reader.fail(&format!("Unknown subrecord: {:?}", sub_name.as_bytes()))
                    );
                }
            }
        }
        Ok(dial_info)
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, LoadSave)]
pub struct DATAstruct {
    pub dialogue_type: DialogueType,
    pub disposition_or_journal: i32,
    pub rank: i8,
    pub gender: Sex,
    pub pc_rank: i8,
    _padding: i8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuestStatus {
    None,
    Name,
    Finished,
    Restart,
}

impl Default for QuestStatus {
    fn default() -> Self {
        QuestStatus::None
    }
}
