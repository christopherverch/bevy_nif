use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activator {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub script: RefId,
    pub name: String,
    pub model: String,
}

impl EsmRecord for Activator {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Acti;
}

impl Activator {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut activator = Activator::default();
        activator.flags = reader.get_record_flags();

        let mut has_name = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    activator.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    activator.model = model.replace('\\', "/");
                }
                b"FNAM" => {
                    activator.name = reader.read_header_decode_string()?;
                }
                b"SCRI" => {
                    activator.script = reader.read_refid()?;
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

        Ok(activator)
    }
}
