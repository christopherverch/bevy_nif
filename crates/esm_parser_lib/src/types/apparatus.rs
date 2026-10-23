use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Apparatus {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub script: RefId,
    pub model: String,
    pub name: String,
    pub icon: String,
    pub data: ApparatusData,
}

impl EsmRecord for Apparatus {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Appa;
}

impl Apparatus {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut apparatus = Apparatus::default();
        apparatus.flags = reader.get_record_flags();
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    apparatus.id = reader.read_refid()?;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    apparatus.model = normalized;
                }
                b"FNAM" => {
                    apparatus.name = reader.read_header_decode_string()?;
                }
                b"SCRI" => {
                    apparatus.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    apparatus.icon = reader.read_header_decode_string()?;
                }
                b"AADT" => {
                    reader.read_sub_header()?;
                    apparatus.data = reader.load()?;
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

        if apparatus.id.is_empty() {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if apparatus.data.m_type == -1 && !*is_deleted {
            return Err(reader.fail("Missing AADT subrecord"));
        }

        Ok(apparatus)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ApparatusData {
    pub m_type: i32,
    pub m_quality: f32,
    pub m_weight: f32,
    pub m_value: i32,
}

impl Load for ApparatusData {
    fn load(reader: &mut EsmReader) -> Result<ApparatusData, EsmError> {
        let [m_type, m_value] = reader.load()?;
        let [m_quality, m_weight] = reader.load()?;

        Ok(Self {
            m_type,
            m_quality,
            m_weight,
            m_value,
        })
    }
}
