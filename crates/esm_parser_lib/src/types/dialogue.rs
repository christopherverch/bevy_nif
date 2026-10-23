/*
use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    format_version_defs::{
        MAX_NAME_IS_REF_ID_ONLY_FORMAT_VERSION, MAX_STRING_REF_ID_FORMAT_VERSION,
    },
    name4,
    types::{
        enums::{DialogueType, RecInt},
        flags::ObjectFlags,
    },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Dialogue {
    pub id: RefId,
    pub string_id: String,
    pub dialogue_type: DialogueType,
    pub info: Vec<DialInfo>,
    pub info_order: InfoOrder<DialInfo>,
}

impl EsmRecord for Dialogue {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Dial;
}

impl Dialogue {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut dialogue = Dialogue::default();
        dialogue.load_id(reader)?;
        dialogue.load_data(reader, is_deleted)?;
        Ok(dialogue)
    }

    fn load_id(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        if reader.header.format_version <= MAX_STRING_REF_ID_FORMAT_VERSION {
            self.string_id = reader.read_header_decode_string()?;
            self.id = RefId::from_str(&self.string_id);
            return Ok(());
        }

        if reader.header.format_version <= MAX_NAME_IS_REF_ID_ONLY_FORMAT_VERSION {
            self.id = reader.read_header_refid_check_name(name4!("NAME"))?;
            return Ok(());
        }

        self.id = reader.read_refid()?;
        Ok(())
    }

    fn load_data(&mut self, reader: &mut EsmReader, is_deleted: &mut bool) -> Result<(), EsmError> {
        *is_deleted = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"DATA" => {
                    reader.read_sub_header()?;
                    let size = reader.get_sub_size();
                    if size == 1 {
                        self.dialogue_type = reader.load()?;
                    } else {
                        reader.skip(size as usize)?;
                        self.dialogue_type = DialogueType::Unknown;
                    }
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    self.dialogue_type = DialogueType::Unknown;
                    *is_deleted = true;
                }
                b"NAME" => {
                    self.string_id = reader.read_header_decode_string()?;
                }
                _ => {
                    return Err(reader.fail("Unknown subrecord"));
                }
            }
        }

        if !*is_deleted
            && MAX_STRING_REF_ID_FORMAT_VERSION < reader.header.format_version
            && reader.header.format_version <= MAX_NAME_IS_REF_ID_ONLY_FORMAT_VERSION
        {
            self.string_id = self.id.to_string();
        }

        Ok(())
    }
    pub fn blank(&mut self) {
        self.dialogue_type = DialogueType::Unknown;
        self.info.clear();
    }

    pub fn read_info(&mut self, reader: &mut EsmReader) -> Result<(), EsmError> {
        let mut info = DialInfo::default();
        let is_deleted = false;
        info.load(reader, is_deleted)?;
        self.info_order.insert_info(info, is_deleted);
        Ok(())
    }

    pub fn set_up(&mut self) {
        self.info_order.remove_deleted();
        self.info_order.extract_ordered_info(&mut self.info);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DialInfo {
    // Define fields based on the actual data structure
}

impl Load for DialInfo {
    fn load(reader: &mut EsmReader, is_deleted: bool) -> Result<Self, EsmError> {
        // Implement loading logic for DialInfo
        Ok(Self::default())
    }
}
*/
