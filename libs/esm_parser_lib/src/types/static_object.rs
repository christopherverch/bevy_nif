use crate::{EsmError, EsmReader, Load, RefId, types::flags::ObjectFlags};

#[derive(Debug, Default, Clone)]
pub struct Static {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
}

// Implement the Load trait for your new struct
impl Load for Static {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let mut stat = Self::default();
        stat.flags = reader.get_record_flags();

        let mut has_name = false;
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;
            match sub_name.as_bytes() {
                b"NAME" => {
                    stat.id = reader.read_ref_id()?;
                    has_name = true;
                }
                b"MODL" => {
                    stat.model = reader.read_header_decode_str()?.to_string();
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    stat.flags.insert(ObjectFlags::DELETED);
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
