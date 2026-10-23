use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub flags: ObjectFlags,
    pub id: RefId,
    pub model: String,
    pub name: String,
    pub icon: String,
    pub script: RefId,
    pub enchant: RefId,
    pub text: String,
    pub data: BookData,
}

impl EsmRecord for Book {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Book;
}

impl Book {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut book = Book::default();
        book.flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;

        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    book.id = reader.read_refid()?;
                    has_name = true;
                }
                b"MODL" => {
                    let model = reader.read_header_decode_string()?;
                    let normalized = model.replace('\\', "/");
                    book.model = normalized;
                }
                b"FNAM" => {
                    book.name = reader.read_header_decode_string()?;
                }
                b"BKDT" => {
                    reader.read_sub_header()?;
                    book.data = reader.load()?;
                    has_data = true;
                }
                b"SCRI" => {
                    book.script = reader.read_refid()?;
                }
                b"ITEX" => {
                    book.icon = reader.read_header_decode_string()?;
                }
                b"ENAM" => {
                    book.enchant = reader.read_refid()?;
                }
                b"TEXT" => {
                    book.text = reader.read_header_decode_string()?;
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
            return Err(reader.fail("Missing BKDT subrecord"));
        }

        Ok(book)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookData {
    pub weight: f32,
    pub value: i32,
    pub is_scroll: i32,
    pub skill_id: i32,
    pub enchant: i32,
}

impl Load for BookData {
    fn load(reader: &mut EsmReader) -> Result<BookData, EsmError> {
        let weight = reader.load()?;
        let [value, is_scroll, skill_id, enchant] = reader.load()?;

        Ok(Self {
            weight,
            value,
            is_scroll,
            skill_id,
            enchant,
        })
    }
}
