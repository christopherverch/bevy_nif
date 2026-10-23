use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Door {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub name: String,
    pub script: RefId,
    pub open_sound: RefId,
    pub close_sound: RefId,
}

impl EsmRecord for Door {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Door;
}

impl Door {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut door = Door::default();
        door.flags = reader.get_record_flags();
        let mut has_name = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    door.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    door.model = normalized;
                }
                b"FNAM" => {
                    door.name = reader.read_header_decode_string()?;
                }
                b"SCRI" => {
                    door.script = reader.read_refid()?;
                }
                b"SNAM" => {
                    door.open_sound = reader.read_refid()?;
                }
                b"ANAM" => {
                    door.close_sound = reader.read_refid()?;
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

        Ok(door)
    }
}
