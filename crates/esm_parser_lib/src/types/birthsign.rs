use crate::{
    EsmError, EsmReader, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags, spell_list::SpellList},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BirthSign {
    pub record_flags: ObjectFlags,
    pub id: RefId,
    pub name: String,
    pub description: String,
    pub texture: String,
    pub powers: SpellList,
}

impl EsmRecord for BirthSign {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Bsgn;
}

impl BirthSign {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut birth_sign = BirthSign::default();
        birth_sign.record_flags = reader.get_record_flags();
        birth_sign.powers.list.clear();

        let mut has_name = false;
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    birth_sign.id = reader.read_refid()?;
                    has_name = true;
                }
                b"FNAM" => {
                    birth_sign.name = reader.read_header_decode_string()?;
                }
                b"TNAM" => {
                    birth_sign.texture = reader.read_header_decode_string()?;
                }
                b"DESC" => {
                    birth_sign.description = reader.read_header_decode_string()?;
                }
                b"NPCS" => {
                    birth_sign.powers.add(reader)?;
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

        Ok(birth_sign)
    }

    pub fn blank(&mut self) {
        self.record_flags = ObjectFlags::empty();
        self.name.clear();
        self.description.clear();
        self.texture.clear();
        self.powers.list.clear();
    }
}
