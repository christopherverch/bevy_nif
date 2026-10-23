use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{
        armor::PartReference,
        enums::{ClothingType, RecInt},
        flags::ObjectFlags,
    },
};
use esm_parser_macros::LoadSave;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clothing {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub icon: String,
    pub name: String,
    pub data: ClothingData,
    pub parts: Vec<PartReference>,
    pub script: RefId,
    pub enchant: RefId,
}

impl EsmRecord for Clothing {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Clot;
}

impl Clothing {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut clothing = Clothing::default();
        clothing.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    clothing.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    clothing.model = normalized;
                }
                b"FNAM" => {
                    clothing.name = reader.read_header_decode_string()?;
                }
                b"CTDT" => {
                    reader.read_sub_header()?;
                    clothing.data = reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    clothing.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    clothing.icon = reader.read_header_decode_string()?;
                }
                b"ENAM" => {
                    clothing.enchant = reader.read_refid()?;
                }
                b"INDX" => {
                    let part = PartReference::load(reader)?;
                    clothing.parts.push(part);
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => {
                    return Err(reader.fail("Unknown subrecord"));
                }
            }
        }

        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if !has_data && !*is_deleted {
            return Err(reader.fail("Missing CTDT subrecord"));
        }

        Ok(clothing)
    }
}

#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct ClothingData {
    pub clothing_type: ClothingType,
    pub weight: f32,
    pub value: u16,
    pub enchant: u16,
}
