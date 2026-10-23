use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};
use esm_parser_macros::LoadSave;

#[derive(Clone, Debug, Default, PartialEq, LoadSave)]
pub struct ProbeData {
    pub weight: f32,
    pub value: i32,
    pub quality: f32,
    pub uses: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probe {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub icon: String,
    pub name: String,
    pub data: ProbeData,
    pub script: RefId,
}

impl EsmRecord for Probe {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Prob;
}

impl Probe {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut probe = Probe::default();
        probe.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    probe.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    probe.model = normalized;
                }
                b"FNAM" => {
                    probe.name = reader.read_header_decode_string()?;
                }
                b"PBDT" => {
                    reader.read_sub_header()?;
                    probe.data = reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    probe.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    probe.icon = reader.read_header_decode_string()?;
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
            return Err(reader.fail("Missing PBDT subrecord"));
        }

        Ok(probe)
    }
}
