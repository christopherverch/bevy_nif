use crate::{EsmError, EsmReader, RefId, esm_record::EsmRecord, name4, types::flags::ObjectFlags};

#[derive(Debug, Default, Clone)]
pub struct Static {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub name: String,
    pub model: String,
}
impl EsmRecord for Static {
    fn id(&self) -> &RefId {
        &self.id
    }

    const RECORD_ID: u32 = name4!("STAT").to_u32();
}

impl Static {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut stat = Self::default();
        stat.record_flags = reader.get_record_flags();

        let mut has_name = false;
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    stat.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    stat.model = reader.read_header_decode_str()?.to_string();
                }
                b"FNAM" => {
                    stat.name = reader.read_header_decode_str()?.to_string();
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => {
                    return Err(reader.fail(&format!(
                        "Unknown subrecord {} in STAT",
                        sub_name.to_string_lossy()
                    )));
                }
            }
        }
        if !has_name {
            return Err(reader.fail("Static object missing NAME subrecord"));
        }
        Ok(stat)
    }
}
