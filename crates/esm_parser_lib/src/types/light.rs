use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};
use esm_parser_macros::LoadSave;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Light {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub icon: String,
    pub name: String,
    pub data: LightData,
    pub script: RefId,
    pub sound: RefId,
}

impl EsmRecord for Light {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Ligh;
}

impl Light {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut light = Light::default();
        light.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    light.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    light.model = normalized;
                }
                b"FNAM" => {
                    light.name = reader.read_header_decode_string()?;
                }
                b"LHDT" => {
                    reader.read_sub_header()?;
                    light.data = reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    light.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    light.icon = reader.read_header_decode_string()?;
                }
                b"SNAM" => {
                    light.sound = reader.read_refid()?;
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
            return Err(reader.fail("Missing LHDT subrecord"));
        }
        Ok(light)
    }
}

#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct LightData {
    pub weight: f32,
    pub value: i32,
    pub time: i32,
    pub radius: i32,
    pub color: u32,
    pub flags: i32,
}
