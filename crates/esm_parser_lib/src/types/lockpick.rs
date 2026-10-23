use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lockpick {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub icon: String,
    pub name: String,
    pub data: LockpickData,
    pub script: RefId,
}

impl EsmRecord for Lockpick {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Lock;
}

impl Lockpick {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut lockpick = Lockpick::default();
        lockpick.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    lockpick.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    lockpick.model = normalized;
                }
                b"FNAM" => {
                    lockpick.name = reader.read_header_decode_string()?;
                }
                b"LKDT" => {
                    lockpick.data = reader.read_typed_header_load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    lockpick.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    lockpick.icon = reader.read_header_decode_string()?;
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
            return Err(reader.fail("Missing LKDT subrecord"));
        }

        Ok(lockpick)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LockpickData {
    pub weight: f32,
    pub value: u32,
    pub quality: f32,
    pub uses: u32,
}

impl Load for LockpickData {
    fn load(reader: &mut EsmReader) -> Result<LockpickData, EsmError> {
        let weight = reader.load()?;
        let value = reader.load()?;
        let quality = reader.load()?;
        let uses = reader.load()?;

        Ok(Self {
            weight,
            value,
            quality,
            uses,
        })
    }
}
