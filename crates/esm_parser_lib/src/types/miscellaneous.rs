use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Miscellaneous {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub icon: String,
    pub name: String,
    pub data: MCDTstruct,
    pub script: RefId,
}

impl EsmRecord for Miscellaneous {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Misc;
}

impl Miscellaneous {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut miscellaneous = Miscellaneous::default();
        miscellaneous.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    miscellaneous.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    miscellaneous.model = normalized;
                }
                b"FNAM" => {
                    miscellaneous.name = reader.read_header_decode_string()?;
                }
                b"MCDT" => {
                    reader.read_sub_header()?;
                    miscellaneous.data = reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    miscellaneous.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    miscellaneous.icon = reader.read_header_decode_string()?;
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
            return Err(reader.fail("Missing MCDT subrecord"));
        }

        Ok(miscellaneous)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MCDTstruct {
    pub m_weight: f32,
    pub m_value: i32,
    pub m_flags: i32,
}

impl Load for MCDTstruct {
    fn load(reader: &mut EsmReader) -> Result<MCDTstruct, EsmError> {
        let m_weight = reader.load()?;
        let m_value = reader.load()?;
        let m_flags = reader.load()?;

        Ok(Self {
            m_weight,
            m_value,
            m_flags,
        })
    }
}
